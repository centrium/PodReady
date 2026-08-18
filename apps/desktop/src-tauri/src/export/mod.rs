pub mod mp3;
pub mod package;
pub mod report;
pub mod transcript;
pub mod types;
pub mod verification;

pub use package::create_publishing_package;
pub use types::*;

#[allow(unused_imports)]
pub use mp3::encode_publishing_mp3;
#[allow(unused_imports)]
pub use report::generate_json_report;
#[allow(unused_imports)]
pub use transcript::write_transcript_file;
#[allow(unused_imports)]
pub use verification::verify_exported_mp3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::engine::{assess_media, OverallStatus};
    use crate::fixplan::engine::FixActionType;
    use crate::media::analysis::analyse_audio;
    use crate::media::binaries::{ffmpeg_cmd, get_resources_dir};
    use crate::media::ffprobe::inspect_media;

    #[test]
    fn test_mp3_export_stereo_and_mono() {
        let temp_dir = std::env::temp_dir();
        let test_wav_stereo = temp_dir.join("test_export_stereo.wav");
        let test_mp3_stereo = temp_dir.join("test_export_stereo.mp3");
        let test_wav_mono = temp_dir.join("test_export_mono.wav");
        let test_mp3_mono = temp_dir.join("test_export_mono.mp3");

        // 1. Generate stereo test WAV
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav_stereo.to_str().unwrap(),
            ])
            .output();

        let original_stereo_bytes = std::fs::read(&test_wav_stereo).unwrap();

        // 2. Export Stereo MP3
        let outcome_stereo = encode_publishing_mp3(
            test_wav_stereo.to_str().unwrap(),
            test_mp3_stereo.to_str().unwrap(),
            2,
            44100,
            None,
        )
        .expect("Stereo MP3 export should succeed");

        assert!(test_mp3_stereo.exists());
        assert!(outcome_stereo.file.file_size_bytes > 0);
        assert_eq!(outcome_stereo.file.file_type, "audio");

        // Source WAV must be strictly untouched
        let after_stereo_bytes = std::fs::read(&test_wav_stereo).unwrap();
        assert_eq!(original_stereo_bytes, after_stereo_bytes);

        // Inspect exported stereo MP3
        let insp_stereo = inspect_media(test_mp3_stereo.to_str().unwrap()).unwrap();
        assert_eq!(insp_stereo.inspection.channels, 2);
        assert_eq!(insp_stereo.format, crate::media::ffprobe::MediaFormat::MP3);

        // 3. Generate mono test WAV
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=mono",
                test_wav_mono.to_str().unwrap(),
            ])
            .output();

        let outcome_mono = encode_publishing_mp3(
            test_wav_mono.to_str().unwrap(),
            test_mp3_mono.to_str().unwrap(),
            1,
            44100,
            None,
        )
        .expect("Mono MP3 export should succeed");

        assert!(test_mp3_mono.exists());
        assert!(outcome_mono.file.file_size_bytes > 0);

        let insp_mono = inspect_media(test_mp3_mono.to_str().unwrap()).unwrap();
        assert_eq!(insp_mono.inspection.channels, 1);

        // Clean up
        let _ = std::fs::remove_file(test_wav_stereo);
        let _ = std::fs::remove_file(test_mp3_stereo);
        let _ = std::fs::remove_file(test_wav_mono);
        let _ = std::fs::remove_file(test_mp3_mono);
    }

    #[test]
    fn test_metadata_and_artwork_embedding() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("test_meta.wav");
        let test_art = temp_dir.join("test_cover.jpg");
        let test_mp3 = temp_dir.join("test_meta.mp3");

        // Create test WAV
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        // Create test image (100x100 RGB image)
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=100x100:d=1.0",
                "-frames:v",
                "1",
                test_art.to_str().unwrap(),
            ])
            .output();

        let metadata = EpisodeMetadata {
            title: Some("Episode 42: The Future".to_string()),
            artist: Some("Tech Wave Show".to_string()),
            album: Some("Tech Wave 2026".to_string()),
            episode_number: Some("42".to_string()),
            year: Some("2026".to_string()),
            genre: Some("Podcast".to_string()),
            artwork_path: Some(test_art.to_str().unwrap().to_string()),
        };

        let outcome = encode_publishing_mp3(
            test_wav.to_str().unwrap(),
            test_mp3.to_str().unwrap(),
            2,
            44100,
            Some(&metadata),
        )
        .expect("MP3 export with metadata and artwork should succeed");

        assert!(outcome.artwork_embedded);
        assert!(test_mp3.exists());

        // Clean up
        let _ = std::fs::remove_file(test_wav);
        let _ = std::fs::remove_file(test_art);
        let _ = std::fs::remove_file(test_mp3);
    }

    #[test]
    fn test_transcript_file_generation() {
        let temp_dir = std::env::temp_dir();
        let txt_path = temp_dir.join("test_transcript.txt");
        let content = "Welcome back to the show. Today we're discussing PodReady publishing packages.";

        let res = write_transcript_file(content, txt_path.to_str().unwrap())
            .expect("Writing transcript should succeed");

        assert_eq!(res.file_type, "transcript");
        assert_eq!(res.filename, "test_transcript.txt");
        let read_back = std::fs::read_to_string(&txt_path).unwrap();
        assert_eq!(read_back, content);

        let _ = std::fs::remove_file(txt_path);
    }

    #[test]
    fn test_post_export_verification() {
        use crate::fixplan::engine::generate_fix_plan;
        use crate::media::processing::execute_fix_plan;

        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("test_verify.wav");
        let test_mp3 = temp_dir.join("test_verify.mp3");

        // Generate test WAV
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=3.0,volume=0.8,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let insp = inspect_media(test_wav.to_str().unwrap()).unwrap();
        let meas = analyse_audio(test_wav.to_str().unwrap(), insp.inspection.duration_seconds).unwrap();
        let assess = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);
        let plan = generate_fix_plan(&assess);

        // Process audio to candidate
        let proc_resp = execute_fix_plan(test_wav.to_str().unwrap(), &plan, Some(meas), Some(assess))
            .expect("Fix plan execution should succeed");

        // Export candidate to MP3
        let outcome = encode_publishing_mp3(
            &proc_resp.candidate_path,
            test_mp3.to_str().unwrap(),
            2,
            44100,
            None,
        )
        .expect("MP3 export should succeed");

        assert!(outcome.file.file_size_bytes > 0);

        // Verify exported MP3 independently
        let verified = verify_exported_mp3(test_mp3.to_str().unwrap())
            .expect("Post-export verification should succeed");

        assert!(verified.measurements.integrated_loudness_lufs.is_some());
        assert!(verified.measurements.true_peak_dbtp.is_some());
        assert_eq!(verified.assessment.overall_status, OverallStatus::Ready);
        assert!(verified.passed);

        let _ = std::fs::remove_file(test_wav);
        let _ = std::fs::remove_file(proc_resp.candidate_path);
        let _ = std::fs::remove_file(test_mp3);
    }

    #[test]
    fn test_publishing_package_assembly_with_real_transcription() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let fixture_path = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        let temp_dir = std::env::temp_dir().join("podready_test_pkg_real_stt");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_path_str = fixture_path.to_string_lossy().to_string();
        let source_bytes = std::fs::read(&fixture_path).expect("fixture exists");

        let inspection = inspect_media(&source_path_str).unwrap();
        let meas = analyse_audio(&source_path_str, inspection.inspection.duration_seconds).unwrap();
        let assess = assess_media(
            &inspection.inspection,
            Some(&meas),
            &inspection.format,
            &inspection.codec,
        );

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: true,
            include_report: true,
            metadata: Some(EpisodeMetadata {
                title: Some("JFK Speech Episode".to_string()),
                artist: Some("Historical Archives".to_string()),
                album: Some("Inaugural Addresses".to_string()),
                episode_number: Some("1".to_string()),
                year: Some("1961".to_string()),
                genre: Some("Speech".to_string()),
                artwork_path: None,
            }),
        };

        let applied = vec![ReportActionItem {
            action_type: FixActionType::LoudnessAdjustment,
            title: "Adjust loudness".to_string(),
            description: "Normalized loudness to -16.0 LUFS".to_string(),
            success: true,
        }];

        let package = create_publishing_package(
            &source_path_str,
            &source_path_str,
            &options,
            Some(meas),
            Some(assess),
            applied,
        )
        .expect("Package assembly with real Whisper transcription should succeed");

        // Verify generation duration measurement
        assert!(package.generation_duration_seconds > 0.0, "Package generation duration must be greater than 0");

        // Verify package structure
        assert!(package.package_name.contains("spoken_jfk_16k_PodReady"));
        assert!(package.audio_file.is_some());
        assert!(package.transcript_file.is_some());
        assert!(package.report_file.is_some());

        let audio = package.audio_file.unwrap();
        let transcript = package.transcript_file.unwrap();
        let report = package.report_file.unwrap();

        assert_eq!(audio.filename, "spoken_jfk_16k_ready.mp3");
        assert_eq!(transcript.filename, "spoken_jfk_16k_transcript.txt");
        assert_eq!(report.filename, "spoken_jfk_16k_report.json");

        assert!(std::path::Path::new(&audio.path).exists());
        assert!(std::path::Path::new(&transcript.path).exists());
        assert!(std::path::Path::new(&report.path).exists());

        // Verify transcript content contains recognized words and NOT legacy placeholder text
        let transcript_content = std::fs::read_to_string(&transcript.path).unwrap();
        assert!(!transcript_content.trim().is_empty());
        assert!(
            !transcript_content.contains("Spoken transcript for"),
            "Transcript MUST NOT contain legacy placeholder prefix"
        );
        assert!(
            !transcript_content.contains("Extracted by PodReady."),
            "Transcript MUST NOT contain legacy placeholder suffix"
        );

        let lower = transcript_content.to_lowercase();
        assert!(
            lower.contains("fellow americans") || lower.contains("country"),
            "Expected speech recognition text in transcript file, got: {}",
            transcript_content
        );

        // Verify report content
        let report_content = std::fs::read_to_string(&report.path).unwrap();
        assert!(report_content.contains("spoken_jfk_16k"));
        assert!(report_content.contains("finalMp3Measurements"));
        assert!(report_content.contains("verificationPassed"));
        assert!(report_content.contains("whisper.cpp"));
        assert!(report_content.contains("SUCCESS"));

        // Source file must remain identical and untouched
        let after_source_bytes = std::fs::read(&fixture_path).unwrap();
        assert_eq!(source_bytes, after_source_bytes);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_regression_production_export_has_no_placeholder() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let fixture_path = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        let temp_dir = std::env::temp_dir().join("podready_test_regression_placeholder");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_path_str = fixture_path.to_string_lossy().to_string();

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: true,
            include_report: true,
            metadata: None,
        };

        let package = create_publishing_package(
            &source_path_str,
            &source_path_str,
            &options,
            None,
            None,
            vec![],
        )
        .expect("Production package export should succeed");

        assert!(package.generation_duration_seconds > 0.0);

        let transcript_file = package.transcript_file.expect("Transcript file must exist");
        let content = std::fs::read_to_string(&transcript_file.path).unwrap();

        // Must NOT contain old placeholder template
        assert!(!content.contains("Spoken transcript for"));
        assert!(!content.contains("Extracted by PodReady."));

        // Must contain genuine transcribed speech
        let lower = content.to_lowercase();
        assert!(lower.contains("fellow americans") || lower.contains("country"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_package_generation_duration_timing_and_serialization() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let fixture_path = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        let temp_dir = std::env::temp_dir().join("podready_test_duration_ser");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_path_str = fixture_path.to_string_lossy().to_string();

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: false,
            include_report: true,
            metadata: None,
        };

        let package = create_publishing_package(
            &source_path_str,
            &source_path_str,
            &options,
            None,
            None,
            vec![],
        )
        .expect("Package export should succeed");

        assert!(package.generation_duration_seconds > 0.0);

        let json = serde_json::to_string(&package).expect("PodReadyPackage serializes to JSON");
        assert!(json.contains("\"generationDurationSeconds\":"), "JSON must have camelCase generationDurationSeconds field");

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let dur = parsed["generationDurationSeconds"].as_f64().unwrap();
        assert!(dur > 0.0);
        assert!((dur - package.generation_duration_seconds).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_audio_survives_transcription_failure() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let temp_dir = std::env::temp_dir().join("podready_test_stt_fail_survive");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_wav = temp_dir.join("test_survive.wav");

        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.5,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                source_wav.to_str().unwrap(),
            ])
            .output();

        let inspection = inspect_media(source_wav.to_str().unwrap()).unwrap();
        let meas = analyse_audio(source_wav.to_str().unwrap(), inspection.inspection.duration_seconds).unwrap();
        let assess = assess_media(
            &inspection.inspection,
            Some(&meas),
            &inspection.format,
            &inspection.codec,
        );

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: true,
            include_report: true,
            metadata: None,
        };

        // Point PODREADY_RESOURCES_DIR to a directory with binaries but no models
        let dummy_res_dir = temp_dir.join("dummy_res");
        let _ = std::fs::create_dir_all(dummy_res_dir.join("bin"));
        let real_res = get_resources_dir().unwrap();
        let _ = std::fs::copy(real_res.join("bin").join("ffmpeg"), dummy_res_dir.join("bin").join("ffmpeg"));
        let _ = std::fs::copy(real_res.join("bin").join("ffprobe"), dummy_res_dir.join("bin").join("ffprobe"));
        let _ = std::fs::copy(real_res.join("bin").join("whisper-cli"), dummy_res_dir.join("bin").join("whisper-cli"));

        std::env::set_var("PODREADY_RESOURCES_DIR", dummy_res_dir.to_str().unwrap());

        let package = create_publishing_package(
            source_wav.to_str().unwrap(),
            source_wav.to_str().unwrap(),
            &options,
            Some(meas),
            Some(assess),
            vec![],
        )
        .expect("Package assembly should not crash if transcription fails");

        // Clean up env override
        std::env::remove_var("PODREADY_RESOURCES_DIR");

        // Audio and report must still exist and succeed
        assert!(package.audio_file.is_some(), "Audio file must survive transcript failure");
        assert!(package.report_file.is_some(), "Report file must exist");
        assert!(package.transcript_error.is_some(), "Transcript error should be recorded when model is missing");
        assert!(package.generation_duration_seconds > 0.0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_mcdonalds_real_file_transcription() {
        let mcd_path = std::path::Path::new("/Users/matt/Desktop/McDonalds_LNG_061019.wav");
        if mcd_path.exists() {
            let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();
            let temp_dir = std::env::temp_dir().join("podready_test_mcdonalds");
            let _ = std::fs::create_dir_all(&temp_dir);

            let options = ExportOptions {
                destination_directory: temp_dir.to_string_lossy().to_string(),
                include_audio: true,
                include_transcript: true,
                include_report: true,
                metadata: None,
            };

            let package = create_publishing_package(
                mcd_path.to_str().unwrap(),
                mcd_path.to_str().unwrap(),
                &options,
                None,
                None,
                vec![],
            )
            .expect("Exporting McDonalds file should succeed");

            assert!(package.generation_duration_seconds > 0.0);
            println!("Package created in {:.1} seconds (exact: {:.4}s)", package.generation_duration_seconds, package.generation_duration_seconds);

            let transcript_file = package.transcript_file.expect("Transcript file must exist");
            let content = std::fs::read_to_string(&transcript_file.path).unwrap();

            println!("=== MCDONALDS TRANSCRIPT OUTPUT ===");
            println!("{}", content);
            println!("===================================");

            assert!(!content.contains("Spoken transcript for"));
            assert!(!content.contains("Extracted by PodReady."));
            assert!(!content.trim().is_empty());

            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }

    #[test]
    fn test_first_vs_second_export_breakdown() {
        let mcd_path = std::path::Path::new("/Users/matt/Desktop/McDonalds_LNG_061019.wav");
        let res_dir = get_resources_dir().expect("resources dir exists");
        let fallback_fixture = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        let test_audio_path = if mcd_path.exists() {
            mcd_path.to_path_buf()
        } else if fallback_fixture.exists() {
            fallback_fixture
        } else {
            return;
        };

        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();
        let temp_dir = std::env::temp_dir().join("podready_test_export_comparison");
        let _ = std::fs::create_dir_all(&temp_dir);

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: true,
            include_report: true,
            metadata: None,
        };

        // Export 1 (Cold)
        let t1_start = std::time::Instant::now();
        let pkg1 = create_publishing_package(
            test_audio_path.to_str().unwrap(),
            test_audio_path.to_str().unwrap(),
            &options,
            None,
            None,
            vec![],
        )
        .expect("Export 1 must succeed");
        let t1_total = t1_start.elapsed().as_secs_f64();

        // Export 2 (Warm)
        let t2_start = std::time::Instant::now();
        let pkg2 = create_publishing_package(
            test_audio_path.to_str().unwrap(),
            test_audio_path.to_str().unwrap(),
            &options,
            None,
            None,
            vec![],
        )
        .expect("Export 2 must succeed");
        let t2_total = t2_start.elapsed().as_secs_f64();

        println!("\n================ EXPORT 1 vs EXPORT 2 COMPARISON ================");
        println!("Export 1 (Cold) - Total: {:.2}s (Backend recorded: {:.2}s)", t1_total, pkg1.generation_duration_seconds);
        println!("Export 2 (Warm) - Total: {:.2}s (Backend recorded: {:.2}s)", t2_total, pkg2.generation_duration_seconds);
        println!("Difference:     {:.2}s", t1_total - t2_total);
        println!("=================================================================\n");

        // Both export 1 and export 2 must be fast (< 15 seconds) because no 487MB checksum runs on the critical path
        assert!(pkg1.generation_duration_seconds < 15.0, "Export 1 should not perform synchronous 487MB checksum");
        assert!(pkg2.generation_duration_seconds < 15.0, "Export 2 should be fast");
        assert!((t1_total - t2_total).abs() < 5.0, "Export 1 and Export 2 should have similar fast execution time");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_corrupt_model_rejected_cleanly() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let temp_dir = std::env::temp_dir().join("podready_test_corrupt_model");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_wav = temp_dir.join("test_corrupt.wav");

        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                source_wav.to_str().unwrap(),
            ])
            .output();

        // Create dummy resources with a tiny corrupt model (less than 1MB)
        let dummy_res_dir = temp_dir.join("dummy_corrupt_res");
        let _ = std::fs::create_dir_all(dummy_res_dir.join("bin"));
        let _ = std::fs::create_dir_all(dummy_res_dir.join("models"));

        let real_res = get_resources_dir().unwrap();
        let _ = std::fs::copy(real_res.join("bin").join("ffmpeg"), dummy_res_dir.join("bin").join("ffmpeg"));
        let _ = std::fs::copy(real_res.join("bin").join("ffprobe"), dummy_res_dir.join("bin").join("ffprobe"));
        let _ = std::fs::copy(real_res.join("bin").join("whisper-cli"), dummy_res_dir.join("bin").join("whisper-cli"));

        // Write corrupt 50-byte model
        std::fs::write(dummy_res_dir.join("models").join(crate::media::binaries::DEFAULT_MODEL_FILENAME), b"corrupt model file content").unwrap();

        std::env::set_var("PODREADY_RESOURCES_DIR", dummy_res_dir.to_str().unwrap());

        let options = ExportOptions {
            destination_directory: temp_dir.to_string_lossy().to_string(),
            include_audio: true,
            include_transcript: true,
            include_report: true,
            metadata: None,
        };

        let package = create_publishing_package(
            source_wav.to_str().unwrap(),
            source_wav.to_str().unwrap(),
            &options,
            None,
            None,
            vec![],
        )
        .expect("Package assembly should not panic on invalid model");

        std::env::remove_var("PODREADY_RESOURCES_DIR");

        assert!(package.audio_file.is_some(), "Audio file must still succeed");
        assert!(package.transcript_file.is_none(), "Transcript file must not be created");
        assert!(package.transcript_error.is_some(), "Transcript error must be recorded");

        let err_msg = package.transcript_error.unwrap();
        assert!(err_msg.contains("incomplete") || err_msg.contains("model"), "Expected clean error message, got: {}", err_msg);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
