pub mod engine;
pub mod model;

use crate::error::AppError;
use engine::{create_batch_job, run_batch_job, BatchJobHandle, DEFAULT_CONCURRENCY};
pub use model::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter};

pub struct BatchManager {
    jobs: Arc<RwLock<HashMap<String, Arc<BatchJobHandle>>>>,
}

impl BatchManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_job<F>(
        &self,
        paths: &[String],
        concurrency: Option<usize>,
        on_progress: F,
    ) -> Result<BatchAnalysisJob, AppError>
    where
        F: Fn(BatchProgressPayload) + Send + Sync + 'static,
    {
        let job = create_batch_job(paths);
        let job_id = job.id.clone();
        let job_arc = Arc::new(RwLock::new(job.clone()));
        let handle = Arc::new(BatchJobHandle {
            id: job_id.clone(),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            active_pids: Arc::new(Mutex::new(HashSet::new())),
            job: job_arc,
        });

        {
            let mut guard = self.jobs.write().unwrap();
            guard.insert(job_id, handle.clone());
        }

        let conc = concurrency.unwrap_or(DEFAULT_CONCURRENCY);
        let handle_clone = handle.clone();
        tauri::async_runtime::spawn(async move {
            run_batch_job(handle_clone, conc, on_progress).await;
        });

        Ok(job)
    }

    pub fn cancel_job(&self, job_id: &str) -> Result<(), AppError> {
        let guard = self.jobs.read().unwrap();
        if let Some(handle) = guard.get(job_id) {
            handle.cancel();
            Ok(())
        } else {
            Err(AppError::SystemError("Batch job not found".into()))
        }
    }

    pub fn get_job(&self, job_id: &str) -> Result<BatchAnalysisJob, AppError> {
        let guard = self.jobs.read().unwrap();
        if let Some(handle) = guard.get(job_id) {
            let job_read = handle.job.read().unwrap();
            Ok(job_read.clone())
        } else {
            Err(AppError::SystemError("Batch job not found".into()))
        }
    }
}

#[tauri::command]
pub async fn start_batch_analysis_cmd(
    paths: Vec<String>,
    app: AppHandle,
    state: tauri::State<'_, BatchManager>,
) -> Result<BatchAnalysisJob, AppError> {
    let app_handle = app.clone();
    state.start_job(&paths, None, move |payload| {
        let _ = app_handle.emit("batch-progress", &payload);
        if payload.status == BatchEpisodeStatus::Complete
            || payload.status == BatchEpisodeStatus::Failed
            || payload.status == BatchEpisodeStatus::Cancelled
        {
            if payload.summary.complete + payload.summary.failed + payload.summary.cancelled
                >= payload.summary.total
            {
                let _ = app_handle.emit("batch-complete", &payload);
            }
        }
    })
}

#[tauri::command]
pub fn cancel_batch_analysis_cmd(
    job_id: String,
    state: tauri::State<'_, BatchManager>,
) -> Result<(), AppError> {
    state.cancel_job(&job_id)
}

#[tauri::command]
pub fn get_batch_job_cmd(
    job_id: String,
    state: tauri::State<'_, BatchManager>,
) -> Result<BatchAnalysisJob, AppError> {
    state.get_job(&job_id)
}

#[tauri::command]
pub async fn select_files_cmd() -> Result<Vec<String>, AppError> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "macos")]
        {
            let script = r#"
            set chosenFiles to choose file with prompt "Select Podcast Audio Episodes" of type {"wav", "wave", "mp3", "m4a", "mp4", "mov", "public.audio"} with multiple selections allowed
            set outPaths to {}
            repeat with aFile in chosenFiles
                set end of outPaths to POSIX path of aFile
            end repeat
            set AppleScript's text item delimiters to ASCII character 10
            outPaths as text
            "#;

            let output = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()
                .map_err(|e| AppError::SystemError(format!("Failed to open file dialog: {}", e)))?;

            if !output.status.success() {
                // User cancelled or dialog dismissed
                return Ok(Vec::new());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let paths: Vec<String> = stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            Ok(paths)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(Vec::new())
        }
    })
    .await
    .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::engine::OverallStatus;
    use crate::batch::engine::update_summary;
    use crate::media::binaries::ffmpeg_cmd;
    use crate::media::ffprobe::inspect_media;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    fn generate_sine_fixture(path: &Path, duration_sec: f64, vol: &str) {
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=f=1000:d={}:sample_rate=44100", duration_sec),
                "-af",
                &format!("volume={}", vol),
                path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to generate test audio fixture");
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
    async fn test_batch_queue_execution_and_bounded_concurrency() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut paths = Vec::new();
        for i in 1..=4 {
            let p = temp_dir.join(format!("ep_{:02}.wav", i));
            generate_sine_fixture(&p, 0.5, "0.5");
            paths.push(p.to_string_lossy().to_string());
        }

        let manager = BatchManager::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let job = manager
            .start_job(&paths, Some(2), move |payload| {
                events_clone.lock().unwrap().push(payload);
            })
            .expect("Failed to start batch job");

        assert_eq!(job.episodes.len(), 4);
        assert_eq!(job.summary.total, 4);

        // Wait for job completion
        let mut completed = false;
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                completed = true;
                assert_eq!(current.summary.complete, 4);
                assert_eq!(current.summary.failed, 0);
                assert_eq!(current.summary.cancelled, 0);
                assert!(current.summary.ready > 0 || current.summary.attention > 0 || current.summary.needs_attention > 0);
                assert!(current.summary.elapsed_seconds > 0.0);
                break;
            }
        }
        assert!(completed, "Batch job did not complete in time");

        let recorded_events = events.lock().unwrap();
        assert!(!recorded_events.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_batch_failure_isolation() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_fail_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let good_1 = temp_dir.join("ep1_good.wav");
        generate_sine_fixture(&good_1, 0.5, "0.5");

        let bad_file = temp_dir.join("ep2_corrupt.wav");
        std::fs::write(&bad_file, b"NOT_A_VALID_AUDIO_FILE_DATA_HERE").unwrap();

        let good_2 = temp_dir.join("ep3_good.wav");
        generate_sine_fixture(&good_2, 0.5, "0.5");

        let paths = vec![
            good_1.to_string_lossy().to_string(),
            bad_file.to_string_lossy().to_string(),
            good_2.to_string_lossy().to_string(),
        ];

        let manager = BatchManager::new();
        let job = manager
            .start_job(&paths, Some(2), |_| {})
            .expect("Failed to start batch job");

        let mut completed = false;
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                completed = true;
                assert_eq!(current.summary.total, 3);
                assert_eq!(current.summary.complete, 2);
                assert_eq!(current.summary.failed, 1);
                assert_eq!(current.episodes[1].status, BatchEpisodeStatus::Failed);
                assert!(current.episodes[1].error.is_some());
                assert_eq!(current.episodes[0].status, BatchEpisodeStatus::Complete);
                assert_eq!(current.episodes[2].status, BatchEpisodeStatus::Complete);
                break;
            }
        }
        assert!(completed, "Failure isolation batch test did not complete");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_batch_cancellation() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_cancel_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut paths = Vec::new();
        for i in 1..=6 {
            let p = temp_dir.join(format!("ep_cancel_{:02}.wav", i));
            generate_sine_fixture(&p, 1.5, "0.5");
            paths.push(p.to_string_lossy().to_string());
        }

        let manager = BatchManager::new();
        let job = manager
            .start_job(&paths, Some(1), |_| {})
            .expect("Failed to start batch job");

        // Give it a brief moment to start the first episode, then cancel immediately
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        manager.cancel_job(&job.id).expect("Failed to cancel job");

        // Wait for status to reflect Cancelled
        let mut reached_cancellation = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Cancelled {
                reached_cancellation = true;
                assert!(current.summary.cancelled > 0);
                assert_eq!(
                    current.summary.complete + current.summary.failed + current.summary.cancelled,
                    current.summary.total
                );
                break;
            }
        }
        assert!(reached_cancellation, "Job was not marked Cancelled");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_batch_path_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_dedup_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file_1 = temp_dir.join("ep1.wav");
        std::fs::write(&file_1, b"dummy").unwrap();

        let paths = vec![
            file_1.to_string_lossy().to_string(),
            file_1.to_string_lossy().to_string(),
            file_1.to_string_lossy().to_string(),
        ];

        let job = create_batch_job(&paths);
        assert_eq!(job.episodes.len(), 1, "Duplicate paths in the same batch must be deduplicated");
        assert_eq!(job.summary.total, 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_batch_source_integrity() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_integrity_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let p1 = temp_dir.join("ep_integrity.wav");
        generate_sine_fixture(&p1, 0.5, "0.5");

        let hash_before = file_sha256(&p1);
        let mtime_before = std::fs::metadata(&p1).unwrap().modified().unwrap();

        let manager = BatchManager::new();
        let job = manager
            .start_job(&[p1.to_string_lossy().to_string()], Some(1), |_| {})
            .expect("Failed to start batch job");

        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                break;
            }
        }

        let hash_after = file_sha256(&p1);
        let mtime_after = std::fs::metadata(&p1).unwrap().modified().unwrap();

        assert_eq!(hash_before, hash_after, "Source file SHA-256 hash must be identical after batch analysis");
        assert_eq!(mtime_before, mtime_after, "Source file modified time must be unchanged");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_batch_assessment_parity() {
        let temp_dir = std::env::temp_dir().join(format!("podready_batch_parity_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let fixture = temp_dir.join("ep_parity.wav");
        generate_sine_fixture(&fixture, 1.0, "0.5");

        // 1. Single-file direct execution
        let single_inspect = inspect_media(&fixture).unwrap();
        let single_measure = crate::media::analysis::analyse_audio(&fixture, single_inspect.inspection.duration_seconds).unwrap();
        let single_assess = crate::assessment::assess_media(&single_inspect.inspection, Some(&single_measure), &single_inspect.format, &single_inspect.codec);

        // 2. Batch engine execution
        let manager = BatchManager::new();
        let job = manager
            .start_job(&[fixture.to_string_lossy().to_string()], Some(1), |_| {})
            .unwrap();

        let mut batch_episode = None;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                batch_episode = Some(current.episodes[0].clone());
                break;
            }
        }

        let ep = batch_episode.expect("Batch episode did not complete");
        let batch_measure = ep.measurements.unwrap();
        let batch_assess = ep.assessment.unwrap();

        assert_eq!(batch_measure.integrated_loudness_lufs, single_measure.integrated_loudness_lufs);
        assert_eq!(batch_measure.true_peak_dbtp, single_measure.true_peak_dbtp);
        assert_eq!(batch_assess.overall_status, single_assess.overall_status);
        assert_eq!(batch_assess.summary, single_assess.summary);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_batch_summary_aggregation() {
        let mut job = BatchAnalysisJob {
            id: "test-job".into(),
            status: BatchJobStatus::Running,
            episodes: vec![
                BatchEpisode {
                    id: "ep1".into(),
                    source_path: "ep1.wav".into(),
                    filename: "ep1.wav".into(),
                    status: BatchEpisodeStatus::Complete,
                    format: None,
                    codec: None,
                    inspection: None,
                    measurements: None,
                    assessment: Some(crate::assessment::engine::Assessment {
                        overall_status: OverallStatus::Ready,
                        summary: "Ready".into(),
                        profile_id: "stereo".into(),
                        profile_version: "1.0".into(),
                        profile_name: "Stereo".into(),
                        audio_checks: vec![],
                        file_checks: vec![],
                    }),
                    duration_seconds: Some(60.0),
                    elapsed_seconds: Some(1.2),
                    error: None,
                },
                BatchEpisode {
                    id: "ep2".into(),
                    source_path: "ep2.wav".into(),
                    filename: "ep2.wav".into(),
                    status: BatchEpisodeStatus::Complete,
                    format: None,
                    codec: None,
                    inspection: None,
                    measurements: None,
                    assessment: Some(crate::assessment::engine::Assessment {
                        overall_status: OverallStatus::Ready,
                        summary: "Ready".into(),
                        profile_id: "stereo".into(),
                        profile_version: "1.0".into(),
                        profile_name: "Stereo".into(),
                        audio_checks: vec![],
                        file_checks: vec![],
                    }),
                    duration_seconds: Some(60.0),
                    elapsed_seconds: Some(1.1),
                    error: None,
                },
                BatchEpisode {
                    id: "ep3".into(),
                    source_path: "ep3.wav".into(),
                    filename: "ep3.wav".into(),
                    status: BatchEpisodeStatus::Complete,
                    format: None,
                    codec: None,
                    inspection: None,
                    measurements: None,
                    assessment: Some(crate::assessment::engine::Assessment {
                        overall_status: OverallStatus::Attention,
                        summary: "Attention".into(),
                        profile_id: "stereo".into(),
                        profile_version: "1.0".into(),
                        profile_name: "Stereo".into(),
                        audio_checks: vec![],
                        file_checks: vec![],
                    }),
                    duration_seconds: Some(60.0),
                    elapsed_seconds: Some(1.3),
                    error: None,
                },
                BatchEpisode {
                    id: "ep4".into(),
                    source_path: "ep4.wav".into(),
                    filename: "ep4.wav".into(),
                    status: BatchEpisodeStatus::Complete,
                    format: None,
                    codec: None,
                    inspection: None,
                    measurements: None,
                    assessment: Some(crate::assessment::engine::Assessment {
                        overall_status: OverallStatus::NeedsAttention,
                        summary: "Needs Attention".into(),
                        profile_id: "stereo".into(),
                        profile_version: "1.0".into(),
                        profile_name: "Stereo".into(),
                        audio_checks: vec![],
                        file_checks: vec![],
                    }),
                    duration_seconds: Some(60.0),
                    elapsed_seconds: Some(1.5),
                    error: None,
                },
                BatchEpisode {
                    id: "ep5".into(),
                    source_path: "ep5.wav".into(),
                    filename: "ep5.wav".into(),
                    status: BatchEpisodeStatus::Failed,
                    format: None,
                    codec: None,
                    inspection: None,
                    measurements: None,
                    assessment: None,
                    duration_seconds: None,
                    elapsed_seconds: None,
                    error: Some("Failed".into()),
                },
            ],
            summary: BatchAnalysisSummary::default(),
            created_at: "2026-08-18T12:00:00Z".into(),
        };

        update_summary(&mut job, 18.4);

        assert_eq!(job.summary.total, 5);
        assert_eq!(job.summary.complete, 4);
        assert_eq!(job.summary.failed, 1);
        assert_eq!(job.summary.cancelled, 0);
        assert_eq!(job.summary.ready, 2);
        assert_eq!(job.summary.attention, 1);
        assert_eq!(job.summary.needs_attention, 1);
        assert_eq!(job.summary.elapsed_seconds, 18.4);
    }

    #[tokio::test]
    async fn test_batch_10_episodes_performance() {
        let temp_dir = std::env::temp_dir().join(format!(
            "podready_batch_perf_10_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut paths = Vec::new();
        for i in 1..=10 {
            let p = temp_dir.join(format!("episode_{:02}.wav", i));
            let vol = if i % 2 == 0 { "0.5" } else { "0.8" };
            generate_sine_fixture(&p, 0.3, vol);
            paths.push(p.to_string_lossy().to_string());
        }

        let start = std::time::Instant::now();
        let manager = BatchManager::new();
        let job = manager
            .start_job(&paths, Some(2), |_| {})
            .expect("Failed to start 10 episode batch");

        let mut completed = false;
        for _ in 0..150 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                completed = true;
                let wall_elapsed = start.elapsed().as_secs_f64();
                println!(
                    "10 episodes batch analysis completed in {:.2}s (reported: {:.1}s)",
                    wall_elapsed, current.summary.elapsed_seconds
                );
                assert_eq!(current.summary.total, 10);
                assert_eq!(current.summary.complete, 10);
                assert_eq!(current.summary.failed, 0);
                assert!(current.summary.ready + current.summary.attention + current.summary.needs_attention == 10);
                break;
            }
        }
        assert!(completed, "10 episode batch did not complete in time");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_batch_25_episodes_performance() {
        let temp_dir = std::env::temp_dir().join(format!(
            "podready_batch_perf_25_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut paths = Vec::new();
        for i in 1..=25 {
            let p = temp_dir.join(format!("episode_{:02}.wav", i));
            let vol = if i % 3 == 0 { "0.3" } else if i % 3 == 1 { "0.6" } else { "0.9" };
            generate_sine_fixture(&p, 0.2, vol);
            paths.push(p.to_string_lossy().to_string());
        }

        let start = std::time::Instant::now();
        let manager = BatchManager::new();
        let job = manager
            .start_job(&paths, Some(2), |_| {})
            .expect("Failed to start 25 episode batch");

        let mut completed = false;
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Complete {
                completed = true;
                let wall_elapsed = start.elapsed().as_secs_f64();
                println!(
                    "25 episodes batch analysis completed in {:.2}s (reported: {:.1}s)",
                    wall_elapsed, current.summary.elapsed_seconds
                );
                assert_eq!(current.summary.total, 25);
                assert_eq!(current.summary.complete, 25);
                assert_eq!(current.summary.failed, 0);
                assert!(current.summary.ready + current.summary.attention + current.summary.needs_attention == 25);
                break;
            }
        }
        assert!(completed, "25 episode batch did not complete in time");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn generate_real_podcast_fixture(path: &Path, duration_sec: f64, gain_db: &str, stereo: bool) {
        let channels = if stereo { "2" } else { "1" };
        let _ = ffmpeg_cmd().unwrap()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!(
                    "sine=f=440:d={}:sample_rate=44100,volume={}dB,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts={}",
                    duration_sec,
                    gain_db,
                    if stereo { "stereo" } else { "mono" }
                ),
                "-ac",
                channels,
                path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to generate real podcast fixture");
    }

    #[tokio::test]
    async fn test_stage_5a_real_duration_podcast_batch_validation() {
        let temp_dir = std::env::temp_dir().join(format!(
            "podready_real_podcast_batch_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 5 real podcast episodes with durations in minutes (90s = 1.5m, 120s = 2.0m, 150s = 2.5m, 180s = 3.0m, 240s = 4.0m)
        let episode_specs = vec![
            ("podcast_ep01_intro.wav", 90.0, "-16", true),
            ("podcast_ep02_interview.mp3", 120.0, "-14", false),
            ("podcast_ep03_deepdive.wav", 150.0, "-19", true),
            ("podcast_ep04_panel.mp3", 180.0, "-15", true),
            ("podcast_ep05_finale.wav", 240.0, "-16", true),
        ];

        let mut paths = Vec::new();
        let mut hashes_before = HashMap::new();

        for (name, dur, gain, stereo) in &episode_specs {
            let p = temp_dir.join(name);
            generate_real_podcast_fixture(&p, *dur, gain, *stereo);
            let hash = file_sha256(&p);
            hashes_before.insert(p.to_string_lossy().to_string(), hash);
            paths.push(p.to_string_lossy().to_string());
        }

        let manager = BatchManager::new();
        let progress_events = Arc::new(Mutex::new(Vec::new()));
        let progress_events_clone = progress_events.clone();

        let start_time = std::time::Instant::now();
        let job = manager
            .start_job(&paths, Some(2), move |payload| {
                progress_events_clone.lock().unwrap().push((
                    std::time::Instant::now(),
                    payload.status,
                    payload.episode.filename.clone(),
                ));
            })
            .expect("Failed to start real podcast batch");

        let mut max_observed_concurrency = 0;
        let mut completed = false;

        for _ in 0..600 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let current = manager.get_job(&job.id).unwrap();

            let active_count = current
                .episodes
                .iter()
                .filter(|ep| {
                    ep.status == BatchEpisodeStatus::Inspecting
                        || ep.status == BatchEpisodeStatus::Analysing
                        || ep.status == BatchEpisodeStatus::Assessing
                })
                .count();

            if active_count > max_observed_concurrency {
                max_observed_concurrency = active_count;
            }

            if current.status == BatchJobStatus::Complete {
                completed = true;
                let wall_elapsed = start_time.elapsed().as_secs_f64();
                println!(
                    "[Real Podcast Batch] 5 episodes (total 13 minutes of audio) analysed in {:.2}s (wall clock), reported: {:.1}s",
                    wall_elapsed, current.summary.elapsed_seconds
                );
                assert_eq!(current.summary.total, 5);
                assert_eq!(current.summary.complete, 5);
                assert_eq!(current.summary.failed, 0);

                // 1. Verify durations are reported accurately and in minutes/seconds
                for (idx, (name, expected_dur, _, _)) in episode_specs.iter().enumerate() {
                    let ep = &current.episodes[idx];
                    assert_eq!(ep.filename, *name);
                    let actual_dur = ep.duration_seconds.expect("Duration must be present");
                    assert!(
                        (actual_dur - *expected_dur).abs() < 1.0,
                        "Episode {} duration was {}s, expected {}s",
                        name,
                        actual_dur,
                        expected_dur
                    );
                }

                // 2. Parity check: Compare episode 0 against single-file pipeline directly
                let ep0_path = &paths[0];
                let single_inspect = inspect_media(ep0_path).unwrap();
                let single_measure = crate::media::analysis::analyse_audio(
                    ep0_path,
                    single_inspect.inspection.duration_seconds,
                )
                .unwrap();
                let single_assess = crate::assessment::assess_media(
                    &single_inspect.inspection,
                    Some(&single_measure),
                    &single_inspect.format,
                    &single_inspect.codec,
                );

                let batch_ep0 = &current.episodes[0];
                assert_eq!(
                    batch_ep0.measurements.as_ref().unwrap().integrated_loudness_lufs,
                    single_measure.integrated_loudness_lufs
                );
                assert_eq!(
                    batch_ep0.measurements.as_ref().unwrap().true_peak_dbtp,
                    single_measure.true_peak_dbtp
                );
                assert_eq!(
                    batch_ep0.assessment.as_ref().unwrap().overall_status,
                    single_assess.overall_status
                );
                assert_eq!(
                    batch_ep0.assessment.as_ref().unwrap().summary,
                    single_assess.summary
                );

                break;
            }
        }

        assert!(completed, "Real podcast batch did not complete in time");
        assert!(
            max_observed_concurrency <= 2,
            "Concurrency exceeded limit of 2: observed {}",
            max_observed_concurrency
        );

        // 3. Verify incremental arrival of progress events
        let events = progress_events.lock().unwrap();
        assert!(events.len() >= 15, "Expected multiple incremental progress events");
        println!("[Real Podcast Batch] Received {} progress events across batch execution", events.len());

        // 4. Verify source integrity
        for path in &paths {
            let hash_after = file_sha256(Path::new(path));
            assert_eq!(
                hashes_before.get(path).unwrap(),
                &hash_after,
                "Source file {} was modified!",
                path
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_stage_5a_real_duration_cancellation_under_load() {
        let temp_dir = std::env::temp_dir().join(format!(
            "podready_real_cancel_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut paths = Vec::new();
        for i in 1..=4 {
            let p = temp_dir.join(format!("long_ep_{:02}.wav", i));
            generate_real_podcast_fixture(&p, 120.0, "-16", true);
            paths.push(p.to_string_lossy().to_string());
        }

        let manager = BatchManager::new();
        let job = manager
            .start_job(&paths, Some(2), |_| {})
            .expect("Failed to start batch");

        // Wait until 2 are actively analysing
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let current = manager.get_job(&job.id).unwrap();
            let active = current
                .episodes
                .iter()
                .filter(|ep| ep.status == BatchEpisodeStatus::Analysing)
                .count();
            if active >= 1 {
                break;
            }
        }

        // Cancel job
        manager.cancel_job(&job.id).expect("Cancel must succeed");

        let mut reached_cancellation = false;
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let current = manager.get_job(&job.id).unwrap();
            if current.status == BatchJobStatus::Cancelled {
                reached_cancellation = true;
                assert!(current.summary.cancelled > 0);
                assert_eq!(
                    current.summary.complete + current.summary.failed + current.summary.cancelled,
                    current.summary.total
                );
                break;
            }
        }
        assert!(reached_cancellation, "Job did not transition to Cancelled");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}


