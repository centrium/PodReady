use std::fs::File;
use std::io::Write;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

use crate::assessment::engine::{assess_media, OverallStatus};
use crate::batch::{BatchEpisode, BatchEpisodeStatus};

use crate::catalogue::baseline::{compute_show_baseline, BaselineMaturity, ShowBaseline};
use crate::catalogue::models::{
    AddEpisodeStatus, SourceAvailability,
};
use crate::catalogue::repository::CatalogueRepository;
use crate::catalogue::service::CatalogueService;
use crate::catalogue::show_check::{
    run_show_check, CandidateMeasurements, MetricComparisonStatus, MetricDirection,
    ShowCheckStatus,
};
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

// =========================================================================
// STAGE 5C: SHOW BASELINE & HISTORICAL CHARACTERISTICS TESTS
// =========================================================================

#[test]
fn test_baseline_maturity_model() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Maturity Show", None).expect("Create show");

    // 0 episodes -> NO_DATA
    let b0 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b0.maturity, BaselineMaturity::NoData);
    assert_eq!(b0.total_episodes, 0);
    assert_eq!(b0.eligible_episodes, 0);

    // 1 episode -> EARLY
    let m1 = create_test_media_source("/tmp/mat_ep1.wav", 600.0, -16.0, -1.5);
    service.add_media_source_to_show(&show.id, &m1).expect("add ep 1");
    let b1 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b1.maturity, BaselineMaturity::Early);
    assert_eq!(b1.eligible_episodes, 1);

    // 2 episodes -> EARLY
    let m2 = create_test_media_source("/tmp/mat_ep2.wav", 650.0, -15.8, -1.4);
    service.add_media_source_to_show(&show.id, &m2).expect("add ep 2");
    let b2 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b2.maturity, BaselineMaturity::Early);
    assert_eq!(b2.eligible_episodes, 2);

    // 3 episodes -> DEVELOPING
    let m3 = create_test_media_source("/tmp/mat_ep3.wav", 700.0, -16.2, -1.6);
    service.add_media_source_to_show(&show.id, &m3).expect("add ep 3");
    let b3 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b3.maturity, BaselineMaturity::Developing);
    assert_eq!(b3.eligible_episodes, 3);

    // 4 episodes -> DEVELOPING
    let m4 = create_test_media_source("/tmp/mat_ep4.wav", 720.0, -16.1, -1.5);
    service.add_media_source_to_show(&show.id, &m4).expect("add ep 4");
    let b4 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b4.maturity, BaselineMaturity::Developing);
    assert_eq!(b4.eligible_episodes, 4);

    // 5 episodes -> ESTABLISHED
    let m5 = create_test_media_source("/tmp/mat_ep5.wav", 800.0, -15.9, -1.3);
    service.add_media_source_to_show(&show.id, &m5).expect("add ep 5");
    let b5 = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(b5.maturity, BaselineMaturity::Established);
    assert_eq!(b5.eligible_episodes, 5);
}

#[test]
fn test_baseline_continuous_metrics_and_r7_quartiles() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Continuous Stats Show", None).expect("Create show");

    // Add 5 episodes with known loudness & true peak & duration
    // Loudness: -18.0, -16.5, -16.0, -15.5, -14.0 (sorted)
    // True Peak: -2.5, -2.0, -1.8, -1.5, -1.0 (sorted)
    // Duration: 100.0, 200.0, 300.0, 400.0, 500.0 (sorted)
    let eps = [
        ("/tmp/c_ep1.wav", 100.0, -18.0, -2.5),
        ("/tmp/c_ep2.wav", 200.0, -16.5, -2.0),
        ("/tmp/c_ep3.wav", 300.0, -16.0, -1.8),
        ("/tmp/c_ep4.wav", 400.0, -15.5, -1.5),
        ("/tmp/c_ep5.wav", 500.0, -14.0, -1.0),
    ];

    for (p, dur, lufs, tp) in eps {
        let m = create_test_media_source(p, dur, lufs, tp);
        service.add_media_source_to_show(&show.id, &m).expect("add ep");
    }

    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.eligible_episodes, 5);

    // Loudness check
    let l = baseline.loudness.expect("loudness metric");
    assert_eq!(l.sample_count, 5);
    assert_eq!(l.median, -16.0);
    assert_eq!(l.q1, -16.5);
    assert_eq!(l.q3, -15.5);
    assert_eq!(l.min, -18.0);
    assert_eq!(l.max, -14.0);

    // True Peak check
    let tp = baseline.true_peak.expect("true peak metric");
    assert_eq!(tp.sample_count, 5);
    assert_eq!(tp.median, -1.8);
    assert_eq!(tp.q1, -2.0);
    assert_eq!(tp.q3, -1.5);
    assert_eq!(tp.min, -2.5);
    assert_eq!(tp.max, -1.0);

    // Duration check
    let d = baseline.duration.expect("duration metric");
    assert_eq!(d.sample_count, 5);
    assert_eq!(d.median, 300.0);
    assert_eq!(d.q1, 200.0);
    assert_eq!(d.q3, 400.0);
}

#[test]
fn test_baseline_categorical_modal_distribution() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Categorical Show", None).expect("Create show");

    // 3 WAV episodes and 1 MP3 episode (using custom MediaSource)
    for i in 1..=3 {
        let path = format!("/tmp/cat_wav_{}.wav", i);
        let m = create_test_media_source(&path, 300.0, -16.0, -1.5);
        service.add_media_source_to_show(&show.id, &m).expect("add");
    }

    // Add 1 MP3 episode
    let mut mp3_source = create_test_media_source("/tmp/cat_mp3_1.mp3", 300.0, -16.0, -1.5);
    mp3_source.format = MediaFormat::MP3;
    mp3_source.codec = "mp3".to_string();
    service.add_media_source_to_show(&show.id, &mp3_source).expect("add mp3");

    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    let fmt = baseline.format.expect("format metric");
    assert_eq!(fmt.dominant_value, "WAV");
    assert_eq!(fmt.dominant_count, 3);
    assert_eq!(fmt.sample_count, 4);
    assert_eq!(fmt.dominant_proportion, 0.75);

    let ch = baseline.channels.expect("channels metric");
    assert_eq!(ch.dominant_value, "Stereo");
    assert_eq!(ch.dominant_count, 4);
    assert_eq!(ch.sample_count, 4);

    let sr = baseline.sample_rate.expect("sample rate metric");
    assert_eq!(sr.dominant_value, "44100 Hz");
    assert_eq!(sr.dominant_count, 4);
}

#[test]
fn test_baseline_source_state_eligibility() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("source_state_baseline.db");
    let repo = CatalogueRepository::open_file(&db_path).expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("Source State Baseline Show", None).expect("create show");

    // File 1: On-disk file that remains unchanged -> AVAILABLE (Eligible)
    let f1_path = dir.path().join("ep1_available.wav");
    {
        let mut f = File::create(&f1_path).expect("create f1");
        f.write_all(b"RIFF....WAVEep1_bytes").expect("write");
    }
    let m1 = create_test_media_source(f1_path.to_str().unwrap(), 300.0, -16.0, -1.5);
    service.add_media_source_to_show(&show.id, &m1).expect("add ep 1");

    // File 2: On-disk file that gets deleted -> MISSING (Eligible)
    let f2_path = dir.path().join("ep2_missing.wav");
    {
        let mut f = File::create(&f2_path).expect("create f2");
        f.write_all(b"RIFF....WAVEep2_bytes").expect("write");
    }
    let m2 = create_test_media_source(f2_path.to_str().unwrap(), 400.0, -17.0, -1.8);
    service.add_media_source_to_show(&show.id, &m2).expect("add ep 2");
    std::fs::remove_file(&f2_path).expect("delete f2");

    // File 3: On-disk file that gets modified -> CHANGED (Excluded from baseline!)
    let f3_path = dir.path().join("ep3_changed.wav");
    {
        let mut f = File::create(&f3_path).expect("create f3");
        f.write_all(b"RIFF....WAVEep3_original").expect("write");
    }
    let m3 = create_test_media_source(f3_path.to_str().unwrap(), 500.0, -10.0, 0.5);
    service.add_media_source_to_show(&show.id, &m3).expect("add ep 3");

    // Modify File 3
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut f = File::create(&f3_path).expect("modify f3");
        f.write_all(b"RIFF....WAVEep3_MODIFIED_DIFFERENT_SIZE_BYTES_XXXX").expect("write");
    }

    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.total_episodes, 3);
    assert_eq!(baseline.eligible_episodes, 2); // ep1 (AVAILABLE) + ep2 (MISSING)
    assert_eq!(baseline.excluded_episodes, 1); // ep3 (CHANGED)
    assert_eq!(baseline.exclusion_summary.changed_source_count, 1);

    // Baseline should reflect only ep1 (-16.0) and ep2 (-17.0), NOT ep3 (-10.0)
    let l = baseline.loudness.expect("loudness metric");
    assert_eq!(l.sample_count, 2);
    assert_eq!(l.median, -16.5); // (-17.0 + -16.0)/2
    assert_eq!(l.max, -16.0); // -10.0 is excluded!
}

#[test]
fn test_reanalysed_changed_source_becomes_eligible() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("reanalyse_baseline.db");
    let repo = CatalogueRepository::open_file(&db_path).expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("Reanalyse Baseline Show", None).expect("create show");

    let f_path = dir.path().join("ep_reanalyse.wav");
    {
        let mut f = File::create(&f_path).expect("create file");
        f.write_all(b"RIFF....WAVEep_initial").expect("write");
    }
    let m = create_test_media_source(f_path.to_str().unwrap(), 300.0, -16.0, -1.5);
    service.add_media_source_to_show(&show.id, &m).expect("add initial");

    // Baseline has 1 eligible episode
    assert_eq!(service.get_show_baseline(&show.id).unwrap().eligible_episodes, 1);

    // Modify file -> CHANGED -> 0 eligible episodes
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut f = File::create(&f_path).expect("modify file");
        f.write_all(b"RIFF....WAVEep_NEW_ANALYSIS_MODIFIED").expect("write");
    }
    let b_changed = service.get_show_baseline(&show.id).expect("get baseline changed");
    assert_eq!(b_changed.eligible_episodes, 0);
    assert_eq!(b_changed.excluded_episodes, 1);

    // Re-analyse file and update show catalogue
    let m_reanalysed = create_test_media_source(f_path.to_str().unwrap(), 350.0, -15.5, -1.2);
    let outcome = service.add_media_source_to_show(&show.id, &m_reanalysed).expect("re-catalogue");
    assert_eq!(outcome.status, AddEpisodeStatus::Updated);

    // Baseline now reflects updated analysis and is AVAILABLE again
    let b_updated = service.get_show_baseline(&show.id).expect("get baseline updated");
    assert_eq!(b_updated.eligible_episodes, 1);
    assert_eq!(b_updated.excluded_episodes, 0);
    assert_eq!(b_updated.loudness.unwrap().median, -15.5);
    assert_eq!(b_updated.duration.unwrap().median, 350.0);
}

#[test]
fn test_adding_and_removing_episode_changes_baseline() {
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Add Remove Show", None).expect("Create show");

    let m1 = create_test_media_source("/tmp/ar_ep1.wav", 300.0, -16.0, -1.5);
    let _out1 = service.add_media_source_to_show(&show.id, &m1).expect("add 1");

    let b1 = service.get_show_baseline(&show.id).expect("baseline 1");
    assert_eq!(b1.eligible_episodes, 1);
    assert_eq!(b1.loudness.unwrap().median, -16.0);

    let m2 = create_test_media_source("/tmp/ar_ep2.wav", 400.0, -14.0, -1.0);
    let out2 = service.add_media_source_to_show(&show.id, &m2).expect("add 2");

    let b2 = service.get_show_baseline(&show.id).expect("baseline 2");
    assert_eq!(b2.eligible_episodes, 2);
    assert_eq!(b2.loudness.unwrap().median, -15.0); // (-16 + -14)/2

    // Delete episode 2 -> baseline recalculates back to 1 episode
    service.delete_episode(&out2.episode_id).expect("delete ep 2");
    let b3 = service.get_show_baseline(&show.id).expect("baseline 3");
    assert_eq!(b3.eligible_episodes, 1);
    assert_eq!(b3.loudness.unwrap().median, -16.0);
}

#[test]
fn test_baseline_persists_across_db_reopen() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("persist_baseline.db");

    let show_id = {
        let repo = CatalogueRepository::open_file(&db_path).expect("open repo 1");
        let service = CatalogueService::new(repo);
        let show = service.create_show("Persistent Show", None).expect("create show");
        let m1 = create_test_media_source("/tmp/p_ep1.wav", 300.0, -16.2, -1.5);
        let m2 = create_test_media_source("/tmp/p_ep2.wav", 450.0, -15.8, -1.3);
        service.add_media_source_to_show(&show.id, &m1).expect("add 1");
        service.add_media_source_to_show(&show.id, &m2).expect("add 2");
        show.id
    };

    // Close and reopen database connection
    {
        let repo = CatalogueRepository::open_file(&db_path).expect("open repo 2");
        let service = CatalogueService::new(repo);
        let baseline = service.get_show_baseline(&show_id).expect("get baseline reopened");
        assert_eq!(baseline.eligible_episodes, 2);
        assert_eq!(baseline.loudness.unwrap().median, -16.0);
        assert_eq!(baseline.duration.unwrap().median, 375.0);
    }
}

#[test]
fn test_publishing_profile_independence_from_baseline() {
    // ARCHITECTURAL SEPARATION GUARANTEE:
    // A Show historically averaging -13.0 LUFS reports a baseline of -13.0 LUFS.
    // However, assessing a -13.0 LUFS episode with the Assessment Engine against
    // standard podcast profiles (podcast-stereo-v1) MUST still flag it as ATTENTION (Loud).
    // The baseline MUST NOT alter standard publishing profiles.

    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("Loud Show", None).expect("Create show");

    // Add 10 historical episodes all at approx -13.0 LUFS
    for i in 1..=10 {
        let path = format!("/tmp/loud_ep_{}.wav", i);
        let m = create_test_media_source(&path, 1200.0, -13.0, -0.8);
        service.add_media_source_to_show(&show.id, &m).expect("add loud ep");
    }

    // Baseline truthfully reports historical typical loudness ≈ -13.0 LUFS
    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.maturity, BaselineMaturity::Established);
    assert_eq!(baseline.eligible_episodes, 10);
    assert_eq!(baseline.loudness.unwrap().median, -13.0);

    // Now assess a candidate audio file that is -13.0 LUFS using the standard Assessment Engine
    let inspection = MediaInspection {
        duration_seconds: 1200.0,
        sample_rate: 44100,
        channels: 2,
        bitrate: Some(192000),
        file_size_bytes: 1024 * 100,
    };
    let measurements = AudioMeasurements {
        integrated_loudness_lufs: Some(-13.0),
        true_peak_dbtp: Some(-0.8),
        leading_silence_seconds: 0.5,
        trailing_silence_seconds: 1.0,
        clipping: ClippingAnalysis {
            sample_peak_dbfs: Some(-0.5),
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

    // Assessment Engine publishing profile targets are UNCHANGED (-16 LUFS stereo)
    // -13 LUFS is louder than -14.5 LUFS upper threshold -> triggers ATTENTION
    let loudness_check = assessment.audio_checks.iter().find(|c| c.id == "loudness").unwrap();
    assert_eq!(loudness_check.status, crate::assessment::engine::AssessmentStatus::Attention);
    assert!(loudness_check.message.contains("A little louder than we'd recommend"));
}

#[test]
fn test_realistic_calibration_fixture_12_episodes() {
    // Deterministic synthetic calibration fixture: 12 episodes representing a plausible show history
    let service = CatalogueService::new_in_memory().expect("Failed init");
    let show = service.create_show("The Audio Engineering Show", Some("A synthetic podcast fixture")).expect("create show");

    // Synthetic data: 12 episodes
    let fixture_episodes = vec![
        // (filename, duration, lufs, dbtp, format, codec, sample_rate, channels, leading_sil, trailing_sil, clipping)
        ("ep001.wav", 1800.0, -16.2, -1.8, MediaFormat::WAV, "pcm_s16le", 44100, 2, 0.4, 1.2, ClippingEvidence::NONE),
        ("ep002.wav", 1920.0, -16.0, -1.6, MediaFormat::WAV, "pcm_s16le", 44100, 2, 0.5, 1.5, ClippingEvidence::NONE),
        ("ep003.mp3", 1850.0, -16.5, -1.5, MediaFormat::MP3, "mp3", 44100, 2, 0.8, 2.0, ClippingEvidence::NONE),
        ("ep004.mp3", 2100.0, -15.8, -1.4, MediaFormat::MP3, "mp3", 44100, 2, 0.6, 1.8, ClippingEvidence::NONE),
        ("ep005.mp3", 1750.0, -16.4, -2.0, MediaFormat::MP3, "mp3", 44100, 2, 0.3, 1.1, ClippingEvidence::NONE),
        ("ep006.mp3", 1900.0, -16.1, -1.7, MediaFormat::MP3, "mp3", 44100, 2, 0.5, 1.4, ClippingEvidence::NONE),
        ("ep007.mp3", 2050.0, -16.3, -1.9, MediaFormat::MP3, "mp3", 44100, 2, 0.7, 2.2, ClippingEvidence::NONE),
        ("ep008.mp3", 1980.0, -15.9, -1.5, MediaFormat::MP3, "mp3", 44100, 2, 0.4, 1.6, ClippingEvidence::NONE),
        ("ep009.mp3", 2200.0, -16.6, -2.1, MediaFormat::MP3, "mp3", 44100, 2, 0.9, 2.5, ClippingEvidence::POSSIBLE),
        ("ep010.mp3", 1820.0, -16.0, -1.6, MediaFormat::MP3, "mp3", 44100, 2, 0.5, 1.3, ClippingEvidence::NONE),
        ("ep011.mp3", 1950.0, -16.2, -1.8, MediaFormat::MP3, "mp3", 44100, 2, 0.6, 1.7, ClippingEvidence::NONE),
        ("ep012.mp3", 2400.0, -15.7, -1.4, MediaFormat::MP3, "mp3", 44100, 2, 0.5, 1.9, ClippingEvidence::NONE),
    ];

    for ep in fixture_episodes {
        let inspection = MediaInspection {
            duration_seconds: ep.1,
            sample_rate: ep.6,
            channels: ep.7,
            bitrate: Some(192000),
            file_size_bytes: 1024 * 500,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(ep.2),
            true_peak_dbtp: Some(ep.3),
            leading_silence_seconds: ep.8,
            trailing_silence_seconds: ep.9,
            clipping: ClippingAnalysis {
                sample_peak_dbfs: Some(-1.0),
                samples_at_ceiling: 0,
                flat_factor: if ep.10 == ClippingEvidence::POSSIBLE { 0.1 } else { 0.0 },
                evidence: ep.10,
            },
        };
        let assessment = assess_media(&inspection, Some(&measurements), &ep.4, ep.5);
        let ms = MediaSource {
            path: format!("/synthetic/show/{}", ep.0),
            filename: ep.0.to_string(),
            format: ep.4,
            codec: ep.5.to_string(),
            inspection,
            measurements: Some(measurements),
            assessment: Some(assessment),
        };
        service.add_media_source_to_show(&show.id, &ms).expect("add fixture ep");
    }

    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.maturity, BaselineMaturity::Established);
    assert_eq!(baseline.eligible_episodes, 12);
    assert_eq!(baseline.total_episodes, 12);
    assert_eq!(baseline.excluded_episodes, 0);

    // Continuous metrics:
    // Loudness sorted (12): [-16.6, -16.5, -16.4, -16.3, -16.2, -16.2, -16.1, -16.0, -16.0, -15.9, -15.8, -15.7]
    // Median (p=0.5) -> (-16.2 + -16.1)/2 = -16.15 LUFS
    let l = baseline.loudness.expect("loudness");
    assert_eq!(l.sample_count, 12);
    assert!((l.median - -16.15).abs() < 1e-4);
    assert_eq!(l.min, -16.6);
    assert_eq!(l.max, -15.7);

    // Duration sorted (12): [1750, 1800, 1820, 1850, 1900, 1920, 1950, 1980, 2050, 2100, 2200, 2400]
    // Median -> (1920 + 1950)/2 = 1935.0 seconds (32:15)
    let d = baseline.duration.expect("duration");
    assert_eq!(d.median, 1935.0);

    // Categorical: Format (10 MP3, 2 WAV)
    let fmt = baseline.format.expect("format");
    assert_eq!(fmt.dominant_value, "MP3");
    assert_eq!(fmt.dominant_count, 10);
    assert_eq!(fmt.sample_count, 12);

    // Sample rate: 12 of 12 44100 Hz
    let sr = baseline.sample_rate.expect("sample rate");
    assert_eq!(sr.dominant_value, "44100 Hz");
    assert_eq!(sr.dominant_count, 12);

    // Channels: 12 of 12 Stereo
    let ch = baseline.channels.expect("channels");
    assert_eq!(ch.dominant_value, "Stereo");
    assert_eq!(ch.dominant_count, 12);

    // Clipping: 11 none, 1 possible
    assert_eq!(baseline.clipping.none_count, 11);
    assert_eq!(baseline.clipping.possible_count, 1);
    assert_eq!(baseline.clipping.total_checked, 12);

    // Historical points ordered chronologically
    assert_eq!(baseline.loudness_history.len(), 12);
    assert_eq!(baseline.true_peak_history.len(), 12);
}

#[test]
fn test_status_count_invariant_and_needs_attention_normalization() {
    let repo = CatalogueRepository::open_in_memory().expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("Invariant Show", None).expect("create show");

    // Episode 1: Healthy -> READY
    let m1 = create_test_media_source("/tmp/ep1.wav", 100.0, -16.0, -2.0);
    service.add_media_source_to_show(&show.id, &m1).expect("add ep 1");

    // Episode 2: Slightly loud -> ATTENTION
    let m2 = create_test_media_source("/tmp/ep2.wav", 100.0, -14.0, -2.0);
    service.add_media_source_to_show(&show.id, &m2).expect("add ep 2");

    // Episode 3: Very loud & hot peak -> NEEDS_ATTENTION
    let m3 = create_test_media_source("/tmp/ep3.wav", 100.0, -12.0, 0.5);
    service.add_media_source_to_show(&show.id, &m3).expect("add ep 3");

    let show_data = service.get_show(&show.id).expect("get show data");
    let episodes = show_data.episodes;

    assert_eq!(episodes.len(), 3);

    let ready_count = episodes.iter().filter(|e| e.overall_assessment_status == "READY").count();
    let attention_count = episodes.iter().filter(|e| e.overall_assessment_status == "ATTENTION").count();
    let needs_attention_count = episodes.iter().filter(|e| e.overall_assessment_status == "NEEDS_ATTENTION").count();
    let unknown_count = episodes.iter().filter(|e| !["READY", "ATTENTION", "NEEDS_ATTENTION"].contains(&e.overall_assessment_status.as_str())).count();

    assert_eq!(ready_count, 1);
    assert_eq!(attention_count, 1);
    assert_eq!(needs_attention_count, 1);
    assert_eq!(unknown_count, 0);

    // INVARIANT: sum of status counts MUST equal total catalogued episodes
    assert_eq!(ready_count + attention_count + needs_attention_count + unknown_count, episodes.len());
}

#[test]
fn test_zero_value_measurements_survive_persistence_and_baseline() {
    let repo = CatalogueRepository::open_in_memory().expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("Zero Value Show", None).expect("create show");

    // Create an episode with exactly 0.0 dBTP true peak and 0.0s silence boundaries
    let mut m = create_test_media_source("/tmp/zero_peak.wav", 120.0, -16.0, 0.0);
    if let Some(ref mut meas) = m.measurements {
        meas.true_peak_dbtp = Some(0.0);
        meas.leading_silence_seconds = 0.0;
        meas.trailing_silence_seconds = 0.0;
    }

    service.add_media_source_to_show(&show.id, &m).expect("add episode");

    let show_data = service.get_show(&show.id).expect("get show data");
    let ep = &show_data.episodes[0];

    // Verify 0.0 is preserved as Some(0.0) / 0.0, NOT None or missing
    assert_eq!(ep.true_peak_dbtp, Some(0.0));
    assert_eq!(ep.leading_silence_seconds, 0.0);
    assert_eq!(ep.trailing_silence_seconds, 0.0);

    // Verify baseline includes 0.0 in statistics
    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    let tp = baseline.true_peak.expect("true peak metric");
    assert_eq!(tp.sample_count, 1);
    assert_eq!(tp.median, 0.0);
    assert_eq!(tp.min, 0.0);
    assert_eq!(tp.max, 0.0);

    let ls = baseline.leading_silence.expect("leading silence");
    assert_eq!(ls.median, 0.0);
}

#[test]
fn test_unknown_assessment_with_valid_measurements_participates_in_baseline() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("unknown_assessment.db");
    let repo = CatalogueRepository::open_file(&db_path).expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("Unknown Assessment Show", None).expect("create show");

    // Insert an episode that has valid measurements
    let m = create_test_media_source("/tmp/ep_unknown.wav", 150.0, -18.0, -1.5);
    service.add_media_source_to_show(&show.id, &m).expect("add ep");

    // Directly mutate status to "UNKNOWN" in SQLite to simulate custom/legacy import
    {
        let conn = rusqlite::Connection::open(&db_path).expect("open conn");
        conn.execute("UPDATE episodes SET overall_assessment_status = 'UNKNOWN' WHERE show_id = ?1", rusqlite::params![show.id]).expect("update");
    }

    let show_data = service.get_show(&show.id).expect("get show");
    assert_eq!(show_data.episodes[0].overall_assessment_status, "UNKNOWN");

    // Baseline calculation must STILL include the valid acoustic measurements
    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.eligible_episodes, 1);
    assert_eq!(baseline.maturity, BaselineMaturity::Early);
    let l = baseline.loudness.expect("loudness metric");
    assert_eq!(l.median, -18.0);
    assert_eq!(l.sample_count, 1);
}

#[test]
fn test_synthetic_four_episode_ubercast_calibration_scenario() {
    let repo = CatalogueRepository::open_in_memory().expect("open repo");
    let service = CatalogueService::new(repo);

    let show = service.create_show("The UberCast", Some("Calibration show")).expect("create show");

    // Episode A: 79s, -23.0 LUFS, 0.0 dBTP, MP3, 44100Hz, Stereo, 1 possible clipping
    // (-23.0 LUFS is < -20.0 (Issue) and 0.0 dBTP is > -0.5 (Issue) -> NeedsAttention)
    let mut m_a = create_test_media_source("/tmp/epA.mp3", 79.0, -23.0, 0.0);
    m_a.format = MediaFormat::MP3;
    m_a.codec = "mp3".to_string();
    if let Some(ref mut meas) = m_a.measurements {
        meas.clipping.evidence = ClippingEvidence::POSSIBLE;
        meas.clipping.samples_at_ceiling = 10;
    }
    service.add_media_source_to_show(&show.id, &m_a).expect("add A");

    // Episode B: 31s, -14.2 LUFS, -1.9 dBTP, MP3, 44100Hz, Stereo, no clipping
    // (-14.2 LUFS is > -15.0 (Attention) -> Attention)
    let mut m_b = create_test_media_source("/tmp/epB.mp3", 31.0, -14.2, -1.9);
    m_b.format = MediaFormat::MP3;
    m_b.codec = "mp3".to_string();
    service.add_media_source_to_show(&show.id, &m_b).expect("add B");

    // Episode C: 44s, -16.6 LUFS, -4.2 dBTP, MP3, 44100Hz, Stereo, no clipping
    // (-16.6 LUFS within [-17, -15], -4.2 dBTP <= -1.5 -> Ready)
    let mut m_c = create_test_media_source("/tmp/epC.mp3", 44.0, -16.6, -4.2);
    m_c.format = MediaFormat::MP3;
    m_c.codec = "mp3".to_string();
    service.add_media_source_to_show(&show.id, &m_c).expect("add C");

    // Episode D: 85s, -15.4 LUFS, -1.0 dBTP, WAV, 44100Hz, Stereo, no clipping
    // (-1.0 dBTP is > -1.5 dBTP (Attention) -> Attention)
    let mut m_d = create_test_media_source("/tmp/epD.wav", 85.0, -15.4, -1.0);
    m_d.format = MediaFormat::WAV;
    m_d.codec = "pcm_s16le".to_string();
    service.add_media_source_to_show(&show.id, &m_d).expect("add D");

    // 1. Verify Status Counts and Invariant
    let show_data = service.get_show(&show.id).expect("get data");
    let episodes = &show_data.episodes;
    assert_eq!(episodes.len(), 4);

    let ep_a = episodes.iter().find(|e| e.filename == "epA.mp3").unwrap();
    let ep_b = episodes.iter().find(|e| e.filename == "epB.mp3").unwrap();
    let ep_c = episodes.iter().find(|e| e.filename == "epC.mp3").unwrap();
    let ep_d = episodes.iter().find(|e| e.filename == "epD.wav").unwrap();

    assert_eq!(ep_a.overall_assessment_status, "NEEDS_ATTENTION");
    assert_eq!(ep_b.overall_assessment_status, "ATTENTION");
    assert_eq!(ep_c.overall_assessment_status, "READY");
    assert_eq!(ep_d.overall_assessment_status, "ATTENTION");

    let ready_count = episodes.iter().filter(|e| e.overall_assessment_status == "READY").count();
    let attention_count = episodes.iter().filter(|e| e.overall_assessment_status == "ATTENTION").count();
    let needs_attention_count = episodes.iter().filter(|e| e.overall_assessment_status == "NEEDS_ATTENTION").count();
    let unknown_count = episodes.iter().filter(|e| !["READY", "ATTENTION", "NEEDS_ATTENTION"].contains(&e.overall_assessment_status.as_str())).count();

    assert_eq!(ready_count, 1);
    assert_eq!(attention_count, 2);
    assert_eq!(needs_attention_count, 1);
    assert_eq!(unknown_count, 0);

    // Sum invariant: All (4) = Ready (1) + Attention (2) + Needs Attention (1) + Unknown (0)
    assert_eq!(ready_count + attention_count + needs_attention_count + unknown_count, 4);

    // 2. Verify Baseline Calculations for 4 episodes (Developing)
    let baseline = service.get_show_baseline(&show.id).expect("get baseline");
    assert_eq!(baseline.maturity, BaselineMaturity::Developing);
    assert_eq!(baseline.eligible_episodes, 4);

    // Loudness: sorted [-23.0, -16.6, -15.4, -14.2]
    // Median: (-16.6 + -15.4) / 2 = -16.0 LUFS
    // Q1 (Method 7, h = 3 * 0.25 = 0.75): -23.0 + 0.75 * 6.4 = -18.2 LUFS
    // Q3 (Method 7, h = 3 * 0.75 = 2.25): -15.4 + 0.25 * 1.2 = -15.1 LUFS
    let l = baseline.loudness.expect("loudness");
    assert!((l.median - -16.0).abs() < 1e-4);
    assert!((l.q1 - -18.2).abs() < 1e-4);
    assert!((l.q3 - -15.1).abs() < 1e-4);

    // True Peak: sorted [-4.2, -1.9, -1.0, 0.0]
    // Median: (-1.9 + -1.0) / 2 = -1.45 dBTP
    // Q1: -4.2 + 0.75 * 2.3 = -2.475 dBTP
    // Q3: -1.0 + 0.25 * 1.0 = -0.75 dBTP
    let tp = baseline.true_peak.expect("true peak");
    assert!((tp.median - -1.45).abs() < 1e-4);
    assert!((tp.q1 - -2.475).abs() < 1e-4);
    assert!((tp.q3 - -0.75).abs() < 1e-4);

    // Duration: sorted [31, 44, 79, 85]
    // Median: (44 + 79) / 2 = 61.5s
    // Q1: 31 + 0.75 * 13 = 40.75s
    // Q3: 79 + 0.25 * 6 = 80.5s
    let dur = baseline.duration.expect("duration");
    assert!((dur.median - 61.5).abs() < 1e-4);
    assert!((dur.q1 - 40.75).abs() < 1e-4);
    assert!((dur.q3 - 80.5).abs() < 1e-4);

    // Delivery characteristics:
    // Format: 3 MP3, 1 WAV -> Dominant MP3 (3 of 4 episodes · 75%)
    let fmt = baseline.format.expect("format");
    assert_eq!(fmt.dominant_value, "MP3");
    assert_eq!(fmt.dominant_count, 3);
    assert_eq!(fmt.sample_count, 4);

    // Channels: 4 Stereo -> Dominant Stereo (4 of 4 episodes)
    let ch = baseline.channels.expect("channels");
    assert_eq!(ch.dominant_value, "Stereo");
    assert_eq!(ch.dominant_count, 4);
    assert_eq!(ch.sample_count, 4);

    // Clipping: 3 none, 1 possible
    assert_eq!(baseline.clipping.none_count, 3);
    assert_eq!(baseline.clipping.possible_count, 1);
    assert_eq!(baseline.clipping.total_checked, 4);
}

// =========================================================================
// STAGE 5D: SHOW CHECK / EPISODE-TO-SHOW COMPARISON ENGINE TESTS
// =========================================================================

#[test]
fn test_show_check_candidate_inside_q1_q3_is_typical() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("UberCast", None).expect("show");

    // Add 4 episodes with loudness: -18.0, -16.5, -15.5, -14.0 LUFS (Median = -16.0, Q1 = -17.625, Q3 = -14.375)
    for (i, lufs) in [-18.0, -16.5, -15.5, -14.0].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    let candidate = create_test_media_source("/test/candidate.wav", 60.0, -16.0, -1.5);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    assert_eq!(check.status, ShowCheckStatus::Typical);
    let l_metric = check.metrics.iter().find(|m| m.id == "loudness").expect("loudness metric");
    assert_eq!(l_metric.status, MetricComparisonStatus::Typical);
    assert_eq!(l_metric.direction, MetricDirection::WithinUsual);
    assert!(l_metric.message.contains("Within this Show's usual loudness range"));
}

#[test]
fn test_show_check_candidate_slightly_outside_usual_range() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("UberCast", None).expect("show");

    // 4 episodes: -17.0, -16.5, -15.5, -15.0 LUFS
    // Q1 = -16.875, Q3 = -15.125, IQR = 1.75 LU
    for (i, lufs) in [-17.0, -16.5, -15.5, -15.0].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate at -17.5 LUFS (< Q1 (-16.875), but >= Q1 - IQR (-18.625)) -> SLIGHTLY_DIFFERENT
    let candidate = create_test_media_source("/test/candidate.wav", 60.0, -17.5, -1.5);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    let l_metric = check.metrics.iter().find(|m| m.id == "loudness").expect("loudness metric");
    assert_eq!(l_metric.status, MetricComparisonStatus::SlightlyDifferent);
    assert_eq!(l_metric.direction, MetricDirection::BelowUsual);
    // Overall status remains TYPICAL because only slightly different
    assert_eq!(check.status, ShowCheckStatus::Typical);
    assert!(check.summary.contains("Within normal variation for this Show with minor differences"));
}

#[test]
fn test_show_check_candidate_materially_outside_history_is_different() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("UberCast", None).expect("show");

    for (i, lufs) in [-17.0, -16.5, -15.5, -15.0].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate at -23.0 LUFS (< Q1 - IQR = -18.625) -> DIFFERENT
    let candidate = create_test_media_source("/test/candidate.wav", 60.0, -23.0, -1.5);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    assert_eq!(check.status, ShowCheckStatus::Different);
    let l_metric = check.metrics.iter().find(|m| m.id == "loudness").expect("loudness metric");
    assert_eq!(l_metric.status, MetricComparisonStatus::Different);
    assert_eq!(l_metric.direction, MetricDirection::BelowUsual);
    assert!(l_metric.message.contains("Quieter than this Show usually runs"));
}

#[test]
fn test_show_check_candidate_below_and_above_directions() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("UberCast", None).expect("show");

    for (i, lufs) in [-17.0, -16.0, -15.0, -14.0].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Below usual
    let quiet_cand = create_test_media_source("/test/quiet.wav", 60.0, -20.0, -1.5);
    let quiet_check = service.run_show_check_for_media(&show.id, &quiet_cand).expect("check");
    let q_l = quiet_check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(q_l.direction, MetricDirection::BelowUsual);

    // Above usual
    let loud_cand = create_test_media_source("/test/loud.wav", 60.0, -10.0, -1.5);
    let loud_check = service.run_show_check_for_media(&show.id, &loud_cand).expect("check");
    let l_l = loud_check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(l_l.direction, MetricDirection::AboveUsual);
}

#[test]
fn test_show_check_no_data_is_insufficient_data() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("EmptyShow", None).expect("show");

    let candidate = create_test_media_source("/test/cand.wav", 60.0, -16.0, -1.5);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    assert_eq!(check.status, ShowCheckStatus::InsufficientData);
    assert_eq!(check.baseline_maturity, BaselineMaturity::NoData);
    assert_eq!(check.baseline_episode_count, 0);
    assert_eq!(check.summary, "No baseline history available for comparison.");
    assert!(check.metrics.is_empty());
}

#[test]
fn test_show_check_maturity_copy_progression() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("MaturityShow", None).expect("show");

    // 1 episode -> EARLY maturity
    let ep1 = create_test_media_source("/test/ep1.wav", 60.0, -16.0, -1.5);
    service.add_media_source_to_show(&show.id, &ep1).expect("add");

    let cand_early = create_test_media_source("/test/cand_early.wav", 60.0, -12.0, -1.5);
    let check_early = service.run_show_check_for_media(&show.id, &cand_early).expect("check");
    assert_eq!(check_early.baseline_maturity, BaselineMaturity::Early);
    let l_early = check_early.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert!(l_early.message.contains("Louder than the episodes currently in this Show"));
    assert_eq!(check_early.summary, "Louder than current episodes in this Show.");

    // Add 2 more episodes (total 3) -> DEVELOPING maturity
    for (i, lufs) in [-15.5, -16.5].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i + 2), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    let check_dev = service.run_show_check_for_media(&show.id, &cand_early).expect("check");
    assert_eq!(check_dev.baseline_maturity, BaselineMaturity::Developing);
    let l_dev = check_dev.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert!(l_dev.message.contains("Louder than this Show usually runs"));
    assert_eq!(check_dev.summary, "Noticeably louder than this Show usually runs.");

    // Add 2 more episodes (total 5) -> ESTABLISHED maturity
    for (i, lufs) in [-16.0, -15.8].iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i + 4), 60.0, *lufs, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    let check_est = service.run_show_check_for_media(&show.id, &cand_early).expect("check");
    assert_eq!(check_est.baseline_maturity, BaselineMaturity::Established);
    assert_eq!(check_est.summary, "Noticeably louder than this Show usually runs.");
}

#[test]
fn test_show_check_zero_iqr_handles_small_variations() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("ZeroIQRShow", None).expect("show");

    // 20 episodes all at exactly -16.0 LUFS -> Q1 = -16.0, Q3 = -16.0, IQR = 0.0
    for i in 0..20 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -16.0, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate at -15.9 LUFS (0.1 LU difference).
    // Because MinTol = 0.5 LU, effective band is 0.5 LU.
    // -15.9 <= -16.0 + 0.5 = -15.5 -> SLIGHTLY_DIFFERENT, NOT DIFFERENT!
    let candidate = create_test_media_source("/test/cand.wav", 60.0, -15.9, -1.5);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    let l_metric = check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(l_metric.status, MetricComparisonStatus::SlightlyDifferent);
    assert_eq!(l_metric.direction, MetricDirection::AboveUsual);
    assert_eq!(check.status, ShowCheckStatus::Typical); // slight differences do not make whole show check DIFFERENT
}

#[test]
fn test_show_check_zero_dbtp_is_legitimate_value() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("ZeroPeakShow", None).expect("show");

    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -16.0, -1.0);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate has 0.0 dBTP True Peak
    let candidate = create_test_media_source("/test/cand.wav", 60.0, -16.0, 0.0);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    let tp_metric = check.metrics.iter().find(|m| m.id == "truePeak" || m.id == "true_peak").expect("tp metric");
    assert!((tp_metric.candidate_value - 0.0).abs() < 1e-4);
    assert_eq!(tp_metric.direction, MetricDirection::AboveUsual);
}

#[test]
fn test_show_check_missing_candidate_metric_does_not_fail_whole_check() {
    let baseline = ShowBaseline {
        show_id: "show1".to_string(),
        show_name: "TestShow".to_string(),
        maturity: BaselineMaturity::Established,
        total_episodes: 5,
        eligible_episodes: 5,
        excluded_episodes: 0,
        exclusion_summary: crate::catalogue::baseline::BaselineExclusionSummary {
            changed_source_count: 0,
            missing_measurement_count: 0,
        },
        generated_at: chrono::Utc::now().to_rfc3339(),
        loudness: Some(crate::catalogue::stats::ContinuousBaselineMetric {
            id: "loudness".to_string(),
            label: "Loudness".to_string(),
            unit: "LUFS".to_string(),
            sample_count: 5,
            median: -16.0,
            q1: -17.0,
            q3: -15.0,
            min: -18.0,
            max: -14.0,
        }),
        true_peak: Some(crate::catalogue::stats::ContinuousBaselineMetric {
            id: "true_peak".to_string(),
            label: "True Peak".to_string(),
            unit: "dBTP".to_string(),
            sample_count: 5,
            median: -1.5,
            q1: -2.0,
            q3: -1.0,
            min: -3.0,
            max: -0.5,
        }),
        duration: None,
        leading_silence: None,
        trailing_silence: None,
        bitrate: None,
        format: None,
        sample_rate: None,
        channels: None,
        codec: None,
        clipping: crate::catalogue::baseline::ClippingBaselineSummary {
            total_checked: 5,
            none_count: 5,
            possible_count: 0,
            uncertain_count: 0,
        },
        loudness_history: Vec::new(),
        true_peak_history: Vec::new(),
    };

    // Candidate has loudness = None, but true_peak = Some(-1.5)
    let candidate = CandidateMeasurements {
        duration_seconds: 60.0,
        format: "WAV".to_string(),
        codec: "pcm_s16le".to_string(),
        sample_rate: 44100,
        channels: 2,
        bitrate: None,
        integrated_loudness_lufs: None,
        true_peak_dbtp: Some(-1.5),
        leading_silence_seconds: None,
        trailing_silence_seconds: None,
    };

    let check = run_show_check(&baseline, &candidate, false);
    assert_eq!(check.status, ShowCheckStatus::Typical);
    assert_eq!(check.metrics.len(), 1);
    assert_eq!(check.metrics[0].id, "true_peak");
}

#[test]
fn test_show_check_categorical_matches_and_differs() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("StereoShow", None).expect("show");

    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -16.0, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Matching format WAV & Stereo
    let matching_cand = create_test_media_source("/test/cand.wav", 60.0, -16.0, -1.5);
    let check_match = service.run_show_check_for_media(&show.id, &matching_cand).expect("check");

    let fmt = check_match.categorical_metrics.iter().find(|c| c.id == "format").unwrap();
    assert_eq!(fmt.status, MetricComparisonStatus::Typical);
    assert!(fmt.message.contains("Matches this Show's usual WAV format"));

    // Differing channel: Mono candidate
    let mut mono_cand = create_test_media_source("/test/mono.wav", 60.0, -19.0, -1.5);
    mono_cand.inspection.channels = 1;
    let check_mono = service.run_show_check_for_media(&show.id, &mono_cand).expect("check");

    let ch = check_mono.categorical_metrics.iter().find(|c| c.id == "channels").unwrap();
    assert_eq!(ch.status, MetricComparisonStatus::Different);
    assert!(ch.message.contains("This episode is mono; this Show is usually stereo"));
}

#[test]
fn test_show_check_publishing_assessment_independence() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("LoudShow", None).expect("show");

    // Historical show averages -13.0 LUFS (loud for podcasting)
    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -13.0, -1.0);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate is also -13.0 LUFS
    let candidate = create_test_media_source("/test/cand.wav", 60.0, -13.0, -1.0);

    // 1. PodReady Check (Assessment Engine) MUST judge ATTENTION (Loud) based on AUDIO_RULES (-16 LUFS stereo target)
    let assessment = candidate.assessment.as_ref().unwrap();
    assert_eq!(assessment.overall_status, OverallStatus::Attention);
    let loudness_check = assessment.audio_checks.iter().find(|c| c.id == "loudness").unwrap();
    assert_eq!(loudness_check.status, crate::assessment::engine::AssessmentStatus::Attention);

    // 2. Show Check MUST judge TYPICAL because candidate matches the show's -13.0 LUFS history
    let show_check = service.run_show_check_for_media(&show.id, &candidate).expect("check");
    assert_eq!(show_check.status, ShowCheckStatus::Typical);
    let sc_loudness = show_check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(sc_loudness.status, MetricComparisonStatus::Typical);

    // Assessment is completely unaffected by Show Check
    assert_eq!(candidate.assessment.as_ref().unwrap().overall_status, OverallStatus::Attention);
}

#[test]
fn test_show_check_status_combinations() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("ComboShow", None).expect("show");

    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -16.0, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Combination A: PodReady READY + Show DIFFERENT
    // -16.0 LUFS stereo, -1.5 dBTP (Ready for publishing), but duration = 1800s (30m) vs show usual 60s (1m)
    let cand_a = create_test_media_source("/test/cand_a.wav", 1800.0, -16.0, -1.5);
    assert_eq!(cand_a.assessment.as_ref().unwrap().overall_status, OverallStatus::Ready);
    let check_a = service.run_show_check_for_media(&show.id, &cand_a).expect("check a");
    assert_eq!(check_a.status, ShowCheckStatus::Different);

    // Combination B: PodReady ATTENTION + Show TYPICAL
    // -14.0 LUFS (Attention - Loud), but Show has 5 episodes at -14.0 LUFS
    let loud_show = service.create_show("LoudShow2", None).expect("show");
    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep_loud_{}.wav", i), 60.0, -14.0, -1.5);
        service.add_media_source_to_show(&loud_show.id, &media).expect("add");
    }
    let cand_b = create_test_media_source("/test/cand_b.wav", 60.0, -14.0, -1.5);
    assert_eq!(cand_b.assessment.as_ref().unwrap().overall_status, OverallStatus::Attention);
    let check_b = service.run_show_check_for_media(&loud_show.id, &cand_b).expect("check b");
    assert_eq!(check_b.status, ShowCheckStatus::Typical);
}

#[test]
fn test_show_check_catalogued_episode_leave_one_out_and_n_minus_one_maturity() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("LOO_Show", None).expect("show");

    // Add 5 episodes to Show (Full Show Baseline = ESTABLISHED with 5 episodes)
    let mut ep_ids = Vec::new();
    for i in 0..5 {
        let lufs = if i == 4 { -10.0 } else { -16.0 }; // Episode 4 is an outlier at -10 LUFS
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, lufs, -1.5);
        let outcome = service.add_media_source_to_show(&show.id, &media).expect("add");
        ep_ids.push(outcome.episode_id);
    }

    // Full Show Baseline maturity is ESTABLISHED (5 episodes)
    let full_baseline = service.get_show_baseline(&show.id).expect("full baseline");
    assert_eq!(full_baseline.maturity, BaselineMaturity::Established);
    assert_eq!(full_baseline.eligible_episodes, 5);

    // Leave-One-Out Show Check on Episode 4 (the -10 LUFS outlier):
    // Comparison history excludes Episode 4, leaving 4 episodes at -16.0 LUFS.
    let check_ep4 = service.get_show_check_for_episode(&ep_ids[4]).expect("check ep4");

    // LOO maturity must be DEVELOPING (4 episodes = N - 1)
    assert_eq!(check_ep4.baseline_maturity, BaselineMaturity::Developing);
    assert_eq!(check_ep4.baseline_episode_count, 4);

    // Episode 4 at -10 LUFS is compared against 4 episodes at -16 LUFS -> DIFFERENT
    assert_eq!(check_ep4.status, ShowCheckStatus::Different);
    let l_ep4 = check_ep4.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(l_ep4.status, MetricComparisonStatus::Different);
    assert_eq!(l_ep4.direction, MetricDirection::AboveUsual);
}

#[test]
fn test_show_check_workspace_candidate_uses_full_baseline() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("WorkspaceShow", None).expect("show");

    for i in 0..5 {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), 60.0, -16.0, -1.5);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    let new_candidate = create_test_media_source("/test/new_cand.wav", 60.0, -16.0, -1.5);
    let check = service.run_show_check_for_media(&show.id, &new_candidate).expect("check");

    // Candidate not in catalogue -> uses full 5 episodes -> ESTABLISHED
    assert_eq!(check.baseline_maturity, BaselineMaturity::Established);
    assert_eq!(check.baseline_episode_count, 5);
}

#[test]
fn test_show_check_source_availability_missing_and_changed_states() {
    let temp = tempdir().expect("tempdir");
    let db_path = temp.path().join("test_source_states.db");
    let repo = CatalogueRepository::open_file(&db_path).expect("repo");
    let service = CatalogueService::new(repo);
    let show = service.create_show("StateShow", None).expect("show");

    // Create real files
    let file1_path = temp.path().join("ep1.wav");
    let file2_path = temp.path().join("ep2.wav");
    let file3_path = temp.path().join("ep3.wav");
    {
        let mut f1 = File::create(&file1_path).unwrap();
        f1.write_all(b"audio data 1").unwrap();
        let mut f2 = File::create(&file2_path).unwrap();
        f2.write_all(b"audio data 2").unwrap();
        let mut f3 = File::create(&file3_path).unwrap();
        f3.write_all(b"audio data 3").unwrap();
    }

    let m1 = create_test_media_source(file1_path.to_str().unwrap(), 60.0, -16.0, -1.5);
    let m2 = create_test_media_source(file2_path.to_str().unwrap(), 60.0, -16.0, -1.5);
    let m3 = create_test_media_source(file3_path.to_str().unwrap(), 60.0, -16.0, -1.5);

    let _o1 = service.add_media_source_to_show(&show.id, &m1).expect("add1");
    let o2 = service.add_media_source_to_show(&show.id, &m2).expect("add2");
    let o3 = service.add_media_source_to_show(&show.id, &m3).expect("add3");

    // 1. Delete file 2 -> MISSING state
    std::fs::remove_file(&file2_path).unwrap();
    let check_missing = service.get_show_check_for_episode(&o2.episode_id).expect("check missing");
    // MISSING still displays stored comparison and is NOT marked stale
    assert_eq!(check_missing.is_stale, false);
    assert_eq!(check_missing.status, ShowCheckStatus::Typical);

    // 2. Modify file 3 -> CHANGED state
    {
        let mut f3 = File::create(&file3_path).unwrap();
        f3.write_all(b"changed audio data 333333333333333333333").unwrap();
    }
    let check_changed = service.get_show_check_for_episode(&o3.episode_id).expect("check changed");
    // CHANGED displays comparison but is explicitly marked is_stale = true
    assert_eq!(check_changed.is_stale, true);
}

#[test]
fn test_show_check_synthetic_ubercast_calibration_fixture() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("The UberCast", None).expect("show");

    // 4 historical episodes matching Stage 5D Section 15 fixture:
    // Loudness: sorted [-23.0, -16.6, -15.4, -14.2] -> Median = -16.0, Q1 = -18.2, Q3 = -15.1 LUFS
    // True Peak: sorted [-4.2, -1.9, -1.0, 0.0] -> Median = -1.45, Q1 = -2.475, Q3 = -0.75 dBTP
    // Duration: sorted [31s, 44s, 79s, 85s] -> Median = 61.5s (1:02), Q1 = 40.75s (0:41), Q3 = 80.5s (1:21)
    let eps_data = [
        (44.0, -15.4, -1.9),
        (31.0, -16.6, -1.0),
        (85.0, -14.2, 0.0),
        (79.0, -23.0, -4.2),
    ];

    for (i, (dur, lufs, tp)) in eps_data.iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.wav", i), *dur, *lufs, *tp);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate:
    // Loudness: -15.4 LUFS (inside -18.2 → -15.1 range) -> TYPICAL
    // True Peak: -1.0 dBTP (inside -2.475 → -0.75 range) -> TYPICAL
    // Duration: 85.0s (1:25) (> Q3 (80.5s / 1:21), but <= Q3 + IQR (80.5 + 39.75 = 120.25s)) -> SLIGHTLY_DIFFERENT (Slightly longer)
    let candidate = create_test_media_source("/test/candidate.wav", 85.0, -15.4, -1.0);
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    assert_eq!(check.baseline_maturity, BaselineMaturity::Developing);
    assert_eq!(check.baseline_episode_count, 4);

    let l_m = check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(l_m.status, MetricComparisonStatus::Typical);
    assert_eq!(l_m.direction, MetricDirection::WithinUsual);

    let tp_m = check.metrics.iter().find(|m| m.id == "truePeak" || m.id == "true_peak").unwrap();
    assert_eq!(tp_m.status, MetricComparisonStatus::Typical);
    assert_eq!(tp_m.direction, MetricDirection::WithinUsual);

    let dur_m = check.metrics.iter().find(|m| m.id == "duration").unwrap();
    assert_eq!(dur_m.status, MetricComparisonStatus::SlightlyDifferent);
    assert_eq!(dur_m.direction, MetricDirection::AboveUsual);
    assert!(dur_m.message.contains("Slightly longer than the current Show baseline"));

    // Overall status remains TYPICAL with minor variation summary
    assert_eq!(check.status, ShowCheckStatus::Typical);
    assert_eq!(check.summary, "Within normal variation for this Show with minor differences.");
}

// =========================================================================
// STAGE 5D.1: SHOW CHECK PRODUCT REFINEMENT & CALIBRATION TESTS
// =========================================================================

#[test]
fn test_show_check_stage_5d1_full_calibration_candidate_fixture() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("The UberCast", None).expect("show");

    // 4 episodes matching Stage 5D.1 Section 14 fixture:
    // Loudness: sorted [-23.0, -16.6, -15.4, -14.2] -> Median = -16.0, Q1 = -18.2, Q3 = -15.1
    // True peak: sorted [-4.2, -1.9, -1.0, 0.0] -> Median = -1.45, Q1 = -2.475, Q3 = -0.75
    // Duration: sorted [31s, 44s, 79s, 85s] -> Median = 61.5s, Q1 = 40.75s, Q3 = 80.5s
    // Dominant: MP3, 44.1 kHz, Stereo
    let eps = [
        (44.0, -15.4, -1.9),
        (31.0, -16.6, -1.0),
        (85.0, -14.2, 0.0),
        (79.0, -23.0, -4.2),
    ];
    for (i, (dur, lufs, tp)) in eps.iter().enumerate() {
        let media = create_test_media_source(&format!("/test/ep{}.mp3", i), *dur, *lufs, *tp);
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate: Mono MP3, 1:19 (79s), -23.0 LUFS, 0.0 dBTP, 0.3s leading, 0.8s trailing, 90 kbps (90000), 44.1 kHz
    let mut candidate = create_test_media_source("/test/candidate.mp3", 79.0, -23.0, 0.0);
    candidate.inspection.channels = 1;
    candidate.format = MediaFormat::MP3;
    if let Some(m) = candidate.measurements.as_mut() {
        m.leading_silence_seconds = 0.3;
        m.trailing_silence_seconds = 0.8;
    }
    candidate.inspection.bitrate = Some(90000);
    // Re-assess candidate with Mono configuration
    candidate.assessment = Some(assess_media(
        &candidate.inspection,
        candidate.measurements.as_ref(),
        &candidate.format,
        &candidate.codec,
    ));

    // 1. Run Show Check
    let check = service.run_show_check_for_media(&show.id, &candidate).expect("check");

    assert_eq!(check.status, ShowCheckStatus::Different);
    assert_eq!(check.baseline_maturity, BaselineMaturity::Developing);
    assert_eq!(check.baseline_episode_count, 4);

    // Primary story:
    // - Loudness: -23.0 LUFS -> DIFFERENT (BelowUsual)
    let loudness = check.metrics.iter().find(|m| m.id == "loudness").unwrap();
    assert_eq!(loudness.status, MetricComparisonStatus::Different);
    assert_eq!(loudness.direction, MetricDirection::BelowUsual);

    // - True Peak: 0.0 dBTP -> SLIGHTLY_DIFFERENT (AboveUsual)
    let tp = check.metrics.iter().find(|m| m.id == "truePeak" || m.id == "true_peak").unwrap();
    assert_eq!(tp.status, MetricComparisonStatus::SlightlyDifferent);
    assert_eq!(tp.direction, MetricDirection::AboveUsual);

    // - Duration: 79.0s -> TYPICAL (WithinUsual)
    let dur = check.metrics.iter().find(|m| m.id == "duration").unwrap();
    assert_eq!(dur.status, MetricComparisonStatus::Typical);
    assert_eq!(dur.direction, MetricDirection::WithinUsual);

    // - Channels: Mono -> DIFFERENT (Show is Stereo)
    let channels = check.categorical_metrics.iter().find(|m| m.id == "channels").unwrap();
    assert_eq!(channels.status, MetricComparisonStatus::Different);

    // Headline Summary synthesises the two primary differences deterministically
    assert_eq!(
        check.summary,
        "Noticeably quieter than this Show usually runs and uses mono rather than the usual stereo delivery."
    );

    // 2. Critical Non-Interference Invariant:
    // PodReady Assessment MUST independently evaluate Mono against podcast-mono-v1 target (-19.0 LUFS)
    // and MUST NOT use the Show's -16.0 LUFS baseline!
    let assessment = candidate.assessment.as_ref().unwrap();
    assert_eq!(assessment.profile_id, "podcast-mono-v1");
    let fix_plan = crate::fixplan::generate_fix_plan(assessment);
    let l_action = fix_plan.actions.iter().find(|a| a.source_check_id == "loudness").unwrap();
    assert_eq!(l_action.to_value.as_deref(), Some("−19.0 LUFS target"));
}

#[test]
fn test_show_check_delivery_format_different_alone_is_different() {
    let service = CatalogueService::new_in_memory().expect("db");
    let show = service.create_show("DeliveryShow", None).expect("show");

    for i in 0..5 {
        let mut media = create_test_media_source(&format!("/test/ep{}.mp3", i), 60.0, -16.0, -1.5);
        media.format = MediaFormat::MP3;
        media.codec = "mp3".to_string();
        service.add_media_source_to_show(&show.id, &media).expect("add");
    }

    // Candidate matches acoustics but differs in format (WAV vs MP3)
    let mut cand = create_test_media_source("/test/cand.wav", 60.0, -16.0, -1.5);
    cand.format = MediaFormat::WAV;

    let check = service.run_show_check_for_media(&show.id, &cand).expect("check");
    assert_eq!(check.status, ShowCheckStatus::Different);
    assert_eq!(check.summary, "Uses WAV rather than the usual MP3 format.");
}



