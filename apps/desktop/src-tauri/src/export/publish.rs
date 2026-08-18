use crate::assessment::engine::{assess_media, Assessment};
use crate::error::AppError;
use crate::export::package::create_publishing_package;
use crate::export::types::{ExportOptions, PodReadyPackage, ReportActionItem};
use crate::fixplan::engine::generate_fix_plan;
use crate::media::analysis::{analyse_audio, AudioMeasurements};
use crate::media::ffprobe::inspect_media;
use crate::media::processing::execute_fix_plan;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublishingEpisodeStage {
    Preparing,
    Processing,
    Verifying,
    Exporting,
    Transcribing,
    Packaging,
}

impl std::fmt::Display for PublishingEpisodeStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparing => write!(f, "PREPARING"),
            Self::Processing => write!(f, "PROCESSING"),
            Self::Verifying => write!(f, "VERIFYING"),
            Self::Exporting => write!(f, "EXPORTING"),
            Self::Transcribing => write!(f, "TRANSCRIBING"),
            Self::Packaging => write!(f, "PACKAGING"),
        }
    }
}

/// Authoritative unified single-episode publishing service.
/// Orchestrates facts inspection -> assessment -> FixPlan -> processing -> verification -> MP3 -> Whisper -> TXT -> Report -> Package.
/// Original source files are strictly preserved and never modified.
pub fn publish_single_episode(
    source_path: &str,
    destination_directory: &str,
    options: &ExportOptions,
    cached_measurements: Option<AudioMeasurements>,
    cached_assessment: Option<Assessment>,
    is_cancelled: Option<&Arc<AtomicBool>>,
    active_pids: Option<&Arc<Mutex<HashSet<u32>>>>,
    on_stage: Option<&dyn Fn(PublishingEpisodeStage)>,
) -> Result<PodReadyPackage, AppError> {
    if let Some(cancelled) = is_cancelled {
        if cancelled.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
    }

    if let Some(cb) = on_stage {
        cb(PublishingEpisodeStage::Preparing);
    }

    let source_file = Path::new(source_path);
    if !source_file.exists() {
        return Err(AppError::SystemError(format!(
            "Source file does not exist at {}",
            source_path
        )));
    }

    // Phase 1: Determine initial inspection, measurements, and assessment
    let inspection = inspect_media(source_path)?;
    let initial_measurements = match cached_measurements {
        Some(m) => m,
        None => {
            if let (Some(cancelled), Some(pids)) = (is_cancelled, active_pids) {
                crate::batch::engine::analyse_audio_cancellable(
                    source_file,
                    inspection.inspection.duration_seconds,
                    cancelled,
                    pids,
                )?
            } else {
                analyse_audio(source_path, inspection.inspection.duration_seconds)?
            }
        }
    };

    let initial_assessment = match cached_assessment {
        Some(a) => a,
        None => assess_media(
            &inspection.inspection,
            Some(&initial_measurements),
            &inspection.format,
            &inspection.codec,
        ),
    };

    if let Some(cancelled) = is_cancelled {
        if cancelled.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
    }

    // Phase 2: FixPlan generation
    let plan = generate_fix_plan(&initial_assessment);

    // Phase 3: Processing (if required by FixPlan)
    let mut candidate_to_cleanup: Option<String> = None;
    let (input_audio_path, applied_actions) = if plan.changes_audio {
        if let Some(cb) = on_stage {
            cb(PublishingEpisodeStage::Processing);
        }

        let response = execute_fix_plan(
            source_path,
            &plan,
            Some(initial_measurements.clone()),
            Some(initial_assessment.clone()),
        )?;

        if response.candidate_path != source_path {
            candidate_to_cleanup = Some(response.candidate_path.clone());
        }

        if let Some(cb) = on_stage {
            cb(PublishingEpisodeStage::Verifying);
        }

        let actions = response
            .result
            .actions_applied
            .into_iter()
            .map(|a| ReportActionItem {
                action_type: a.action_type,
                title: a.title,
                description: a.description,
                success: a.success,
            })
            .collect();

        (response.candidate_path, actions)
    } else {
        // Audio already meets target profile
        (source_path.to_string(), Vec::new())
    };

    if let Some(cancelled) = is_cancelled {
        if cancelled.load(Ordering::SeqCst) {
            if let Some(ref temp_path) = candidate_to_cleanup {
                let _ = std::fs::remove_file(temp_path);
            }
            return Err(AppError::Cancelled);
        }
    }

    // Phase 4: Create publishing package (MP3, Verification, Whisper, TXT, Report)
    if let Some(cb) = on_stage {
        cb(PublishingEpisodeStage::Exporting);
    }

    let mut episode_export_options = options.clone();
    episode_export_options.destination_directory = destination_directory.to_string();

    let package_result = create_publishing_package(
        &input_audio_path,
        source_path,
        &episode_export_options,
        Some(initial_measurements),
        Some(initial_assessment),
        applied_actions,
    );

    // Phase 5: Workspace Cleanup
    if let Some(ref temp_path) = candidate_to_cleanup {
        if Path::new(temp_path).exists() && temp_path != source_path {
            let _ = std::fs::remove_file(temp_path);
        }
    }

    package_result
}
