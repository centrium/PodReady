use super::model::{
    BatchAnalysisJob, BatchAnalysisSummary, BatchEpisode, BatchEpisodeStatus, BatchJobStatus,
    BatchProgressPayload,
};
use crate::assessment::engine::{assess_media, OverallStatus};
use crate::error::AppError;
use crate::media::analysis::{evaluate_clipping_evidence, parse_ffmpeg_analysis, AudioMeasurements};
use crate::media::binaries::ffmpeg_cmd;
use crate::media::ffprobe::inspect_media;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

pub const DEFAULT_CONCURRENCY: usize = 2;

pub struct BatchJobHandle {
    pub id: String,
    pub is_cancelled: Arc<AtomicBool>,
    pub active_pids: Arc<Mutex<HashSet<u32>>>,
    pub job: Arc<RwLock<BatchAnalysisJob>>,
}

impl BatchJobHandle {
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::SeqCst);
        let pids: Vec<u32> = {
            let guard = self.active_pids.lock().unwrap();
            guard.iter().copied().collect()
        };
        for pid in pids {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output();
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .output();
            }
        }
    }
}

pub fn create_batch_job(paths: &[String]) -> BatchAnalysisJob {
    let job_id = format!("job-{}", uuid_simple());
    let mut seen_paths = HashSet::new();
    let mut episodes = Vec::new();

    for path_str in paths {
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            continue;
        }

        let path = Path::new(trimmed);
        // Path deduplication
        let canonical_key = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| trimmed.to_string());

        if !seen_paths.insert(canonical_key) {
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let episode_id = format!("ep-{}", uuid_simple());
        episodes.push(BatchEpisode {
            id: episode_id,
            source_path: trimmed.to_string(),
            filename,
            status: BatchEpisodeStatus::Waiting,
            format: None,
            codec: None,
            inspection: None,
            measurements: None,
            assessment: None,
            duration_seconds: None,
            elapsed_seconds: None,
            error: None,
        });
    }

    let total = episodes.len();
    BatchAnalysisJob {
        id: job_id,
        status: BatchJobStatus::Queued,
        episodes,
        summary: BatchAnalysisSummary {
            total,
            ..Default::default()
        },
        created_at: now_iso_timestamp(),
    }
}

fn uuid_simple() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = duration.as_nanos();
    let rand_val: u32 = rand_simple(nanos as u64);
    format!("{:x}{:04x}", nanos & 0xffffffffffff, rand_val & 0xffff)
}

fn rand_simple(seed: u64) -> u32 {
    let mut x = seed.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    (x ^ (x >> 31)) as u32
}

fn now_iso_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let dt: chrono_light::DateTime = now.into();
    dt.format_iso()
}

mod chrono_light {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub struct DateTime {
        pub year: u32,
        pub month: u32,
        pub day: u32,
        pub hour: u32,
        pub minute: u32,
        pub second: u32,
    }

    impl From<SystemTime> for DateTime {
        fn from(st: SystemTime) -> Self {
            let d = st.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
            let mut secs = d.as_secs();

            let second = (secs % 60) as u32;
            secs /= 60;
            let minute = (secs % 60) as u32;
            secs /= 60;
            let hour = (secs % 24) as u32;
            let days = (secs / 24) as i64;

            let mut year = 1970;
            let mut days_rem = days;
            loop {
                let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
                let days_in_year = if leap { 366 } else { 365 };
                if days_rem >= days_in_year {
                    days_rem -= days_in_year;
                    year += 1;
                } else {
                    break;
                }
            }

            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            let month_days = [
                31,
                if leap { 29 } else { 28 },
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ];
            let mut month = 1;
            for &md in &month_days {
                if days_rem >= md {
                    days_rem -= md;
                    month += 1;
                } else {
                    break;
                }
            }
            let day = (days_rem + 1) as u32;

            DateTime {
                year: year as u32,
                month,
                day,
                hour,
                minute,
                second,
            }
        }
    }

    impl DateTime {
        pub fn format_iso(&self) -> String {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                self.year, self.month, self.day, self.hour, self.minute, self.second
            )
        }
    }
}

pub fn analyse_audio_cancellable(
    path: &Path,
    total_duration_seconds: f64,
    is_cancelled: &Arc<AtomicBool>,
    active_pids: &Arc<Mutex<HashSet<u32>>>,
) -> Result<AudioMeasurements, AppError> {
    if is_cancelled.load(Ordering::SeqCst) {
        return Err(AppError::Cancelled);
    }

    let path_str = path.to_string_lossy().to_string();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let is_lossy = ext == "mp3" || ext == "m4a" || ext == "aac" || ext == "mp4" || ext == "mov";

    let mut cmd = ffmpeg_cmd()?;
    cmd.args([
        "-nostats",
        "-i",
        &path_str,
        "-af",
        "ebur128=peak=true:framelog=quiet,silencedetect=noise=-50dB:d=0.1,astats",
        "-f",
        "null",
        "-",
    ]);

    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        log::error!("Failed to spawn ffmpeg for analysis: {}", e);
        AppError::AudioAnalysisFailed("Failed to spawn ffmpeg analysis process".into())
    })?;

    let pid = child.id();
    {
        let mut pids = active_pids.lock().unwrap();
        pids.insert(pid);
    }

    let result = loop {
        if is_cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            {
                let mut pids = active_pids.lock().unwrap();
                pids.remove(&pid);
            }
            return Err(AppError::Cancelled);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stderr_bytes = Vec::new();
                if let Some(mut stderr) = child.stderr.take() {
                    use std::io::Read;
                    let _ = stderr.read_to_end(&mut stderr_bytes);
                }
                break Ok((status, stderr_bytes));
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                break Err(AppError::AudioAnalysisFailed(format!(
                    "Failed to monitor analysis process: {}",
                    e
                )));
            }
        }
    };

    {
        let mut pids = active_pids.lock().unwrap();
        pids.remove(&pid);
    }

    let (status, stderr_bytes) = result?;
    let stderr_str = String::from_utf8_lossy(&stderr_bytes);

    if !status.success() {
        if is_cancelled.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        log::error!("ffmpeg analysis process exited with error: {}", stderr_str);
        return Err(AppError::AudioAnalysisFailed(
            "Audio analysis could not complete for this file".into(),
        ));
    }

    let parsed = parse_ffmpeg_analysis(&stderr_str, total_duration_seconds, is_lossy);
    let clipping = evaluate_clipping_evidence(
        parsed.sample_peak_dbfs,
        parsed.samples_at_ceiling,
        parsed.flat_factor,
        parsed.is_lossy,
    );

    Ok(AudioMeasurements {
        integrated_loudness_lufs: parsed.integrated_loudness_lufs,
        true_peak_dbtp: parsed.true_peak_dbtp,
        leading_silence_seconds: (parsed.leading_silence_seconds * 10.0).round() / 10.0,
        trailing_silence_seconds: (parsed.trailing_silence_seconds * 10.0).round() / 10.0,
        clipping,
    })
}

pub fn update_summary(job: &mut BatchAnalysisJob, elapsed_seconds: f64) {
    let mut complete = 0;
    let mut failed = 0;
    let mut cancelled = 0;
    let mut ready = 0;
    let mut attention = 0;
    let mut needs_attention = 0;

    for ep in &job.episodes {
        match ep.status {
            BatchEpisodeStatus::Complete => {
                complete += 1;
                if let Some(assessment) = &ep.assessment {
                    match assessment.overall_status {
                        OverallStatus::Ready => ready += 1,
                        OverallStatus::Attention => attention += 1,
                        OverallStatus::NeedsAttention => needs_attention += 1,
                    }
                }
            }
            BatchEpisodeStatus::Failed => {
                failed += 1;
            }
            BatchEpisodeStatus::Cancelled => {
                cancelled += 1;
            }
            _ => {}
        }
    }

    job.summary = BatchAnalysisSummary {
        total: job.episodes.len(),
        complete,
        failed,
        cancelled,
        ready,
        attention,
        needs_attention,
        elapsed_seconds: (elapsed_seconds * 10.0).round() / 10.0,
    };
}

impl BatchAnalysisJob {
    pub fn update_episode<F>(
        &mut self,
        index: usize,
        update_fn: F,
        elapsed_seconds: f64,
    ) -> (BatchEpisode, BatchAnalysisSummary)
    where
        F: FnOnce(&mut BatchEpisode),
    {
        if let Some(ep) = self.episodes.get_mut(index) {
            update_fn(ep);
        }
        update_summary(self, elapsed_seconds);
        (self.episodes[index].clone(), self.summary.clone())
    }
}

pub async fn run_batch_job<F>(
    job_handle: Arc<BatchJobHandle>,
    concurrency: usize,
    on_progress: F,
) where
    F: Fn(BatchProgressPayload) + Send + Sync + 'static,
{
    let on_progress = Arc::new(on_progress);
    let start_time = Instant::now();

    {
        let mut job_write = job_handle.job.write().unwrap();
        job_write.status = BatchJobStatus::Running;
    }

    let episode_count = {
        let job_read = job_handle.job.read().unwrap();
        job_read.episodes.len()
    };

    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut handles = Vec::new();

    for index in 0..episode_count {
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };

        let handle_clone = job_handle.clone();
        let on_progress_clone = on_progress.clone();

        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let is_cancelled = handle_clone.is_cancelled.clone();
            let active_pids = handle_clone.active_pids.clone();

            if is_cancelled.load(Ordering::SeqCst) {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Cancelled;
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Cancelled,
                    episode: ep_clone,
                    summary: summary_clone,
                });
                return;
            }

            let source_path = {
                let job_read = handle_clone.job.read().unwrap();
                job_read.episodes[index].source_path.clone()
            };

            let ep_start = Instant::now();

            // Step 1: Inspecting
            {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Inspecting;
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Inspecting,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }

            if is_cancelled.load(Ordering::SeqCst) {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Cancelled;
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Cancelled,
                    episode: ep_clone,
                    summary: summary_clone,
                });
                return;
            }

            // Execute inspect_media
            let inspected = match inspect_media(&source_path) {
                Ok(src) => src,
                Err(err) => {
                    let (ep_clone, summary_clone) = {
                        let mut job_write = handle_clone.job.write().unwrap();
                        let ep_elapsed =
                            (ep_start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
                        job_write.update_episode(
                            index,
                            |ep| {
                                ep.status = BatchEpisodeStatus::Failed;
                                ep.error = Some(match err {
                                    AppError::UnsupportedFormat => {
                                        "Unsupported audio format or corrupt header".into()
                                    }
                                    _ => "Could not inspect audio file".into(),
                                });
                                ep.elapsed_seconds = Some(ep_elapsed);
                            },
                            start_time.elapsed().as_secs_f64(),
                        )
                    };
                    on_progress_clone(BatchProgressPayload {
                        job_id: handle_clone.id.clone(),
                        episode_id: ep_clone.id.clone(),
                        status: BatchEpisodeStatus::Failed,
                        episode: ep_clone,
                        summary: summary_clone,
                    });
                    return;
                }
            };

            // Step 2: Analysing
            {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Analysing;
                            ep.format = Some(inspected.format.clone());
                            ep.codec = Some(inspected.codec.clone());
                            ep.inspection = Some(inspected.inspection.clone());
                            ep.duration_seconds = Some(inspected.inspection.duration_seconds);
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Analysing,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }

            if is_cancelled.load(Ordering::SeqCst) {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Cancelled;
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Cancelled,
                    episode: ep_clone,
                    summary: summary_clone,
                });
                return;
            }

            // Execute analyse_audio
            let measurements = match analyse_audio_cancellable(
                Path::new(&source_path),
                inspected.inspection.duration_seconds,
                &is_cancelled,
                &active_pids,
            ) {
                Ok(m) => m,
                Err(AppError::Cancelled) => {
                    let (ep_clone, summary_clone) = {
                        let mut job_write = handle_clone.job.write().unwrap();
                        job_write.update_episode(
                            index,
                            |ep| {
                                ep.status = BatchEpisodeStatus::Cancelled;
                            },
                            start_time.elapsed().as_secs_f64(),
                        )
                    };
                    on_progress_clone(BatchProgressPayload {
                        job_id: handle_clone.id.clone(),
                        episode_id: ep_clone.id.clone(),
                        status: BatchEpisodeStatus::Cancelled,
                        episode: ep_clone,
                        summary: summary_clone,
                    });
                    return;
                }
                Err(_err) => {
                    let (ep_clone, summary_clone) = {
                        let mut job_write = handle_clone.job.write().unwrap();
                        let ep_elapsed =
                            (ep_start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
                        job_write.update_episode(
                            index,
                            |ep| {
                                ep.status = BatchEpisodeStatus::Failed;
                                ep.error = Some("Audio measurement analysis failed".into());
                                ep.elapsed_seconds = Some(ep_elapsed);
                            },
                            start_time.elapsed().as_secs_f64(),
                        )
                    };
                    on_progress_clone(BatchProgressPayload {
                        job_id: handle_clone.id.clone(),
                        episode_id: ep_clone.id.clone(),
                        status: BatchEpisodeStatus::Failed,
                        episode: ep_clone,
                        summary: summary_clone,
                    });
                    return;
                }
            };

            // Step 3: Assessing
            {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Assessing;
                            ep.measurements = Some(measurements.clone());
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Assessing,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }

            if is_cancelled.load(Ordering::SeqCst) {
                let (ep_clone, summary_clone) = {
                    let mut job_write = handle_clone.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = BatchEpisodeStatus::Cancelled;
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };
                on_progress_clone(BatchProgressPayload {
                    job_id: handle_clone.id.clone(),
                    episode_id: ep_clone.id.clone(),
                    status: BatchEpisodeStatus::Cancelled,
                    episode: ep_clone,
                    summary: summary_clone,
                });
                return;
            }

            // Execute assess_media
            let assessment = assess_media(
                &inspected.inspection,
                Some(&measurements),
                &inspected.format,
                &inspected.codec,
            );

            // Step 4: Complete
            let ep_elapsed = (ep_start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
            let (ep_clone, summary_clone) = {
                let mut job_write = handle_clone.job.write().unwrap();
                job_write.update_episode(
                    index,
                    |ep| {
                        ep.status = BatchEpisodeStatus::Complete;
                        ep.assessment = Some(assessment);
                        ep.elapsed_seconds = Some(ep_elapsed);
                    },
                    start_time.elapsed().as_secs_f64(),
                )
            };
            on_progress_clone(BatchProgressPayload {
                job_id: handle_clone.id.clone(),
                episode_id: ep_clone.id.clone(),
                status: BatchEpisodeStatus::Complete,
                episode: ep_clone,
                summary: summary_clone,
            });
        });

        handles.push(task);
    }

    for h in handles {
        let _ = h.await;
    }

    let final_elapsed = start_time.elapsed().as_secs_f64();
    let is_cancelled = job_handle.is_cancelled.load(Ordering::SeqCst);

    {
        let mut job_write = job_handle.job.write().unwrap();
        if is_cancelled {
            for ep in &mut job_write.episodes {
                if ep.status == BatchEpisodeStatus::Waiting
                    || ep.status == BatchEpisodeStatus::Inspecting
                    || ep.status == BatchEpisodeStatus::Analysing
                    || ep.status == BatchEpisodeStatus::Assessing
                {
                    ep.status = BatchEpisodeStatus::Cancelled;
                }
            }
            job_write.status = BatchJobStatus::Cancelled;
        } else {
            job_write.status = BatchJobStatus::Complete;
        }
        update_summary(&mut job_write, final_elapsed);
    }
}
