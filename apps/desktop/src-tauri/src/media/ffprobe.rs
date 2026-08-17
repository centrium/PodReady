use crate::error::AppError;
use crate::media::analysis::AudioMeasurements;
use crate::media::binaries::ffprobe_cmd;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MediaFormat {
    WAV,
    MP3,
    M4A,
    MOV,
    MP4,
    UNKNOWN,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInspection {
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: Option<u32>,
    pub file_size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaSource {
    pub path: String,
    pub filename: String,
    pub format: MediaFormat,
    pub codec: String,
    pub inspection: MediaInspection,
    pub measurements: Option<AudioMeasurements>,
}

#[derive(Deserialize, Debug)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

#[derive(Deserialize, Debug)]
struct FfprobeStream {
    codec_type: String,
    codec_name: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize, Debug)]
struct FfprobeFormat {
    format_name: String,
    size: String,
    duration: Option<String>,
    bit_rate: Option<String>,
}

pub fn inspect_media<P: AsRef<Path>>(path: P) -> Result<MediaSource, AppError> {
    let path_ref = path.as_ref();
    let path_str = path_ref.to_string_lossy().to_string();
    let filename = path_ref
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let output = ffprobe_cmd()
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &path_str,
        ])
        .output()
        .map_err(|e| {
            log::error!("Failed to execute ffprobe: {}", e);
            AppError::MediaInspectionFailed("Failed to execute ffprobe".into())
        })?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        log::error!("ffprobe exited with error: {}", err_str);
        return Err(AppError::MediaInspectionFailed(err_str.into_owned()));
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout).map_err(|e| {
        log::error!("Failed to parse ffprobe JSON: {}", e);
        AppError::MediaInspectionFailed("Invalid output from ffprobe".into())
    })?;

    // Find the first audio stream
    let audio_stream = parsed
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .ok_or(AppError::UnsupportedFormat)?;

    let format_name = parsed.format.format_name.to_lowercase();
    let media_format = if format_name.contains("wav") {
        MediaFormat::WAV
    } else if format_name.contains("mp3") {
        MediaFormat::MP3
    } else if format_name.contains("m4a") || format_name.contains("mp4") || format_name.contains("mov") {
        let ext = path_ref
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if ext == "m4a" {
            MediaFormat::M4A
        } else if ext == "mp4" {
            MediaFormat::MP4
        } else if ext == "mov" {
            MediaFormat::MOV
        } else {
            MediaFormat::UNKNOWN
        }
    } else {
        MediaFormat::UNKNOWN
    };

    let sample_rate = audio_stream
        .sample_rate
        .as_ref()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let channels = audio_stream.channels.unwrap_or(0);

    // Prefer stream duration/bitrate, fallback to format duration/bitrate
    let duration_str = audio_stream
        .duration
        .as_ref()
        .or(parsed.format.duration.as_ref());
    let duration_seconds = duration_str
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let bit_rate_str = audio_stream
        .bit_rate
        .as_ref()
        .or(parsed.format.bit_rate.as_ref());
    let bitrate = bit_rate_str.and_then(|s| s.parse::<u32>().ok());

    let file_size_bytes = parsed.format.size.parse::<u64>().unwrap_or(0);

    Ok(MediaSource {
        path: path_str,
        filename,
        format: media_format,
        codec: audio_stream.codec_name.clone().unwrap_or_default(),
        inspection: MediaInspection {
            duration_seconds,
            sample_rate,
            channels,
            bitrate,
            file_size_bytes,
        },
        measurements: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ffprobe_output() {
        let json = r#"{
            "streams": [
                {
                    "codec_name": "pcm_s16le",
                    "codec_type": "audio",
                    "sample_rate": "48000",
                    "channels": 2,
                    "bit_rate": "1536000",
                    "duration": "3138.123"
                }
            ],
            "format": {
                "format_name": "wav",
                "size": "602519616",
                "duration": "3138.123",
                "bit_rate": "1536000"
            }
        }"#;

        let parsed: FfprobeOutput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.streams[0].codec_name.as_deref(), Some("pcm_s16le"));
        assert_eq!(parsed.format.format_name, "wav");
    }
}
