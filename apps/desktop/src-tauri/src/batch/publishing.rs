use crate::assessment::engine::assess_media;
use crate::catalogue::models::SourceAvailability;
use crate::catalogue::service::CatalogueService;
use crate::error::AppError;
use crate::export::publish::{publish_single_episode, PublishingEpisodeStage};
use crate::export::types::{ExportOptions, PodReadyPackage};
use crate::media::analysis::AudioMeasurements;
use crate::media::ffprobe::inspect_media;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublishingEpisodeStatus {
    Waiting,
    Preparing,
    Processing,
    Verifying,
    Exporting,
    Transcribing,
    Packaging,
    Complete,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingEpisode {
    pub episode_id: String,
    pub source_path: String,
    pub filename: String,
    pub status: PublishingEpisodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<PublishingEpisodeStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_availability: Option<SourceAvailability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<PodReadyPackage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reanalysed: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingSummary {
    pub total: usize,
    pub complete: usize,
    pub partial: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub skipped: usize,
    pub elapsed_seconds: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchPublishingJobStatus {
    Queued,
    Running,
    Complete,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingJob {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_name: Option<String>,
    pub status: BatchPublishingJobStatus,
    pub destination_directory: String,
    pub episodes: Vec<BatchPublishingEpisode>,
    pub summary: BatchPublishingSummary,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingProgressPayload {
    pub job_id: String,
    pub episode_id: String,
    pub status: PublishingEpisodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<PublishingEpisodeStage>,
    pub episode: BatchPublishingEpisode,
    pub summary: BatchPublishingSummary,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingManifestItem {
    pub episode_id: String,
    pub filename: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchPublishingManifest {
    pub podready_version: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_name: Option<String>,
    pub destination_directory: String,
    pub summary: BatchPublishingSummary,
    pub episodes: Vec<BatchPublishingManifestItem>,
}

pub struct BatchPublishingJobHandle {
    pub id: String,
    pub is_cancelled: Arc<AtomicBool>,
    pub active_pids: Arc<Mutex<HashSet<u32>>>,
    pub job: Arc<RwLock<BatchPublishingJob>>,
}

impl BatchPublishingJobHandle {
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

pub struct BatchPublishingManager {
    jobs: Arc<RwLock<HashMap<String, Arc<BatchPublishingJobHandle>>>>,
}

impl BatchPublishingManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_job<F>(
        &self,
        show_id: Option<String>,
        show_name: Option<String>,
        episodes_input: Vec<BatchPublishingEpisodeInput>,
        destination_directory: String,
        options: ExportOptions,
        catalogue_service: Option<CatalogueService>,
        on_progress: F,
    ) -> Result<BatchPublishingJob, AppError>
    where
        F: Fn(BatchPublishingProgressPayload) + Send + Sync + 'static,
    {
        let job_id = format!("pub-job-{}", uuid_simple());
        let mut episodes = Vec::new();

        for ep in episodes_input {
            episodes.push(BatchPublishingEpisode {
                episode_id: ep.id,
                source_path: ep.source_path,
                filename: ep.filename,
                status: PublishingEpisodeStatus::Waiting,
                stage: None,
                elapsed_seconds: None,
                source_availability: Some(ep.source_availability),
                skip_reason: None,
                package: None,
                error: None,
                reanalysed: None,
            });
        }

        let total = episodes.len();
        let created_at = now_iso_timestamp();
        let job = BatchPublishingJob {
            id: job_id.clone(),
            show_id,
            show_name,
            status: BatchPublishingJobStatus::Queued,
            destination_directory,
            episodes,
            summary: BatchPublishingSummary {
                total,
                ..Default::default()
            },
            created_at,
            started_at: None,
            elapsed_seconds: None,
        };

        let job_arc = Arc::new(RwLock::new(job.clone()));
        let handle = Arc::new(BatchPublishingJobHandle {
            id: job_id.clone(),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            active_pids: Arc::new(Mutex::new(HashSet::new())),
            job: job_arc,
        });

        {
            let mut guard = self.jobs.write().unwrap();
            guard.insert(job_id, handle.clone());
        }

        let handle_clone = handle.clone();
        tauri::async_runtime::spawn(async move {
            run_batch_publishing_job(handle_clone, options, catalogue_service, on_progress).await;
        });

        Ok(job)
    }

    pub fn cancel_job(&self, job_id: &str) -> Result<(), AppError> {
        let guard = self.jobs.read().unwrap();
        if let Some(handle) = guard.get(job_id) {
            handle.cancel();
            Ok(())
        } else {
            Err(AppError::SystemError("Batch publishing job not found".into()))
        }
    }

    pub fn get_job(&self, job_id: &str) -> Result<BatchPublishingJob, AppError> {
        let guard = self.jobs.read().unwrap();
        if let Some(handle) = guard.get(job_id) {
            let job_read = handle.job.read().unwrap();
            Ok(job_read.clone())
        } else {
            Err(AppError::SystemError("Batch publishing job not found".into()))
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchPublishingEpisodeInput {
    pub id: String,
    pub source_path: String,
    pub filename: String,
    pub source_availability: SourceAvailability,
}

fn update_summary(job: &mut BatchPublishingJob, elapsed_seconds: f64) {
    let mut complete = 0;
    let mut partial = 0;
    let mut failed = 0;
    let mut cancelled = 0;
    let mut skipped = 0;

    for ep in &job.episodes {
        match ep.status {
            PublishingEpisodeStatus::Complete => {
                if let Some(ref pkg) = ep.package {
                    if pkg.transcript_error.is_some() {
                        partial += 1;
                    } else {
                        complete += 1;
                    }
                } else {
                    complete += 1;
                }
            }
            PublishingEpisodeStatus::Failed => failed += 1,
            PublishingEpisodeStatus::Cancelled => cancelled += 1,
            PublishingEpisodeStatus::Skipped => skipped += 1,
            _ => {}
        }
    }

    job.summary = BatchPublishingSummary {
        total: job.episodes.len(),
        complete,
        partial,
        failed,
        cancelled,
        skipped,
        elapsed_seconds: (elapsed_seconds * 10.0).round() / 10.0,
    };
    job.elapsed_seconds = Some(job.summary.elapsed_seconds);
}

impl BatchPublishingJob {
    pub fn update_episode<F>(
        &mut self,
        index: usize,
        update_fn: F,
        elapsed_seconds: f64,
    ) -> (BatchPublishingEpisode, BatchPublishingSummary)
    where
        F: FnOnce(&mut BatchPublishingEpisode),
    {
        if let Some(ep) = self.episodes.get_mut(index) {
            update_fn(ep);
        }
        update_summary(self, elapsed_seconds);
        (self.episodes[index].clone(), self.summary.clone())
    }
}

pub async fn run_batch_publishing_job<F>(
    job_handle: Arc<BatchPublishingJobHandle>,
    options: ExportOptions,
    catalogue_service: Option<CatalogueService>,
    on_progress: F,
) where
    F: Fn(BatchPublishingProgressPayload) + Send + Sync + 'static,
{
    let on_progress = Arc::new(on_progress);
    let start_time = Instant::now();

    {
        let mut job_write = job_handle.job.write().unwrap();
        job_write.status = BatchPublishingJobStatus::Running;
        job_write.started_at = Some(now_iso_timestamp());
    }

    let (episode_count, destination_directory) = {
        let job_read = job_handle.job.read().unwrap();
        (job_read.episodes.len(), job_read.destination_directory.clone())
    };

    // Ensure destination directory exists
    let _ = std::fs::create_dir_all(&destination_directory);

    // Concurrency = 1 (Serial execution policy)
    for index in 0..episode_count {
        let is_cancelled = job_handle.is_cancelled.clone();
        let active_pids = job_handle.active_pids.clone();

        if is_cancelled.load(Ordering::SeqCst) {
            let (ep_clone, summary_clone) = {
                let mut job_write = job_handle.job.write().unwrap();
                job_write.update_episode(
                    index,
                    |ep| {
                        ep.status = PublishingEpisodeStatus::Cancelled;
                    },
                    start_time.elapsed().as_secs_f64(),
                )
            };
            on_progress(BatchPublishingProgressPayload {
                job_id: job_handle.id.clone(),
                episode_id: ep_clone.episode_id.clone(),
                status: PublishingEpisodeStatus::Cancelled,
                stage: None,
                episode: ep_clone,
                summary: summary_clone,
            });
            continue;
        }

        let (episode_id, source_path, source_availability) = {
            let job_read = job_handle.job.read().unwrap();
            let ep = &job_read.episodes[index];
            (
                ep.episode_id.clone(),
                ep.source_path.clone(),
                ep.source_availability.clone().unwrap_or(SourceAvailability::Available),
            )
        };

        let ep_start = Instant::now();

        // 1. Source Availability Evaluation
        let source_path_obj = Path::new(&source_path);
        if source_availability == SourceAvailability::Missing || !source_path_obj.exists() {
            let ep_elapsed = (ep_start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
            let (ep_clone, summary_clone) = {
                let mut job_write = job_handle.job.write().unwrap();
                job_write.update_episode(
                    index,
                    |ep| {
                        ep.status = PublishingEpisodeStatus::Skipped;
                        ep.skip_reason = Some("Source file unavailable".to_string());
                        ep.elapsed_seconds = Some(ep_elapsed);
                    },
                    start_time.elapsed().as_secs_f64(),
                )
            };
            on_progress(BatchPublishingProgressPayload {
                job_id: job_handle.id.clone(),
                episode_id,
                status: PublishingEpisodeStatus::Skipped,
                stage: None,
                episode: ep_clone,
                summary: summary_clone,
            });
            continue;
        }

        let is_changed = source_availability == SourceAvailability::Changed;
        if is_changed {
            let (ep_clone, summary_clone) = {
                let mut job_write = job_handle.job.write().unwrap();
                job_write.update_episode(
                    index,
                    |ep| {
                        ep.status = PublishingEpisodeStatus::Preparing;
                        ep.stage = Some(PublishingEpisodeStage::Preparing);
                        ep.reanalysed = Some(true);
                    },
                    start_time.elapsed().as_secs_f64(),
                )
            };
            on_progress(BatchPublishingProgressPayload {
                job_id: job_handle.id.clone(),
                episode_id: episode_id.clone(),
                status: PublishingEpisodeStatus::Preparing,
                stage: Some(PublishingEpisodeStage::Preparing),
                episode: ep_clone,
                summary: summary_clone,
            });
        }

        // 2. Execute unified single-episode publishing workflow
        let job_handle_stage = job_handle.clone();
        let on_progress_stage = on_progress.clone();
        let ep_id_stage = episode_id.clone();
        let start_time_stage = start_time;

        let stage_callback = move |stage: PublishingEpisodeStage| {
            let status = match stage {
                PublishingEpisodeStage::Preparing => PublishingEpisodeStatus::Preparing,
                PublishingEpisodeStage::Processing => PublishingEpisodeStatus::Processing,
                PublishingEpisodeStage::Verifying => PublishingEpisodeStatus::Verifying,
                PublishingEpisodeStage::Exporting => PublishingEpisodeStatus::Exporting,
                PublishingEpisodeStage::Transcribing => PublishingEpisodeStatus::Transcribing,
                PublishingEpisodeStage::Packaging => PublishingEpisodeStatus::Packaging,
            };

            let (ep_clone, summary_clone) = {
                let mut job_write = job_handle_stage.job.write().unwrap();
                job_write.update_episode(
                    index,
                    |ep| {
                        ep.status = status;
                        ep.stage = Some(stage);
                    },
                    start_time_stage.elapsed().as_secs_f64(),
                )
            };

            on_progress_stage(BatchPublishingProgressPayload {
                job_id: job_handle_stage.id.clone(),
                episode_id: ep_id_stage.clone(),
                status,
                stage: Some(stage),
                episode: ep_clone,
                summary: summary_clone,
            });
        };

        // Determine if cached measurements are safe to reuse
        let (cached_meas, cached_assess) = if is_changed {
            (None, None)
        } else {
            // Re-fetch episode from catalogue repository if available
            if let Some(ref cat) = catalogue_service {
                if let Ok(cat_ep) = cat.get_episode(&episode_id) {
                    let evidence = match cat_ep.clipping_evidence.to_uppercase().as_str() {
                        "POSSIBLE" => crate::media::analysis::ClippingEvidence::POSSIBLE,
                        "UNCERTAIN" => crate::media::analysis::ClippingEvidence::UNCERTAIN,
                        _ => crate::media::analysis::ClippingEvidence::NONE,
                    };
                    let meas = AudioMeasurements {
                        integrated_loudness_lufs: cat_ep.integrated_loudness_lufs,
                        true_peak_dbtp: cat_ep.true_peak_dbtp,
                        leading_silence_seconds: cat_ep.leading_silence_seconds,
                        trailing_silence_seconds: cat_ep.trailing_silence_seconds,
                        clipping: crate::media::analysis::ClippingAnalysis {
                            sample_peak_dbfs: None,
                            samples_at_ceiling: 0,
                            flat_factor: 0.0,
                            evidence,
                        },
                    };
                    let assess = cat_ep.assessment.or_else(|| {
                        let insp = crate::media::ffprobe::MediaInspection {
                            duration_seconds: cat_ep.duration_seconds,
                            sample_rate: cat_ep.sample_rate,
                            channels: cat_ep.channels as u32,
                            bitrate: cat_ep.bitrate,
                            file_size_bytes: cat_ep.file_size_bytes.max(0) as u64,
                        };
                        Some(assess_media(&insp, Some(&meas), &cat_ep.format, &cat_ep.codec))
                    });
                    (Some(meas), assess)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };


        let pub_result = publish_single_episode(
            &source_path,
            &destination_directory,
            &options,
            cached_meas,
            cached_assess,
            Some(&is_cancelled),
            Some(&active_pids),
            Some(&stage_callback),
        );

        let ep_elapsed = (ep_start.elapsed().as_secs_f64() * 10.0).round() / 10.0;

        match pub_result {
            Ok(pkg) => {
                // If CHANGED, update catalogue record with fresh measurements
                if is_changed {
                    if let Some(ref cat) = catalogue_service {
                        if let Ok(cat_ep) = cat.get_episode(&episode_id) {
                            if let Ok(insp) = inspect_media(&source_path) {
                                let media_source = crate::media::ffprobe::MediaSource {
                                    path: source_path.clone(),
                                    filename: insp.filename.clone(),
                                    format: insp.format.clone(),
                                    codec: insp.codec.clone(),
                                    inspection: insp.inspection.clone(),
                                    measurements: Some(pkg.verification_result.measurements.clone()),
                                    assessment: Some(pkg.verification_result.assessment.clone()),
                                };
                                let _ = cat.add_media_source_to_show(&cat_ep.show_id, &media_source);
                            }
                        }
                    }
                }

                let (ep_clone, summary_clone) = {
                    let mut job_write = job_handle.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = PublishingEpisodeStatus::Complete;
                            ep.stage = None;
                            ep.elapsed_seconds = Some(ep_elapsed);
                            ep.package = Some(pkg);
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };

                on_progress(BatchPublishingProgressPayload {
                    job_id: job_handle.id.clone(),
                    episode_id,
                    status: PublishingEpisodeStatus::Complete,
                    stage: None,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }
            Err(AppError::Cancelled) => {
                let (ep_clone, summary_clone) = {
                    let mut job_write = job_handle.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = PublishingEpisodeStatus::Cancelled;
                            ep.stage = None;
                            ep.elapsed_seconds = Some(ep_elapsed);
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };

                on_progress(BatchPublishingProgressPayload {
                    job_id: job_handle.id.clone(),
                    episode_id,
                    status: PublishingEpisodeStatus::Cancelled,
                    stage: None,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }
            Err(err) => {
                let (ep_clone, summary_clone) = {
                    let mut job_write = job_handle.job.write().unwrap();
                    job_write.update_episode(
                        index,
                        |ep| {
                            ep.status = PublishingEpisodeStatus::Failed;
                            ep.stage = None;
                            ep.elapsed_seconds = Some(ep_elapsed);
                            ep.error = Some(err.to_string());
                        },
                        start_time.elapsed().as_secs_f64(),
                    )
                };

                on_progress(BatchPublishingProgressPayload {
                    job_id: job_handle.id.clone(),
                    episode_id,
                    status: PublishingEpisodeStatus::Failed,
                    stage: None,
                    episode: ep_clone,
                    summary: summary_clone,
                });
            }
        }
    }

    let final_elapsed = start_time.elapsed().as_secs_f64();
    let is_cancelled = job_handle.is_cancelled.load(Ordering::SeqCst);

    {
        let mut job_write = job_handle.job.write().unwrap();
        if is_cancelled {
            for ep in &mut job_write.episodes {
                if ep.status == PublishingEpisodeStatus::Waiting
                    || ep.status == PublishingEpisodeStatus::Preparing
                    || ep.status == PublishingEpisodeStatus::Processing
                    || ep.status == PublishingEpisodeStatus::Verifying
                    || ep.status == PublishingEpisodeStatus::Exporting
                    || ep.status == PublishingEpisodeStatus::Transcribing
                    || ep.status == PublishingEpisodeStatus::Packaging
                {
                    ep.status = PublishingEpisodeStatus::Cancelled;
                }
            }
            job_write.status = BatchPublishingJobStatus::Cancelled;
        } else {
            job_write.status = BatchPublishingJobStatus::Complete;
        }
        update_summary(&mut job_write, final_elapsed);

        // Write batch manifest file
        write_batch_manifest(&job_write);
    }
}

fn write_batch_manifest(job: &BatchPublishingJob) {
    let manifest_path = Path::new(&job.destination_directory).join("podready_batch_manifest.json");
    let manifest_items: Vec<BatchPublishingManifestItem> = job
        .episodes
        .iter()
        .map(|ep| BatchPublishingManifestItem {
            episode_id: ep.episode_id.clone(),
            filename: ep.filename.clone(),
            status: match ep.status {
                PublishingEpisodeStatus::Complete => "COMPLETE".to_string(),
                PublishingEpisodeStatus::Failed => "FAILED".to_string(),
                PublishingEpisodeStatus::Cancelled => "CANCELLED".to_string(),
                PublishingEpisodeStatus::Skipped => "SKIPPED".to_string(),
                _ => "UNKNOWN".to_string(),
            },
            package_directory: ep.package.as_ref().map(|p| p.package_directory.clone()),
            elapsed_seconds: ep.elapsed_seconds,
            error: ep.error.clone(),
            skip_reason: ep.skip_reason.clone(),
        })
        .collect();

    let manifest = BatchPublishingManifest {
        podready_version: "1.0.0".to_string(),
        created_at: now_iso_timestamp(),
        show_id: job.show_id.clone(),
        show_name: job.show_name.clone(),
        destination_directory: job.destination_directory.clone(),
        summary: job.summary.clone(),
        episodes: manifest_items,
    };

    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = std::fs::write(manifest_path, json);
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
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    format!("{}-01-01T00:00:00Z (timestamp: {})", 1970 + secs / 31536000, secs)
}
