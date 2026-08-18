use crate::error::AppError;
use crate::export::types::{EpisodeMetadata, ExportedFile};
use crate::media::binaries::ffmpeg_cmd;
use std::path::Path;

pub struct Mp3ExportOutcome {
    pub file: ExportedFile,
    pub artwork_embedded: bool,
}

/// Encodes publishing-ready MP3 with opinionated podcast defaults (192kbps stereo / 128kbps mono),
/// ID3v2 metadata, and optional cover artwork. Source files are strictly preserved.
pub fn encode_publishing_mp3(
    source_audio_path: &str,
    output_mp3_path: &str,
    channels: u32,
    sample_rate: u32,
    metadata: Option<&EpisodeMetadata>,
) -> Result<Mp3ExportOutcome, AppError> {
    let source_path = Path::new(source_audio_path);
    if !source_path.exists() {
        return Err(AppError::ProcessingFailed(format!(
            "Input audio file does not exist: {}",
            source_audio_path
        )));
    }

    // Determine bitrate based on channel count (192k stereo, 128k mono)
    let bitrate = if channels <= 1 { "128k" } else { "192k" };
    let sample_rate_str = sample_rate.to_string();

    // Check artwork availability
    let mut artwork_valid = false;
    let mut artwork_path_str: Option<&str> = None;

    if let Some(meta) = metadata {
        if let Some(art_path) = &meta.artwork_path {
            if !art_path.trim().is_empty() {
                let p = Path::new(art_path);
                if p.exists() && p.is_file() {
                    artwork_valid = true;
                    artwork_path_str = Some(art_path.as_str());
                }
            }
        }
    }

    let mut cmd = ffmpeg_cmd()?;
    cmd.args(["-y", "-hide_banner", "-nostats", "-i", source_audio_path]);

    if artwork_valid {
        if let Some(art_p) = artwork_path_str {
            cmd.args(["-i", art_p]);
            cmd.args(["-map", "0:a", "-map", "1:0"]);
        }
    }

    cmd.args(["-c:a", "libmp3lame", "-b:a", bitrate, "-ar", &sample_rate_str]);

    if artwork_valid {
        cmd.args([
            "-c:v",
            "copy",
            "-metadata:s:v",
            "title=Album cover",
            "-metadata:s:v",
            "comment=Cover (front)",
        ]);
    }

    // Embed ID3v2.3 tags
    cmd.args(["-id3v2_version", "3"]);

    if let Some(meta) = metadata {
        if let Some(title) = &meta.title {
            if !title.trim().is_empty() {
                cmd.args(["-metadata", &format!("title={}", title)]);
            }
        }
        if let Some(artist) = &meta.artist {
            if !artist.trim().is_empty() {
                cmd.args(["-metadata", &format!("artist={}", artist)]);
            }
        }
        if let Some(album) = &meta.album {
            if !album.trim().is_empty() {
                cmd.args(["-metadata", &format!("album={}", album)]);
            }
        }
        if let Some(ep_num) = &meta.episode_number {
            if !ep_num.trim().is_empty() {
                cmd.args(["-metadata", &format!("track={}", ep_num)]);
            }
        }
        if let Some(year) = &meta.year {
            if !year.trim().is_empty() {
                cmd.args(["-metadata", &format!("date={}", year)]);
            }
        }
        if let Some(genre) = &meta.genre {
            if !genre.trim().is_empty() {
                cmd.args(["-metadata", &format!("genre={}", genre)]);
            }
        } else {
            cmd.args(["-metadata", "genre=Podcast"]);
        }
    }

    cmd.arg(output_mp3_path);

    let output = cmd
        .output()
        .map_err(|e| AppError::ProcessingFailed(format!("Failed to execute FFmpeg MP3 encoding: {}", e)))?;

    if !output.status.success() {
        let err_text = String::from_utf8_lossy(&output.stderr);
        // If it failed because of artwork, retry once without artwork gracefully
        if artwork_valid {
            log::warn!("MP3 encoding failed with artwork, falling back to audio only: {}", err_text);
            return encode_publishing_mp3(
                source_audio_path,
                output_mp3_path,
                channels,
                sample_rate,
                metadata.map(|m| {
                    let mut without_art = m.clone();
                    without_art.artwork_path = None;
                    without_art
                }).as_ref(),
            );
        }

        return Err(AppError::ProcessingFailed(
            "We couldn't encode the publishing MP3 file.".to_string(),
        ));
    }

    let out_file_path = Path::new(output_mp3_path);
    let file_size = std::fs::metadata(out_file_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let filename = out_file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("episode.mp3")
        .to_string();

    Ok(Mp3ExportOutcome {
        file: ExportedFile {
            path: output_mp3_path.to_string(),
            filename,
            file_size_bytes: file_size,
            file_type: "audio".to_string(),
        },
        artwork_embedded: artwork_valid,
    })
}
