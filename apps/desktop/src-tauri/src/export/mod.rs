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
    use crate::media::ffprobe::inspect_media;
    use std::process::Command;

    #[test]
    fn test_mp3_export_stereo_and_mono() {
        let temp_dir = std::env::temp_dir();
        let test_wav_stereo = temp_dir.join("test_export_stereo.wav");
        let test_mp3_stereo = temp_dir.join("test_export_stereo.mp3");
        let test_wav_mono = temp_dir.join("test_export_mono.wav");
        let test_mp3_mono = temp_dir.join("test_export_mono.mp3");

        // 1. Generate stereo test WAV
        let _ = Command::new("ffmpeg")
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
        let _ = Command::new("ffmpeg")
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
        let _ = Command::new("ffmpeg")
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
        let _ = Command::new("ffmpeg")
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

        // Test with missing artwork gracefully
        let test_mp3_no_art = temp_dir.join("test_meta_no_art.mp3");
        let metadata_missing_art = EpisodeMetadata {
            title: Some("Audio Only".to_string()),
            artist: Some("Podcaster".to_string()),
            album: None,
            episode_number: None,
            year: None,
            genre: None,
            artwork_path: Some("/nonexistent/cover.jpg".to_string()),
        };

        let outcome_no_art = encode_publishing_mp3(
            test_wav.to_str().unwrap(),
            test_mp3_no_art.to_str().unwrap(),
            2,
            44100,
            Some(&metadata_missing_art),
        )
        .expect("MP3 export should succeed even if artwork path is missing");

        assert!(!outcome_no_art.artwork_embedded);
        assert!(test_mp3_no_art.exists());

        // Clean up
        let _ = std::fs::remove_file(test_wav);
        let _ = std::fs::remove_file(test_art);
        let _ = std::fs::remove_file(test_mp3);
        let _ = std::fs::remove_file(test_mp3_no_art);
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
        let _ = Command::new("ffmpeg")
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
    fn test_publishing_package_assembly_e2e() {
        let temp_dir = std::env::temp_dir().join("podready_test_export_dest");
        let _ = std::fs::create_dir_all(&temp_dir);

        let source_wav = temp_dir.join("McDonalds_LNG_061019.wav");

        // Generate healthy audio file
        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=2.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                source_wav.to_str().unwrap(),
            ])
            .output();

        let source_bytes = std::fs::read(&source_wav).unwrap();

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
            metadata: Some(EpisodeMetadata {
                title: Some("McDonalds LNG Episode".to_string()),
                artist: Some("McDonalds LNG".to_string()),
                album: Some("Season 2".to_string()),
                episode_number: Some("19".to_string()),
                year: Some("2026".to_string()),
                genre: Some("Podcast".to_string()),
                artwork_path: None,
            }),
            transcript_text: Some("Spoken transcript content for McDonald's LNG.".to_string()),
        };

        let applied = vec![ReportActionItem {
            action_type: FixActionType::LoudnessAdjustment,
            title: "Adjust loudness".to_string(),
            description: "Normalized loudness to -16.0 LUFS".to_string(),
            success: true,
        }];

        let package = create_publishing_package(
            source_wav.to_str().unwrap(),
            source_wav.to_str().unwrap(),
            &options,
            Some(meas),
            Some(assess),
            applied,
        )
        .expect("Package assembly should succeed");

        // Verify package structure
        assert!(package.package_name.contains("McDonalds_LNG_061019_PodReady"));
        assert!(package.audio_file.is_some());
        assert!(package.transcript_file.is_some());
        assert!(package.report_file.is_some());

        let audio = package.audio_file.unwrap();
        let transcript = package.transcript_file.unwrap();
        let report = package.report_file.unwrap();

        assert_eq!(audio.filename, "McDonalds_LNG_061019_ready.mp3");
        assert_eq!(transcript.filename, "McDonalds_LNG_061019_transcript.txt");
        assert_eq!(report.filename, "McDonalds_LNG_061019_report.json");

        assert!(std::path::Path::new(&audio.path).exists());
        assert!(std::path::Path::new(&transcript.path).exists());
        assert!(std::path::Path::new(&report.path).exists());

        // Verify report content
        let report_content = std::fs::read_to_string(&report.path).unwrap();
        assert!(report_content.contains("McDonalds_LNG_061019"));
        assert!(report_content.contains("finalMp3Measurements"));
        assert!(report_content.contains("verificationPassed"));

        // Source file must remain identical and untouched
        let after_source_bytes = std::fs::read(&source_wav).unwrap();
        assert_eq!(source_bytes, after_source_bytes);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
