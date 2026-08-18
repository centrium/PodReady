use crate::error::AppError;
use crate::media::binaries::whisper_cmd;
use crate::transcription::types::{TranscriptResult, TranscriptSegment};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonOutput {
    system_info: Option<String>,
    model: Option<WhisperJsonModel>,
    params: Option<serde_json::Value>,
    result: Option<WhisperJsonResult>,
    transcription: Option<Vec<WhisperJsonSegment>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonModel {
    #[serde(rename = "type")]
    model_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonResult {
    language: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonSegment {
    timestamps: Option<WhisperJsonTimestamps>,
    offsets: Option<WhisperJsonOffsets>,
    text: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonTimestamps {
    from: String,
    to: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WhisperJsonOffsets {
    from: i64,
    to: i64,
}

#[derive(Debug, Clone, Default)]
pub struct WhisperExecutionTimings {
    pub process_duration_seconds: f64,
    #[allow(dead_code)]
    pub load_seconds: f64,
    pub inference_seconds: f64,
    pub output_parse_seconds: f64,
}

#[allow(dead_code)]
/// Runs local Whisper inference on a 16kHz 16-bit mono WAV file using the bundled runtime and model.
pub fn run_whisper_inference(
    wav_16k_path: &Path,
    model_path: &Path,
) -> Result<TranscriptResult, AppError> {
    let (res, _) = run_whisper_inference_with_timings(wav_16k_path, model_path)?;
    Ok(res)
}

/// Runs local Whisper inference with detailed execution timings.
pub fn run_whisper_inference_with_timings(
    wav_16k_path: &Path,
    model_path: &Path,
) -> Result<(TranscriptResult, WhisperExecutionTimings), AppError> {
    if !wav_16k_path.exists() {
        return Err(AppError::SystemError(format!(
            "Audio source file for transcription does not exist: {}",
            wav_16k_path.display()
        )));
    }

    if !model_path.exists() {
        return Err(AppError::SystemError(
            "Transcription isn't available because PodReady's speech model could not be loaded."
                .to_string(),
        ));
    }

    let temp_dir = std::env::temp_dir();
    let unique_id = format!(
        "podready_trans_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let output_prefix = temp_dir.join(&unique_id);
    let output_prefix_str = output_prefix.to_string_lossy().to_string();

    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get().min(4).to_string())
        .unwrap_or_else(|_| "4".to_string());

    let mut cmd = whisper_cmd()?;
    cmd.args([
        "-m",
        &model_path.to_string_lossy(),
        "-f",
        &wav_16k_path.to_string_lossy(),
        "-oj",
        "-otxt",
        "-of",
        &output_prefix_str,
        "-l",
        "auto",
        "-fa",
        "-bs",
        "1",
        "-bo",
        "1",
        "-t",
        &thread_count,
        "-np",
    ]);

    let process_start = std::time::Instant::now();
    let output = cmd.output().map_err(|e| {
        AppError::SystemError(format!(
            "Failed to execute local Whisper speech recognizer: {}",
            e
        ))
    })?;
    let process_duration = process_start.elapsed().as_secs_f64();

    let json_path = PathBuf::from(format!("{}.json", output_prefix_str));
    let txt_path = PathBuf::from(format!("{}.txt", output_prefix_str));

    if !output.status.success() && !json_path.exists() && !txt_path.exists() {
        let stderr_err = String::from_utf8_lossy(&output.stderr);
        log::error!("Whisper transcription failed: {}", stderr_err);
        return Err(AppError::SystemError(
            "Speech recognition failed to process this episode.".to_string(),
        ));
    }

    let parse_start = std::time::Instant::now();

    // Try parsing structured JSON output
    let mut detected_language = None;
    let mut segments = Vec::new();
    let mut full_text = String::new();

    if json_path.exists() {
        if let Ok(json_content) = std::fs::read_to_string(&json_path) {
            if let Ok(parsed) = serde_json::from_str::<WhisperJsonOutput>(&json_content) {
                if let Some(res) = parsed.result {
                    detected_language = res.language;
                }
                if let Some(trans_segs) = parsed.transcription {
                    for seg in trans_segs {
                        let start_sec = seg
                            .offsets
                            .as_ref()
                            .map(|o| o.from as f64 / 1000.0)
                            .unwrap_or(0.0);
                        let end_sec = seg
                            .offsets
                            .as_ref()
                            .map(|o| o.to as f64 / 1000.0)
                            .unwrap_or(0.0);
                        let text = seg.text.trim().to_string();
                        if !text.is_empty() {
                            segments.push(TranscriptSegment {
                                start_sec,
                                end_sec,
                                text,
                            });
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&json_path);
    }

    // If text file was generated, read clean plain text
    if txt_path.exists() {
        if let Ok(txt_content) = std::fs::read_to_string(&txt_path) {
            full_text = clean_transcript_text(&txt_content);
        }
        let _ = std::fs::remove_file(&txt_path);
    }

    // Fallback: build full text from segments if txt was missing or empty
    if full_text.trim().is_empty() && !segments.is_empty() {
        full_text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<&str>>()
            .join(" ");
    }

    let parse_duration = parse_start.elapsed().as_secs_f64();
    let duration_seconds = segments.last().map(|s| s.end_sec);

    let timings = WhisperExecutionTimings {
        process_duration_seconds: process_duration,
        load_seconds: 0.5, // typical Metal model load
        inference_seconds: (process_duration - 0.5).max(0.0),
        output_parse_seconds: parse_duration,
    };

    Ok((
        TranscriptResult {
            text: full_text,
            language: detected_language,
            duration_seconds,
            segments,
        },
        timings,
    ))
}

/// Cleans plain text output from speech recognition into natural, readable paragraphs.
pub fn clean_transcript_text(raw_text: &str) -> String {
    let mut lines = Vec::new();
    for line in raw_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed);
        }
    }
    lines.join("\n\n")
}
