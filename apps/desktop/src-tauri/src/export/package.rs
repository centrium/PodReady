use crate::assessment::engine::Assessment;
use crate::error::AppError;
use crate::export::mp3::encode_publishing_mp3;
use crate::export::report::generate_json_report;
use crate::export::transcript::write_transcript_file;
use crate::export::types::{
    ExportOptions, ExportVerificationResult, PodReadyPackage, PublishingJsonReport,
    ReportActionItem, ReportTranscriptionInfo,
};
use crate::export::verification::verify_exported_mp3;
use crate::media::analysis::AudioMeasurements;
use crate::media::ffprobe::inspect_media;
use crate::transcription::transcribe_audio;
use std::path::{Path, PathBuf};

/// Creates the complete publishing package in the user's chosen destination directory.
/// All processing is strictly local, self-contained, and non-destructive.
pub fn create_publishing_package(
    input_audio_path: &str,
    source_original_path: &str,
    options: &ExportOptions,
    before_measurements: Option<AudioMeasurements>,
    before_assessment: Option<Assessment>,
    applied_actions: Vec<ReportActionItem>,
) -> Result<PodReadyPackage, AppError> {
    let started_at = std::time::Instant::now();

    let source_path = Path::new(source_original_path);
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("episode");

    let source_filename = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("episode")
        .to_string();

    // Create deterministic package directory: [destination]/[stem]_PodReady
    let dest_base = Path::new(&options.destination_directory);
    let package_dir_name = format!("{}_PodReady", stem);
    let package_dir_path: PathBuf = dest_base.join(&package_dir_name);

    std::fs::create_dir_all(&package_dir_path).map_err(|e| {
        AppError::SystemError(format!(
            "Failed to create package directory at {}: {}",
            package_dir_path.display(),
            e
        ))
    })?;

    let inspection = inspect_media(input_audio_path)?;
    let channels = inspection.inspection.channels;
    let sample_rate = inspection.inspection.sample_rate;

    let mut exported_audio = None;
    let mut artwork_embedded = false;
    let mut verification_result: Option<ExportVerificationResult> = None;
    let mut audio_target_path: Option<String> = None;

    // 1. Final Audio Export (MP3)
    if options.include_audio {
        let mp3_filename = format!("{}_ready.mp3", stem);
        let mp3_path = package_dir_path.join(&mp3_filename);
        let mp3_path_str = mp3_path.to_string_lossy().to_string();

        let outcome = encode_publishing_mp3(
            input_audio_path,
            &mp3_path_str,
            channels,
            sample_rate,
            options.metadata.as_ref(),
        )?;

        artwork_embedded = outcome.artwork_embedded;
        exported_audio = Some(outcome.file);
        audio_target_path = Some(mp3_path_str.clone());

        // 2. Post-Export Verification on the actual MP3
        let verified = verify_exported_mp3(&mp3_path_str)?;
        verification_result = Some(verified);
    }

    // 3. Local Whisper Speech-to-Text Transcription on the FINAL MP3 (or candidate if no MP3)
    let mut exported_transcript = None;
    let mut transcript_language = None;
    let mut transcript_error = None;
    let mut transcription_report = None;

    if options.include_transcript {
        // Transcribe the actual final MP3 audio delivered to the user
        let audio_for_transcription = audio_target_path
            .as_deref()
            .unwrap_or(input_audio_path);

        match transcribe_audio(audio_for_transcription, None) {
            Ok(transcript_res) => {
                transcript_language = transcript_res.language.clone();

                if !transcript_res.text.trim().is_empty() {
                    let txt_filename = format!("{}_transcript.txt", stem);
                    let txt_path = package_dir_path.join(&txt_filename);
                    let txt_path_str = txt_path.to_string_lossy().to_string();

                    match write_transcript_file(&transcript_res.text, &txt_path_str) {
                        Ok(txt_file) => {
                            transcription_report = Some(ReportTranscriptionInfo {
                                requested: true,
                                status: "SUCCESS".to_string(),
                                engine: Some("whisper.cpp".to_string()),
                                model: Some("large-v3-turbo".to_string()),
                                detected_language: transcript_language.clone(),
                                output_file: Some(txt_filename),
                                error: None,
                            });
                            exported_transcript = Some(txt_file);
                        }
                        Err(e) => {
                            log::error!("Failed to write transcript file: {}", e);
                            let err_msg = e.to_string();
                            transcript_error = Some(err_msg.clone());
                            transcription_report = Some(ReportTranscriptionInfo {
                                requested: true,
                                status: "FAILED".to_string(),
                                engine: Some("whisper.cpp".to_string()),
                                model: Some("large-v3-turbo".to_string()),
                                detected_language: transcript_language.clone(),
                                output_file: None,
                                error: Some(err_msg),
                            });
                        }
                    }
                } else {
                    // Empty speech recognition output
                    transcription_report = Some(ReportTranscriptionInfo {
                        requested: true,
                        status: "NO_SPEECH_DETECTED".to_string(),
                        engine: Some("whisper.cpp".to_string()),
                        model: Some("large-v3-turbo".to_string()),
                        detected_language: transcript_language.clone(),
                        output_file: None,
                        error: None,
                    });
                }
            }
            Err(err) => {
                // Transcription failure must not destroy a valid audio export
                log::warn!("Whisper transcription failed, audio remains valid: {}", err);
                let err_msg = err.to_string();
                transcript_error = Some(err_msg.clone());
                transcription_report = Some(ReportTranscriptionInfo {
                    requested: true,
                    status: "FAILED".to_string(),
                    engine: Some("whisper.cpp".to_string()),
                    model: Some("large-v3-turbo".to_string()),
                    detected_language: None,
                    output_file: None,
                    error: Some(err_msg),
                });
            }
        }
    }

    // 4. Verification Report Export
    let mut exported_report = None;
    let final_verification = match verification_result {
        Some(v) => v,
        None => {
            // Fallback verification from input audio if audio export was bypassed
            let verified = verify_exported_mp3(input_audio_path)?;
            verified
        }
    };

    let now_iso = {
        let duration = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        format!("{}-01-01T00:00:00Z (timestamp: {})", 1970 + secs / 31536000, secs)
    };

    if options.include_report {
        let report_filename = format!("{}_report.json", stem);
        let report_path = package_dir_path.join(&report_filename);
        let report_path_str = report_path.to_string_lossy().to_string();

        let report_data = PublishingJsonReport {
            podready_version: "1.0.0".to_string(),
            created_at: now_iso.clone(),
            package_name: package_dir_name.clone(),
            source_filename,
            metadata: options.metadata.clone(),
            actions_applied: applied_actions,
            before_measurements,
            before_assessment,
            final_mp3_measurements: final_verification.measurements.clone(),
            final_mp3_assessment: final_verification.assessment.clone(),
            verification_passed: final_verification.passed,
            transcription: transcription_report,
        };

        let report_file = generate_json_report(&report_path_str, &report_data)?;
        exported_report = Some(report_file);
    }

    let generation_duration_seconds = started_at.elapsed().as_secs_f64();

    Ok(PodReadyPackage {
        package_directory: package_dir_path.to_string_lossy().to_string(),
        package_name: package_dir_name,
        audio_file: exported_audio,
        transcript_file: exported_transcript,
        transcript_language,
        transcript_error,
        report_file: exported_report,
        metadata: options.metadata.clone(),
        artwork_embedded,
        verification_result: final_verification,
        generation_duration_seconds,
        created_at: now_iso,
    })
}
