use std::fs::File;
use std::io::Write;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

use crate::assessment::engine::{assess_media, OverallStatus};
use crate::batch::{BatchEpisode, BatchEpisodeStatus};

use crate::catalogue::models::{
    AddEpisodeStatus, SourceAvailability,
};
use crate::catalogue::repository::CatalogueRepository;
use crate::catalogue::service::CatalogueService;
use crate::media::analysis::{AudioMeasurements, ClippingAnalysis, ClippingEvidence};
use crate::media::ffprobe::{MediaFormat, MediaInspection, MediaSource};

fn create_test_media_source(path: &str, duration: f64, lufs: f64, dbtp: f64) -> MediaSource {
    let inspection = MediaInspection {
        duration_seconds: duration,
        sample_rate: 44100,
        channels: 2,
        bitrate: Some(192000),
        file_size_bytes: 1024 * 100,
    };
    let measurements = AudioMeasurements {
        integrated_loudness_lufs: Some(lufs),
        true_peak_dbtp: Some(dbtp),
        leading_silence_seconds: 0.5,
        trailing_silence_seconds: 1.0,
        clipping: ClippingAnalysis {
            sample_peak_dbfs: Some(-1.0),
            samples_at_ceiling: 0,
            flat_factor: 0.0,
            evidence: ClippingEvidence::NONE,
        },
    };
    let assessment = assess_media(
        &inspection,
        Some(&measurements),
        &MediaFormat::WAV,
        "pcm_s16le",
    );

    MediaSource {
        path: path.to_string(),
        filename: std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        format: MediaFormat::WAV,
        codec: "pcm_s16le".to_string(),
        inspection,
        measurements: Some(measurements),
        assessment: Some(assessment),
    }
}


fn file_sha256(path: &std::path::Path) -> String {
    let mut file = File::open(path).expect("Failed to open file for sha256");
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).expect("Failed to hash file");
    format!("{:x}", hasher.finalize())
}

#[test]
fn test_fresh_database_creation_and_migrations() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let shows = service.get_shows().expect("Failed to get shows");
    assert_eq!(shows.len(), 0);
}

#[test]
fn test_database_persistence_across_reopen() {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_catalogue.db");

    // Phase 1: Create show and episode, then close
    {
        let repo = CatalogueRepository::open_file(&db_path).expect("Failed to open DB");
        let service = CatalogueService::new(repo);
        let show = service
            .create_show("Persisted Podcast", Some("Testing DB reopen"))
            .expect("Failed to create show");

        let media = create_test_media_source("/tmp/episode1.wav", 120.0, -16.0, -1.5);
        let outcome = service
            .add_media_source_to_show(&show.id, &media)
            .expect("Failed to add episode");
        assert_eq!(outcome.status, AddEpisodeStatus::Added);
    }

    // Phase 2: Re-open database from disk and verify data is intact
    {
        let repo = CatalogueRepository::open_file(&db_path).expect("Failed to reopen DB");
        let service = CatalogueService::new(repo);

        let shows = service.get_shows().expect("Failed to query shows");
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].name, "Persisted Podcast");
        assert_eq!(shows[0].episode_count, 1);

        let show_with_eps = service.get_show(&shows[0].id).expect("Failed to get show");
        assert_eq!(show_with_eps.episodes.len(), 1);
        assert_eq!(show_with_eps.episodes[0].filename, "episode1.wav");
        assert_eq!(
            show_with_eps.episodes[0].integrated_loudness_lufs,
            Some(-16.0)
        );
        assert_eq!(show_with_eps.episodes[0].true_peak_dbtp, Some(-1.5));
    }
}

#[test]
fn test_crud_show_management() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");

    // 1. Create
    let show = service
        .create_show("My Daily Tech", Some("Daily tech news"))
        .expect("Create failed");
    assert_eq!(show.name, "My Daily Tech");
    assert_eq!(show.description.as_deref(), Some("Daily tech news"));

    // 2. Read
    let shows = service.get_shows().expect("Get shows failed");
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].id, show.id);
    assert_eq!(shows[0].episode_count, 0);

    let retrieved = service.get_show(&show.id).expect("Get show failed");
    assert_eq!(retrieved.show.name, "My Daily Tech");
    assert_eq!(retrieved.episodes.len(), 0);

    // 3. Update / Rename
    let updated = service
        .update_show(&show.id, "My Daily Tech (Season 2)", Some("Updated desc"))
        .expect("Update failed");
    assert_eq!(updated.name, "My Daily Tech (Season 2)");
    assert_eq!(updated.description.as_deref(), Some("Updated desc"));

    // 4. Delete
    service.delete_show(&show.id).expect("Delete failed");
    let after_delete = service.get_shows().expect("Get shows failed");
    assert_eq!(after_delete.len(), 0);
}

#[test]
fn test_add_single_episode_and_fidelity() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let show = service
        .create_show("Science Hour", None)
        .expect("Create show failed");

    let media = create_test_media_source("/media/sci_ep01.wav", 1800.0, -15.8, -1.5);
    let outcome = service
        .add_media_source_to_show(&show.id, &media)
        .expect("Add media source failed");

    assert_eq!(outcome.status, AddEpisodeStatus::Added);
    assert_eq!(outcome.filename, "sci_ep01.wav");

    let episode = service
        .get_episode(&outcome.episode_id)
        .expect("Get episode failed");
    assert_eq!(episode.show_id, show.id);
    assert_eq!(episode.duration_seconds, 1800.0);
    assert_eq!(episode.format, MediaFormat::WAV);
    assert_eq!(episode.codec, "pcm_s16le");
    assert_eq!(episode.integrated_loudness_lufs, Some(-15.8));
    assert_eq!(episode.true_peak_dbtp, Some(-1.5));

    assert_eq!(episode.assessment_profile_id, "podcast-stereo-v1");
    assert_eq!(episode.assessment_profile_version, "1.0.0");
    assert_eq!(episode.overall_assessment_status, "READY");

    let assessment = episode.assessment.expect("Missing parsed assessment");
    assert_eq!(assessment.overall_status, OverallStatus::Ready);
    assert_eq!(assessment.audio_checks.len(), 5);


}

#[test]
fn test_duplicate_unchanged_detection() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let show = service
        .create_show("History Weekly", None)
        .expect("Create show failed");

    let media = create_test_media_source("/audio/hw_01.wav", 600.0, -16.0, -2.0);

    // First addition
    let first = service
        .add_media_source_to_show(&show.id, &media)
        .expect("First add failed");
    assert_eq!(first.status, AddEpisodeStatus::Added);

    // Second addition with identical source
    let second = service
        .add_media_source_to_show(&show.id, &media)
        .expect("Second add failed");
    assert_eq!(second.status, AddEpisodeStatus::AlreadyExists);
    assert_eq!(second.episode_id, first.episode_id);

    let show_data = service.get_show(&show.id).expect("Get show failed");
    assert_eq!(show_data.episodes.len(), 1, "Duplicate row was not created");
}

#[test]
fn test_changed_source_metadata_updates_catalogue() {
    let dir = tempdir().expect("Failed tempdir");
    let audio_file = dir.path().join("episode.wav");

    // Write initial audio fixture
    {
        let mut f = File::create(&audio_file).expect("Create file failed");
        f.write_all(b"RIFF....WAVEfmt ....data12345678").expect("Write failed");
    }

    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let show = service
        .create_show("Dynamic Podcast", None)
        .expect("Create show failed");

    let media_v1 = create_test_media_source(
        audio_file.to_str().unwrap(),
        300.0,
        -20.0,
        -3.0,
    );

    let res1 = service
        .add_media_source_to_show(&show.id, &media_v1)
        .expect("Add v1 failed");
    assert_eq!(res1.status, AddEpisodeStatus::Added);

    // Modify the audio file on disk (changing mtime and size)
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut f = File::create(&audio_file).expect("Re-create file failed");
        f.write_all(b"RIFF....WAVEfmt ....data12345678EXTRABYTES").expect("Write failed");
    }

    let media_v2 = create_test_media_source(
        audio_file.to_str().unwrap(),
        305.0,
        -16.0,
        -1.5,
    );

    let res2 = service
        .add_media_source_to_show(&show.id, &media_v2)
        .expect("Add v2 failed");
    assert_eq!(res2.status, AddEpisodeStatus::Updated);
    assert_eq!(res2.episode_id, res1.episode_id);

    let show_data = service.get_show(&show.id).expect("Get show failed");
    assert_eq!(show_data.episodes.len(), 1);
    assert_eq!(show_data.episodes[0].duration_seconds, 305.0);
    assert_eq!(
        show_data.episodes[0].integrated_loudness_lufs,
        Some(-16.0)
    );
}

#[test]
fn test_missing_source_file_representation() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let show = service
        .create_show("Ghost Podcast", None)
        .expect("Create show failed");

    // Non-existent path
    let media = create_test_media_source(
        "/non/existent/path/to/ep_missing.wav",
        100.0,
        -16.0,
        -1.5,
    );
    let outcome = service
        .add_media_source_to_show(&show.id, &media)
        .expect("Add failed");

    let ep = service
        .get_episode(&outcome.episode_id)
        .expect("Get episode failed");
    assert_eq!(ep.source_availability, SourceAvailability::Missing);

    let show_data = service.get_show(&show.id).expect("Get show failed");
    assert_eq!(
        show_data.episodes[0].source_availability,
        SourceAvailability::Missing
    );
}

#[test]
fn test_batch_results_to_catalogue_mapping_and_failure_isolation() {
    let service = CatalogueService::new_in_memory().expect("Failed to initialize catalogue");
    let show = service
        .create_show("Batch Mastery", None)
        .expect("Create show failed");

    let inspection = MediaInspection {
        duration_seconds: 120.0,
        sample_rate: 44100,
        channels: 2,
        bitrate: Some(192000),
        file_size_bytes: 20000,
    };
    let measurements = AudioMeasurements {
        integrated_loudness_lufs: Some(-16.0),
        true_peak_dbtp: Some(-1.5),
        leading_silence_seconds: 0.2,
        trailing_silence_seconds: 0.5,
        clipping: ClippingAnalysis {
            sample_peak_dbfs: Some(-1.5),
            samples_at_ceiling: 0,
            flat_factor: 0.0,
            evidence: ClippingEvidence::NONE,
        },
    };
    let assessment = assess_media(
        &inspection,
        Some(&measurements),
        &MediaFormat::WAV,
        "pcm_s16le",
    );

    let episodes = vec![
        // Complete item 1
        BatchEpisode {
            id: "batch_1".to_string(),
            source_path: "/tmp/batch_ep1.wav".to_string(),
            filename: "batch_ep1.wav".to_string(),
            status: BatchEpisodeStatus::Complete,
            format: Some(MediaFormat::WAV),
            codec: Some("pcm_s16le".to_string()),
            inspection: Some(inspection.clone()),
            measurements: Some(measurements.clone()),
            assessment: Some(assessment.clone()),
            duration_seconds: Some(120.0),
            elapsed_seconds: Some(0.4),
            error: None,
        },
        // Complete item 2
        BatchEpisode {
            id: "batch_2".to_string(),
            source_path: "/tmp/batch_ep2.wav".to_string(),
            filename: "batch_ep2.wav".to_string(),
            status: BatchEpisodeStatus::Complete,
            format: Some(MediaFormat::WAV),
            codec: Some("pcm_s16le".to_string()),
            inspection: Some(inspection.clone()),
            measurements: Some(measurements.clone()),
            assessment: Some(assessment.clone()),
            duration_seconds: Some(120.0),
            elapsed_seconds: Some(0.5),
            error: None,
        },
        // Failed item 3
        BatchEpisode {
            id: "batch_3".to_string(),
            source_path: "/tmp/corrupt.wav".to_string(),
            filename: "corrupt.wav".to_string(),
            status: BatchEpisodeStatus::Failed,
            format: None,
            codec: None,
            inspection: None,
            measurements: None,
            assessment: None,
            duration_seconds: None,
            elapsed_seconds: Some(0.1),
            error: Some("Invalid audio stream".to_string()),
        },
        // Cancelled item 4
        BatchEpisode {
            id: "batch_4".to_string(),
            source_path: "/tmp/cancelled.wav".to_string(),
            filename: "cancelled.wav".to_string(),
            status: BatchEpisodeStatus::Cancelled,
            format: None,
            codec: None,
            inspection: None,
            measurements: None,
            assessment: None,
            duration_seconds: None,
            elapsed_seconds: None,
            error: None,
        },
    ];

    let result = service
        .add_batch_episodes_to_show(&show.id, &episodes)
        .expect("Batch catalogue failed");

    assert_eq!(result.total_processed, 4);
    assert_eq!(result.added, 2);
    assert_eq!(result.skipped_failed, 2);
    assert_eq!(result.outcomes.len(), 2);

    let show_data = service.get_show(&show.id).expect("Get show failed");
    assert_eq!(show_data.episodes.len(), 2);
}

#[test]
fn test_source_safety_guarantee_audio_untouched_on_show_delete() {
    let dir = tempdir().expect("Failed tempdir");
    let audio_file = dir.path().join("master_audio.wav");

    let sample_content = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x00\x00\x00\x00";
    {
        let mut f = File::create(&audio_file).expect("Create file");
        f.write_all(sample_content).expect("Write content");
    }

    let original_hash = file_sha256(&audio_file);
    let original_len = std::fs::metadata(&audio_file).unwrap().len();

    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service
        .create_show("Safety First Show", None)
        .expect("Create show");

    let media = create_test_media_source(
        audio_file.to_str().unwrap(),
        10.0,
        -16.0,
        -1.5,
    );
    service
        .add_media_source_to_show(&show.id, &media)
        .expect("Add episode");

    // Verify episode is present in catalogue
    let show_data = service.get_show(&show.id).expect("Get show");
    assert_eq!(show_data.episodes.len(), 1);

    // Delete Show from catalogue
    service.delete_show(&show.id).expect("Delete show");

    // Source Safety Assertions:
    // 1. Audio file must still exist
    assert!(audio_file.exists(), "Audio file must exist after show deletion");
    // 2. Length must match exactly
    assert_eq!(
        std::fs::metadata(&audio_file).unwrap().len(),
        original_len,
        "File length must be unchanged"
    );
    // 3. SHA-256 hash must be byte-for-byte identical
    let after_hash = file_sha256(&audio_file);
    assert_eq!(
        after_hash, original_hash,
        "Audio file hash must be 100% byte-for-byte identical"
    );
}

#[test]
fn test_cascade_deletion_cleans_episodes() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Cascade Test", None).expect("Create show");

    let media1 = create_test_media_source("/tmp/ep1.wav", 10.0, -16.0, -1.5);
    let media2 = create_test_media_source("/tmp/ep2.wav", 10.0, -16.0, -1.5);
    let ep1 = service
        .add_media_source_to_show(&show.id, &media1)
        .expect("Add 1");
    let ep2 = service
        .add_media_source_to_show(&show.id, &media2)
        .expect("Add 2");

    assert!(service.get_episode(&ep1.episode_id).is_ok());
    assert!(service.get_episode(&ep2.episode_id).is_ok());

    service.delete_show(&show.id).expect("Delete show");

    assert!(service.get_episode(&ep1.episode_id).is_err());
    assert!(service.get_episode(&ep2.episode_id).is_err());
}

#[test]
fn test_canonical_app_identifier_is_com_podready_desktop() {
    let tauri_conf_str = include_str!("../../tauri.conf.json");
    let json: serde_json::Value = serde_json::from_str(tauri_conf_str).expect("Valid tauri.conf.json");
    let identifier = json["identifier"].as_str().expect("Identifier field exists");
    assert_eq!(identifier, "com.podready.desktop", "Canonical identifier must be com.podready.desktop");
}

#[test]
fn test_source_availability_three_states_and_retained_analysis() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Lifecycle Show", None).expect("Create show");

    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("lifecycle_ep.wav");
    {
        let mut f = File::create(&file_path).expect("create file");
        f.write_all(b"RIFF....WAVEINITIAL_CONTENT_V1").expect("write");
    }

    let media_v1 = create_test_media_source(
        file_path.to_str().unwrap(),
        300.0,
        -16.0,
        -1.5,
    );

    let outcome = service
        .add_media_source_to_show(&show.id, &media_v1)
        .expect("Add v1");
    assert_eq!(outcome.status, AddEpisodeStatus::Added);

    // State 1: Unchanged file on disk -> AVAILABLE
    let ep = service.get_episode(&outcome.episode_id).expect("get ep");
    assert_eq!(ep.source_availability, SourceAvailability::Available);
    assert_eq!(ep.integrated_loudness_lufs, Some(-16.0));
    assert_eq!(ep.duration_seconds, 300.0);

    // State 2: Deleted file on disk -> MISSING (historical analysis retained)
    std::fs::remove_file(&file_path).expect("delete file");
    let ep_missing = service.get_episode(&outcome.episode_id).expect("get ep missing");
    assert_eq!(ep_missing.source_availability, SourceAvailability::Missing);
    assert_eq!(ep_missing.integrated_loudness_lufs, Some(-16.0));
    assert_eq!(ep_missing.duration_seconds, 300.0);

    // State 3: Modified file on disk -> CHANGED (historical analysis retained)
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut f = File::create(&file_path).expect("re-create modified file");
        f.write_all(b"RIFF....WAVENEW_LONGER_CONTENT_V2_DIFFERENT_BYTES").expect("write");
    }
    let ep_changed = service.get_episode(&outcome.episode_id).expect("get ep changed");
    assert_eq!(ep_changed.source_availability, SourceAvailability::Changed);
    // Historical stored measurements remain completely intact
    assert_eq!(ep_changed.integrated_loudness_lufs, Some(-16.0));
    assert_eq!(ep_changed.true_peak_dbtp, Some(-1.5));
    assert_eq!(ep_changed.duration_seconds, 300.0);
    assert!(ep_changed.assessment.is_some());
}

#[test]
fn test_explicit_typed_columns_queryable_without_assessment_json() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Typed Columns Show", None).expect("Create show");

    let media = create_test_media_source("/tmp/typed_col_ep.wav", 1500.0, -16.0, -1.5);
    let outcome = service
        .add_media_source_to_show(&show.id, &media)
        .expect("Add ep");

    let ep = service.get_episode(&outcome.episode_id).expect("get ep");
    // All core facts are stored in explicit typed columns
    assert_eq!(ep.duration_seconds, 1500.0);
    assert_eq!(ep.format, MediaFormat::WAV);
    assert_eq!(ep.codec, "pcm_s16le");
    assert_eq!(ep.sample_rate, 44100);
    assert_eq!(ep.channels, 2);
    assert_eq!(ep.integrated_loudness_lufs, Some(-16.0));
    assert_eq!(ep.true_peak_dbtp, Some(-1.5));
    assert_eq!(ep.leading_silence_seconds, 0.5);
    assert_eq!(ep.trailing_silence_seconds, 1.0);
    assert_eq!(ep.overall_assessment_status, "READY");
    assert_eq!(ep.assessment_profile_id, "podcast-stereo-v1");
    assert_eq!(ep.assessment_profile_version, "1.0.0");
    // assessment_json is supplemental for detailed drilldown
    assert!(ep.assessment_json.is_some());
}

