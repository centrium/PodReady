use crate::assessment::engine::assess_media;
use crate::assessment::profiles::{PODCAST_MONO_V1, PODCAST_STEREO_V1};
use crate::batch::publishing::*;
use crate::catalogue::models::SourceAvailability;
use crate::catalogue::service::CatalogueService;
use crate::export::publish::publish_single_episode;
use crate::export::types::{EpisodeMetadata, ExportOptions};
use crate::fixplan::engine::generate_fix_plan;
use crate::media::binaries::ffmpeg_cmd;
use crate::media::ffprobe::{inspect_media, MediaSource};
use std::fs::File;
use std::io::Read;
use std::path::Path;

fn generate_sine_fixture(path: &Path, duration_sec: f64, vol: &str, channels: u32) {
    let out = ffmpeg_cmd()
        .unwrap()
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=f=440:d={}:sample_rate=44100", duration_sec),
            "-ac",
            &channels.to_string(),
            "-af",
            &format!("volume={}", vol),
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ffmpeg for test fixture");

    assert!(
        out.status.success(),
        "Failed to generate test fixture: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}


fn file_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path).expect("failed to open file for sha");
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("failed to read file");
    let mut hasher = Sha256::new();
    hasher.update(&buf);
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn test_single_available_episode_publishes_successfully() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_pub_test_1_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let audio_path = temp_dir.join("Episode_01.wav");
    generate_sine_fixture(&audio_path, 1.0, "0.3", 2);

    let cat_service = CatalogueService::new_in_memory().unwrap();
    let show = cat_service.create_show("Tech Wave", None).unwrap();

    let insp = inspect_media(audio_path.to_str().unwrap()).unwrap();
    let meas = crate::media::analysis::analyse_audio(audio_path.to_str().unwrap(), 1.0).unwrap();
    let assess = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);
    let media = MediaSource {
        path: audio_path.to_str().unwrap().to_string(),
        filename: insp.filename.clone(),
        format: insp.format.clone(),
        codec: insp.codec.clone(),
        inspection: insp.inspection.clone(),
        measurements: Some(meas.clone()),
        assessment: Some(assess.clone()),
    };
    let outcome = cat_service
        .add_media_source_to_show(&show.id, &media)
        .unwrap();

    let dest_dir = temp_dir.join("Tech_Wave_PodReady");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let input_episodes = vec![BatchPublishingEpisodeInput {
        id: outcome.episode_id.clone(),
        source_path: audio_path.to_str().unwrap().to_string(),
        filename: "Episode_01.wav".to_string(),
        source_availability: SourceAvailability::Available,
    }];

    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some(show.id.clone()),
            Some(show.name.clone()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            Some(cat_service),
            |_| {},
        )
        .unwrap();

    // Poll until complete
    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.total, 1);
    assert_eq!(completed_job.summary.complete, 1);
    assert_eq!(completed_job.summary.failed, 0);
    assert_eq!(completed_job.summary.skipped, 0);

    // Verify package directory structure
    let package_dir = dest_dir.join("Episode_01_PodReady");
    assert!(package_dir.exists(), "Package directory should exist");
    assert!(package_dir.join("Episode_01_ready.mp3").exists());
    assert!(package_dir.join("Episode_01_report.json").exists());
    assert!(dest_dir.join("podready_batch_manifest.json").exists());
}

#[tokio::test]
async fn test_three_episodes_publish_serially() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_pub_test_3_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let cat_service = CatalogueService::new_in_memory().unwrap();
    let show = cat_service.create_show("Tri-Show", None).unwrap();

    let mut input_episodes = Vec::new();
    for i in 1..=3 {
        let p = temp_dir.join(format!("Ep_{:02}.wav", i));
        generate_sine_fixture(&p, 0.5, "0.2", 2);
        let insp = inspect_media(p.to_str().unwrap()).unwrap();
        let meas = crate::media::analysis::analyse_audio(p.to_str().unwrap(), 0.5).unwrap();
        let assess = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);
        let media = MediaSource {
            path: p.to_str().unwrap().to_string(),
            filename: insp.filename.clone(),
            format: insp.format.clone(),
            codec: insp.codec.clone(),
            inspection: insp.inspection.clone(),
            measurements: Some(meas.clone()),
            assessment: Some(assess.clone()),
        };
        let out = cat_service
            .add_media_source_to_show(&show.id, &media)
            .unwrap();

        input_episodes.push(BatchPublishingEpisodeInput {
            id: out.episode_id,
            source_path: p.to_str().unwrap().to_string(),
            filename: format!("Ep_{:02}.wav", i),
            source_availability: SourceAvailability::Available,
        });
    }

    let dest_dir = temp_dir.join("Tri_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some(show.id.clone()),
            Some(show.name.clone()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            Some(cat_service),
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.complete, 3);
    assert_eq!(completed_job.summary.failed, 0);

    for i in 1..=3 {
        let pkg_dir = dest_dir.join(format!("Ep_{:02}_PodReady", i));
        assert!(pkg_dir.exists());
        assert!(pkg_dir.join(format!("Ep_{:02}_ready.mp3", i)).exists());
        assert!(pkg_dir.join(format!("Ep_{:02}_report.json", i)).exists());
    }
}

#[tokio::test]
async fn test_missing_source_skipped_and_batch_continues() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_pub_missing_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let cat_service = CatalogueService::new_in_memory().unwrap();
    let show = cat_service.create_show("Resilient Show", None).unwrap();

    let p1 = temp_dir.join("Ep_01.wav");
    generate_sine_fixture(&p1, 0.5, "0.3", 2);

    let p2_nonexistent = temp_dir.join("Ep_02_Deleted.wav");

    let p3 = temp_dir.join("Ep_03.wav");
    generate_sine_fixture(&p3, 0.5, "0.3", 2);

    let input_episodes = vec![
        BatchPublishingEpisodeInput {
            id: "ep-1".into(),
            source_path: p1.to_str().unwrap().to_string(),
            filename: "Ep_01.wav".into(),
            source_availability: SourceAvailability::Available,
        },
        BatchPublishingEpisodeInput {
            id: "ep-2".into(),
            source_path: p2_nonexistent.to_str().unwrap().to_string(),
            filename: "Ep_02_Deleted.wav".into(),
            source_availability: SourceAvailability::Missing,
        },
        BatchPublishingEpisodeInput {
            id: "ep-3".into(),
            source_path: p3.to_str().unwrap().to_string(),
            filename: "Ep_03.wav".into(),
            source_availability: SourceAvailability::Available,
        },
    ];

    let dest_dir = temp_dir.join("Resilient_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some(show.id.clone()),
            Some(show.name.clone()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            Some(cat_service),
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.complete, 2);
    assert_eq!(completed_job.summary.skipped, 1);
    assert_eq!(completed_job.summary.failed, 0);

    assert_eq!(completed_job.episodes[1].status, PublishingEpisodeStatus::Skipped);
    assert_eq!(
        completed_job.episodes[1].skip_reason.as_deref(),
        Some("Source file unavailable")
    );
}

#[tokio::test]
async fn test_changed_source_reanalysed_before_publishing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_pub_changed_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let p = temp_dir.join("Changed_Episode.wav");
    // Initially quiet
    generate_sine_fixture(&p, 0.5, "0.05", 2);

    let cat_service = CatalogueService::new_in_memory().unwrap();
    let show = cat_service.create_show("Dynamic Show", None).unwrap();

    let insp = inspect_media(p.to_str().unwrap()).unwrap();
    let meas = crate::media::analysis::analyse_audio(p.to_str().unwrap(), 0.5).unwrap();
    let assess = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);
    let media = MediaSource {
        path: p.to_str().unwrap().to_string(),
        filename: insp.filename.clone(),
        format: insp.format.clone(),
        codec: insp.codec.clone(),
        inspection: insp.inspection.clone(),
        measurements: Some(meas.clone()),
        assessment: Some(assess.clone()),
    };
    let outcome = cat_service
        .add_media_source_to_show(&show.id, &media)
        .unwrap();

    // Now modify the source on disk (make it loud)
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    generate_sine_fixture(&p, 0.5, "0.9", 2);

    let input_episodes = vec![BatchPublishingEpisodeInput {
        id: outcome.episode_id.clone(),
        source_path: p.to_str().unwrap().to_string(),
        filename: "Changed_Episode.wav".into(),
        source_availability: SourceAvailability::Changed,
    }];


    let dest_dir = temp_dir.join("Changed_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some(show.id.clone()),
            Some(show.name.clone()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            Some(cat_service),
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.complete, 1);
    assert_eq!(completed_job.episodes[0].reanalysed, Some(true));
}

#[tokio::test]
async fn test_show_check_does_not_alter_fixplan_target() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_show_check_indep_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create a quiet stereo audio file (~ -25 LUFS)
    let candidate_path = temp_dir.join("quiet_stereo.wav");
    generate_sine_fixture(&candidate_path, 1.0, "0.05", 2);

    let insp = inspect_media(candidate_path.to_str().unwrap()).unwrap();
    let meas = crate::media::analysis::analyse_audio(candidate_path.to_str().unwrap(), 1.0).unwrap();
    let assessment = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);

    // Generate FixPlan
    let plan = generate_fix_plan(&assessment);

    // Assert that the FixPlan strictly targets the stereo standard target (-16 LUFS)
    assert!(plan.changes_audio);
    let loudness_action = plan
        .actions
        .iter()
        .find(|a| a.action_type == crate::fixplan::engine::FixActionType::LoudnessAdjustment)
        .expect("Loudness adjustment must be planned");

    let expected_stereo_target = format!("−{:.1} LUFS target", PODCAST_STEREO_V1.loudness.target_lufs.abs());
    assert_eq!(
        loudness_action.to_value.as_deref(),
        Some(expected_stereo_target.as_str())
    );
}

#[tokio::test]
async fn test_mono_episode_in_stereo_show_uses_mono_publishing_target() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_mono_target_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create mono audio file (1 channel)
    let mono_path = temp_dir.join("mono_ep.wav");
    generate_sine_fixture(&mono_path, 1.0, "0.1", 1);

    let insp = inspect_media(mono_path.to_str().unwrap()).unwrap();
    assert_eq!(insp.inspection.channels, 1);

    let meas = crate::media::analysis::analyse_audio(mono_path.to_str().unwrap(), 1.0).unwrap();
    let assessment = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);

    assert_eq!(assessment.profile_id, "podcast-mono-v1");

    let plan = generate_fix_plan(&assessment);
    let loudness_action = plan
        .actions
        .iter()
        .find(|a| a.action_type == crate::fixplan::engine::FixActionType::LoudnessAdjustment)
        .expect("Loudness adjustment must be planned");

    let expected_mono_target = format!("−{:.1} LUFS target", PODCAST_MONO_V1.loudness.target_lufs.abs());
    assert_eq!(
        loudness_action.to_value.as_deref(),
        Some(expected_mono_target.as_str())
    );
}



#[tokio::test]
async fn test_cancellation_retains_completed_packages() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_pub_cancel_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let p1 = temp_dir.join("Ep_01.wav");
    generate_sine_fixture(&p1, 0.5, "0.3", 2);

    let p2 = temp_dir.join("Ep_02.wav");
    generate_sine_fixture(&p2, 0.5, "0.3", 2);

    let input_episodes = vec![
        BatchPublishingEpisodeInput {
            id: "ep-1".into(),
            source_path: p1.to_str().unwrap().to_string(),
            filename: "Ep_01.wav".into(),
            source_availability: SourceAvailability::Available,
        },
        BatchPublishingEpisodeInput {
            id: "ep-2".into(),
            source_path: p2.to_str().unwrap().to_string(),
            filename: "Ep_02.wav".into(),
            source_availability: SourceAvailability::Available,
        },
    ];

    let dest_dir = temp_dir.join("Cancel_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some("show-1".into()),
            Some("Cancel Show".into()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            None,
            |_| {},
        )
        .unwrap();

    // Cancel immediately
    let _ = pub_mgr.cancel_job(&job.id);

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let final_job = pub_mgr.get_job(&job.id).unwrap();
    assert_eq!(final_job.status, BatchPublishingJobStatus::Cancelled);
}

#[tokio::test]
async fn test_source_bytes_remain_strictly_unchanged() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_source_safety_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let audio_path = temp_dir.join("Source_Safe.wav");
    generate_sine_fixture(&audio_path, 1.0, "0.3", 2);

    let sha_before = file_sha256(&audio_path);

    let dest_dir = temp_dir.join("Safe_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let opts = ExportOptions {
        destination_directory: dest_dir_str,
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let result = publish_single_episode(
        audio_path.to_str().unwrap(),
        opts.destination_directory.as_str(),
        &opts,
        None,
        None,
        None,
        None,
        None,
    );

    assert!(result.is_ok());

    let sha_after = file_sha256(&audio_path);
    assert_eq!(
        sha_before, sha_after,
        "Source file bytes MUST be strictly identical before and after publishing"
    );
}

#[tokio::test]
async fn test_single_catalogue_publish_matches_workspace_publish_semantics() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_equiv_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let source_path = temp_dir.join("Equiv_Episode.wav");
    generate_sine_fixture(&source_path, 1.0, "0.3", 2);

    let dest_dir = temp_dir.join("Equiv_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let opts = ExportOptions {
        destination_directory: dest_dir_str,
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: Some(EpisodeMetadata {
            title: Some("Equiv Ep".into()),
            artist: Some("PodReady".into()),
            album: None,
            episode_number: Some("1".into()),
            year: Some("2026".into()),
            genre: None,
            artwork_path: None,
        }),
    };

    let pkg = publish_single_episode(
        source_path.to_str().unwrap(),
        &opts.destination_directory,
        &opts,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Publish single episode should succeed");

    assert!(pkg.audio_file.is_some());
    assert!(pkg.report_file.is_some());
    assert_eq!(pkg.package_name, "Equiv_Episode_PodReady");
    assert!(pkg.verification_result.passed);
}

#[tokio::test]
async fn test_whole_show_selection_publishes_all_eligible() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_whole_show_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let cat_service = CatalogueService::new_in_memory().unwrap();
    let show = cat_service.create_show("Whole Show", None).unwrap();

    let mut input_episodes = Vec::new();
    for i in 1..=4 {
        let p = temp_dir.join(format!("Ep_{:02}.wav", i));
        if i == 4 {
            // Missing episode
            input_episodes.push(BatchPublishingEpisodeInput {
                id: format!("ep-{}", i),
                source_path: p.to_str().unwrap().to_string(),
                filename: format!("Ep_{:02}.wav", i),
                source_availability: SourceAvailability::Missing,
            });
        } else {
            generate_sine_fixture(&p, 0.5, "0.2", 2);
            let insp = inspect_media(p.to_str().unwrap()).unwrap();
            let meas = crate::media::analysis::analyse_audio(p.to_str().unwrap(), 0.5).unwrap();
            let assess = assess_media(&insp.inspection, Some(&meas), &insp.format, &insp.codec);
            let media = MediaSource {
                path: p.to_str().unwrap().to_string(),
                filename: insp.filename.clone(),
                format: insp.format.clone(),
                codec: insp.codec.clone(),
                inspection: insp.inspection.clone(),
                measurements: Some(meas.clone()),
                assessment: Some(assess.clone()),
            };
            let out = cat_service.add_media_source_to_show(&show.id, &media).unwrap();

            input_episodes.push(BatchPublishingEpisodeInput {
                id: out.episode_id,
                source_path: p.to_str().unwrap().to_string(),
                filename: format!("Ep_{:02}.wav", i),
                source_availability: SourceAvailability::Available,
            });
        }
    }

    let dest_dir = temp_dir.join("Whole_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some(show.id.clone()),
            Some(show.name.clone()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            Some(cat_service),
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.total, 4);
    assert_eq!(completed_job.summary.complete, 3);
    assert_eq!(completed_job.summary.skipped, 1);
}

#[tokio::test]
async fn test_one_processing_failure_does_not_stop_remaining_episodes() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_fail_isolation_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let p1 = temp_dir.join("Ep_01_Good.wav");
    generate_sine_fixture(&p1, 0.5, "0.2", 2);

    let p2_corrupt = temp_dir.join("Ep_02_Corrupt.wav");
    std::fs::write(&p2_corrupt, b"NOT_A_VALID_WAV_HEADER_CORRUPTED").unwrap();

    let p3 = temp_dir.join("Ep_03_Good.wav");
    generate_sine_fixture(&p3, 0.5, "0.2", 2);

    let input_episodes = vec![
        BatchPublishingEpisodeInput {
            id: "ep-1".into(),
            source_path: p1.to_str().unwrap().to_string(),
            filename: "Ep_01_Good.wav".into(),
            source_availability: SourceAvailability::Available,
        },
        BatchPublishingEpisodeInput {
            id: "ep-2".into(),
            source_path: p2_corrupt.to_str().unwrap().to_string(),
            filename: "Ep_02_Corrupt.wav".into(),
            source_availability: SourceAvailability::Available,
        },
        BatchPublishingEpisodeInput {
            id: "ep-3".into(),
            source_path: p3.to_str().unwrap().to_string(),
            filename: "Ep_03_Good.wav".into(),
            source_availability: SourceAvailability::Available,
        },
    ];

    let dest_dir = temp_dir.join("Isolation_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some("show-isolation".into()),
            Some("Isolation Show".into()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            None,
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert_eq!(completed_job.summary.complete, 2);
    assert_eq!(completed_job.summary.failed, 1);

    // Ep 1 and Ep 3 packages exist
    assert!(dest_dir.join("Ep_01_Good_PodReady").exists());
    assert!(dest_dir.join("Ep_03_Good_PodReady").exists());
}

#[tokio::test]
async fn test_batch_total_timing_populated() {
    let temp_dir = std::env::temp_dir().join(format!(
        "podready_timing_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let p1 = temp_dir.join("Ep_Timing.wav");
    generate_sine_fixture(&p1, 0.5, "0.3", 2);

    let input_episodes = vec![BatchPublishingEpisodeInput {
        id: "ep-time".into(),
        source_path: p1.to_str().unwrap().to_string(),
        filename: "Ep_Timing.wav".into(),
        source_availability: SourceAvailability::Available,
    }];


    let dest_dir = temp_dir.join("Timing_Export");
    let dest_dir_str = dest_dir.to_str().unwrap().to_string();

    let pub_mgr = BatchPublishingManager::new();
    let opts = ExportOptions {
        destination_directory: dest_dir_str.clone(),
        include_audio: true,
        include_transcript: false,
        include_report: true,
        metadata: None,
    };

    let job = pub_mgr
        .start_job(
            Some("show-timing".into()),
            Some("Timing Show".into()),
            input_episodes,
            dest_dir_str.clone(),
            opts,
            None,
            |_| {},
        )
        .unwrap();

    let mut completed_job = job;
    for _ in 0..100 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let current = pub_mgr.get_job(&completed_job.id).unwrap();
        if current.status == BatchPublishingJobStatus::Complete {
            completed_job = current;
            break;
        }
    }

    assert_eq!(completed_job.status, BatchPublishingJobStatus::Complete);
    assert!(completed_job.summary.elapsed_seconds > 0.0);
    assert!(completed_job.episodes[0].elapsed_seconds.unwrap_or(0.0) > 0.0);
}

