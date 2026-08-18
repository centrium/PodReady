use std::path::Path;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use uuid::Uuid;

use crate::assessment::engine::OverallStatus;
use crate::assessment::Assessment;
use crate::batch::{BatchEpisode, BatchEpisodeStatus};
use crate::catalogue::baseline::{compute_show_baseline, ShowBaseline};
use crate::catalogue::models::{
    AddBatchEpisodesResult, AddEpisodeOutcome, AddEpisodeStatus, CatalogueEpisode, Show,
    ShowSummary, ShowWithEpisodes, SourceAvailability,
};
use crate::catalogue::repository::CatalogueRepository;
use crate::catalogue::show_check::{run_show_check, CandidateMeasurements, ShowCheck};
use crate::error::AppError;
use crate::media::ffprobe::{MediaFormat, MediaSource};

#[derive(Clone)]
pub struct CatalogueService {
    repo: Arc<Mutex<CatalogueRepository>>,
}

impl CatalogueService {
    pub fn new(repo: CatalogueRepository) -> Self {
        Self {
            repo: Arc::new(Mutex::new(repo)),
        }
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self, AppError> {
        let repo = CatalogueRepository::open_in_memory()?;
        Ok(Self::new(repo))
    }

    pub fn create_show(&self, name: &str, description: Option<&str>) -> Result<Show, AppError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::SystemError("Show name cannot be empty".to_string()));
        }

        let id = format!("show_{}", Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();

        let mut repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.create_show(&id, trimmed_name, description.map(|s| s.trim()), &now)
    }

    pub fn update_show(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Show, AppError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::SystemError("Show name cannot be empty".to_string()));
        }

        let now = Utc::now().to_rfc3339();

        let mut repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.update_show(id, trimmed_name, description.map(|s| s.trim()), &now)
    }

    pub fn get_shows(&self) -> Result<Vec<ShowSummary>, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.get_shows()
    }

    pub fn get_show(&self, id: &str) -> Result<ShowWithEpisodes, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        let show = repo
            .get_show_by_id(id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", id)))?;

        let episodes = repo.get_episodes_for_show(id)?;

        Ok(ShowWithEpisodes { show, episodes })
    }

    pub fn get_show_baseline(&self, id: &str) -> Result<ShowBaseline, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        let show = repo
            .get_show_by_id(id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", id)))?;

        let episodes = repo.get_episodes_for_show(id)?;

        Ok(compute_show_baseline(&show, &episodes))
    }

    /// Computes Show Check for a catalogued episode using Leave-One-Out (LOO) baseline.
    /// Excludes the target episode from the comparison baseline so the candidate does not define its own baseline.
    pub fn get_show_check_for_episode(&self, episode_id: &str) -> Result<ShowCheck, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        let target_ep = repo
            .get_episode_by_id(episode_id)?
            .ok_or_else(|| AppError::NotFound(format!("Episode {} not found", episode_id)))?;

        let show = repo
            .get_show_by_id(&target_ep.show_id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", target_ep.show_id)))?;

        let all_episodes = repo.get_episodes_for_show(&target_ep.show_id)?;

        // Leave-One-Out: filter out this episode from historical baseline dataset
        let other_episodes: Vec<CatalogueEpisode> = all_episodes
            .into_iter()
            .filter(|ep| ep.id != target_ep.id)
            .collect();

        let loo_baseline = compute_show_baseline(&show, &other_episodes);
        let candidate_measurements = CandidateMeasurements::from_catalogue_episode(&target_ep);
        let is_stale = target_ep.source_availability == SourceAvailability::Changed;

        Ok(run_show_check(&loo_baseline, &candidate_measurements, is_stale))
    }

    /// Computes Show Check for a newly analysed media candidate (e.g. in Workspace) against a Show's full baseline.
    pub fn run_show_check_for_media(&self, show_id: &str, media: &MediaSource) -> Result<ShowCheck, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        let show = repo
            .get_show_by_id(show_id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", show_id)))?;

        let episodes = repo.get_episodes_for_show(show_id)?;
        let baseline = compute_show_baseline(&show, &episodes);
        let candidate_measurements = CandidateMeasurements::from_media_source(media);

        Ok(run_show_check(&baseline, &candidate_measurements, false))
    }

    pub fn delete_show(&self, id: &str) -> Result<(), AppError> {
        let mut repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.delete_show(id)
    }

    pub fn get_episode(&self, id: &str) -> Result<CatalogueEpisode, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.get_episode_by_id(id)?
            .ok_or_else(|| AppError::NotFound(format!("Episode {} not found", id)))
    }

    pub fn delete_episode(&self, id: &str) -> Result<(), AppError> {
        let mut repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        repo.delete_episode(id)
    }

    pub fn add_media_source_to_show(
        &self,
        show_id: &str,
        media: &MediaSource,
    ) -> Result<AddEpisodeOutcome, AppError> {
        let assessment = media.assessment.as_ref().ok_or_else(|| {
            AppError::SystemError("Cannot catalogue episode without assessment".to_string())
        })?;

        let (file_size_bytes, source_modified_at) =
            extract_source_metadata(&media.path, media.inspection.file_size_bytes as i64);

        self.add_or_update_episode_internal(
            show_id,
            &media.path,
            &media.filename,
            file_size_bytes,
            media.inspection.duration_seconds,
            media.format.clone(),
            &media.codec,
            media.inspection.sample_rate,
            media.inspection.channels as u16,
            media.inspection.bitrate,
            media.measurements.as_ref().and_then(|m| m.integrated_loudness_lufs),
            media.measurements.as_ref().and_then(|m| m.true_peak_dbtp),
            media.measurements.as_ref().map(|m| m.leading_silence_seconds).unwrap_or(0.0),
            media.measurements.as_ref().map(|m| m.trailing_silence_seconds).unwrap_or(0.0),
            media.measurements.as_ref().map(|m| format!("{:?}", m.clipping.evidence)).unwrap_or_else(|| "NONE".to_string()),
            assessment,
            source_modified_at,
        )
    }

    pub fn add_batch_episodes_to_show(
        &self,
        show_id: &str,
        episodes: &[BatchEpisode],
    ) -> Result<AddBatchEpisodesResult, AppError> {
        let repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;


        let show = repo
            .get_show_by_id(show_id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", show_id)))?;

        // Drop repo lock before calling internal methods
        drop(repo);

        let mut added = 0;
        let mut updated = 0;
        let mut already_exists = 0;
        let mut skipped_failed = 0;
        let mut outcomes = Vec::new();

        for ep in episodes {
            // Guard: FAILED or CANCELLED batch items must not silently enter the catalogue
            if ep.status != BatchEpisodeStatus::Complete || ep.assessment.is_none() {
                skipped_failed += 1;
                continue;
            }

            let assessment = ep.assessment.as_ref().unwrap();
            let inspection = ep.inspection.as_ref();
            let duration = ep.duration_seconds.or_else(|| inspection.map(|i| i.duration_seconds)).unwrap_or(0.0);
            let format = ep.format.clone().unwrap_or(MediaFormat::UNKNOWN);
            let codec = ep.codec.clone().unwrap_or_default();
            let sample_rate = inspection.map(|i| i.sample_rate).unwrap_or(0);
            let channels = inspection.map(|i| i.channels).unwrap_or(0);
            let bitrate = inspection.and_then(|i| i.bitrate);

            let default_size = inspection.map(|i| i.file_size_bytes).unwrap_or(0);
            let (file_size_bytes, source_modified_at) = extract_source_metadata(&ep.source_path, default_size as i64);

            let outcome = self.add_or_update_episode_internal(
                show_id,
                &ep.source_path,
                &ep.filename,
                file_size_bytes,
                duration,
                format,
                &codec,
                sample_rate,
                channels as u16,
                bitrate,
                ep.measurements.as_ref().and_then(|m| m.integrated_loudness_lufs),
                ep.measurements.as_ref().and_then(|m| m.true_peak_dbtp),
                ep.measurements.as_ref().map(|m| m.leading_silence_seconds).unwrap_or(0.0),
                ep.measurements.as_ref().map(|m| m.trailing_silence_seconds).unwrap_or(0.0),
                ep.measurements.as_ref().map(|m| format!("{:?}", m.clipping.evidence)).unwrap_or_else(|| "NONE".to_string()),
                assessment,
                source_modified_at,
            )?;


            match outcome.status {
                AddEpisodeStatus::Added => added += 1,
                AddEpisodeStatus::Updated => updated += 1,
                AddEpisodeStatus::AlreadyExists => already_exists += 1,
            }

            outcomes.push(outcome);
        }

        Ok(AddBatchEpisodesResult {
            show_id: show.id,
            show_name: show.name,
            total_processed: episodes.len(),
            added,
            updated,
            already_exists,
            skipped_failed,
            outcomes,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn add_or_update_episode_internal(
        &self,
        show_id: &str,
        source_path: &str,
        filename: &str,
        file_size_bytes: i64,
        duration_seconds: f64,
        format: MediaFormat,
        codec: &str,
        sample_rate: u32,
        channels: u16,
        bitrate: Option<u32>,
        integrated_loudness_lufs: Option<f64>,
        true_peak_dbtp: Option<f64>,
        leading_silence_seconds: f64,
        trailing_silence_seconds: f64,
        clipping_evidence: String,
        assessment: &Assessment,
        source_modified_at: Option<String>,
    ) -> Result<AddEpisodeOutcome, AppError> {
        let mut repo = self.repo.lock().map_err(|e| {
            AppError::SystemError(format!("Catalogue lock error: {}", e))
        })?;

        // Verify show exists
        if repo.get_show_by_id(show_id)?.is_none() {
            return Err(AppError::NotFound(format!("Show {} not found", show_id)));
        }

        let now = Utc::now().to_rfc3339();
        let assessment_json = serde_json::to_string(assessment).ok();
        let overall_assessment_status = match assessment.overall_status {
            OverallStatus::Ready => "READY",
            OverallStatus::Attention => "ATTENTION",
            OverallStatus::NeedsAttention => "NEEDS_ATTENTION",
        }.to_string();

        let existing = repo.get_episode_by_source_path(show_id, source_path)?;

        if let Some(existing_ep) = existing {
            // Duplicate / Identity Check:
            // Check if file size and mtime are identical
            let size_matches = existing_ep.file_size_bytes == file_size_bytes;
            let mtime_matches = match (&existing_ep.source_modified_at, &source_modified_at) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };

            if size_matches && mtime_matches {
                return Ok(AddEpisodeOutcome {
                    episode_id: existing_ep.id,
                    filename: existing_ep.filename,
                    status: AddEpisodeStatus::AlreadyExists,
                    message: Some("Episode already catalogued and source unchanged.".to_string()),
                });
            }

            // Source changed since previous catalogue: update analysis facts
            let updated_ep = CatalogueEpisode {
                id: existing_ep.id.clone(),
                show_id: show_id.to_string(),
                source_path: source_path.to_string(),
                filename: filename.to_string(),
                file_size_bytes,
                duration_seconds,
                format,
                codec: codec.to_string(),
                sample_rate,
                channels,
                bitrate,
                integrated_loudness_lufs,
                true_peak_dbtp,
                leading_silence_seconds,
                trailing_silence_seconds,
                clipping_evidence,
                overall_assessment_status,
                assessment_profile_id: assessment.profile_id.clone(),
                assessment_profile_version: assessment.profile_version.clone(),
                analysed_at: now.clone(),
                source_modified_at,
                created_at: existing_ep.created_at,
                updated_at: now,
                assessment_json,
                assessment: Some(assessment.clone()),
                source_availability: SourceAvailability::Available,
            };

            repo.update_episode(&updated_ep)?;

            Ok(AddEpisodeOutcome {
                episode_id: existing_ep.id,
                filename: filename.to_string(),
                status: AddEpisodeStatus::Updated,
                message: Some("Catalogue episode updated with new analysis.".to_string()),
            })
        } else {
            // New episode to catalogue
            let episode_id = format!("ep_{}", Uuid::new_v4().simple());
            let new_ep = CatalogueEpisode {
                id: episode_id.clone(),
                show_id: show_id.to_string(),
                source_path: source_path.to_string(),
                filename: filename.to_string(),
                file_size_bytes,
                duration_seconds,
                format,
                codec: codec.to_string(),
                sample_rate,
                channels,
                bitrate,
                integrated_loudness_lufs,
                true_peak_dbtp,
                leading_silence_seconds,
                trailing_silence_seconds,
                clipping_evidence,
                overall_assessment_status,
                assessment_profile_id: assessment.profile_id.clone(),
                assessment_profile_version: assessment.profile_version.clone(),
                analysed_at: now.clone(),
                source_modified_at,
                created_at: now.clone(),
                updated_at: now,
                assessment_json,
                assessment: Some(assessment.clone()),
                source_availability: SourceAvailability::Available,
            };

            repo.insert_episode(&new_ep)?;

            Ok(AddEpisodeOutcome {
                episode_id,
                filename: filename.to_string(),
                status: AddEpisodeStatus::Added,
                message: Some("Episode added to show catalogue.".to_string()),
            })
        }
    }
}

fn extract_source_metadata(source_path: &str, fallback_size: i64) -> (i64, Option<String>) {
    let path = Path::new(source_path);
    if let Ok(metadata) = std::fs::metadata(path) {
        let size = metadata.len() as i64;
        let mtime = metadata
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339());
        (size, mtime)
    } else {
        (fallback_size, None)
    }
}
