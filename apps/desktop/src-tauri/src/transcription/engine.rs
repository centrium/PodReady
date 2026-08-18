use crate::error::AppError;
use crate::media::binaries::{ffmpeg_cmd, resolve_default_model};
use crate::media::ffprobe::inspect_media;
use crate::transcription::types::{TranscriptResult, TranscriptionBenchmark};
use crate::transcription::whisper::run_whisper_inference_with_timings;
use std::path::{Path, PathBuf};

/// Transcribes any audio file (WAV, MP3, M4A, etc.) using the bundled local Whisper engine.
pub fn transcribe_audio(
    audio_path: &str,
    model_override: Option<&Path>,
) -> Result<TranscriptResult, AppError> {
    let (res, _) = transcribe_audio_with_benchmark(audio_path, model_override)?;
    Ok(res)
}

/// Transcribes audio and returns detailed stage-by-stage benchmark metrics.
pub fn transcribe_audio_with_benchmark(
    audio_path: &str,
    model_override: Option<&Path>,
) -> Result<(TranscriptResult, TranscriptionBenchmark), AppError> {
    let total_start = std::time::Instant::now();

    let source_path = Path::new(audio_path);
    if !source_path.exists() {
        return Err(AppError::SystemError(format!(
            "Audio file does not exist: {}",
            audio_path
        )));
    }

    // Inspect duration
    let duration_seconds = inspect_media(audio_path)
        .map(|m| m.inspection.duration_seconds)
        .unwrap_or(0.0);

    // 1. Resolve speech model
    let model_start = std::time::Instant::now();
    let model_path = match model_override {
        Some(p) => p.to_path_buf(),
        None => resolve_default_model()?,
    };
    let model_init_seconds = model_start.elapsed().as_secs_f64();

    let model_filename = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown_model")
        .to_string();

    let model_size_bytes = std::fs::metadata(&model_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // 2. Prepare 16kHz 16-bit mono PCM WAV for Whisper
    let prep_start = std::time::Instant::now();
    let (wav_path, is_temp) = prepare_16k_mono_wav(audio_path)?;
    let prep_seconds = prep_start.elapsed().as_secs_f64();

    // 3. Run speech recognition inference with timings
    let (result, whisper_timings) = run_whisper_inference_with_timings(&wav_path, &model_path)?;

    // 4. Clean up temporary WAV if created
    if is_temp && wav_path.exists() {
        let _ = std::fs::remove_file(&wav_path);
    }

    let total_seconds = total_start.elapsed().as_secs_f64();
    let audio_dur = if duration_seconds > 0.0 {
        duration_seconds
    } else {
        result.duration_seconds.unwrap_or(1.0)
    };

    let real_time_factor = if audio_dur > 0.0 {
        total_seconds / audio_dur
    } else {
        0.0
    };

    let benchmark = TranscriptionBenchmark {
        audio_duration_seconds: audio_dur,
        prep_seconds,
        runtime_startup_seconds: (whisper_timings.process_duration_seconds - whisper_timings.inference_seconds).max(0.0),
        model_init_seconds,
        inference_seconds: whisper_timings.inference_seconds,
        output_processing_seconds: whisper_timings.output_parse_seconds,
        total_seconds,
        real_time_factor,
        model_name: model_filename,
        model_size_bytes,
        detected_language: result.language.clone(),
    };

    Ok((result, benchmark))
}

/// Converts any input audio file to a standard 16kHz mono 16-bit PCM WAV required by Whisper.
fn prepare_16k_mono_wav(audio_path: &str) -> Result<(PathBuf, bool), AppError> {
    let temp_dir = std::env::temp_dir();
    let temp_wav = temp_dir.join(format!(
        "podready_16k_{}_{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    let mut cmd = ffmpeg_cmd()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-nostats",
        "-i",
        audio_path,
        "-ar",
        "16000",
        "-ac",
        "1",
        "-c:a",
        "pcm_s16le",
        &temp_wav.to_string_lossy(),
    ]);

    let output = cmd.output().map_err(|e| {
        AppError::SystemError(format!(
            "Failed to execute FFmpeg for transcription audio preparation: {}",
            e
        ))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("FFmpeg audio conversion to 16k mono failed: {}", stderr);
        return Err(AppError::SystemError(
            "Failed to prepare audio stream for speech recognition.".to_string(),
        ));
    }

    Ok((temp_wav, true))
}
