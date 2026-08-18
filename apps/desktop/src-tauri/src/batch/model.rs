use crate::assessment::engine::Assessment;
use crate::media::analysis::AudioMeasurements;
use crate::media::ffprobe::{MediaFormat, MediaInspection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchEpisodeStatus {
    Waiting,
    Inspecting,
    Analysing,
    Assessing,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchEpisode {
    pub id: String,
    pub source_path: String,
    pub filename: String,
    pub status: BatchEpisodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<MediaFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection: Option<MediaInspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<AudioMeasurements>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessment: Option<Assessment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchAnalysisSummary {
    pub total: usize,
    pub complete: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub ready: usize,
    pub attention: usize,
    pub needs_attention: usize,
    pub elapsed_seconds: f64,
}

impl Default for BatchAnalysisSummary {
    fn default() -> Self {
        Self {
            total: 0,
            complete: 0,
            failed: 0,
            cancelled: 0,
            ready: 0,
            attention: 0,
            needs_attention: 0,
            elapsed_seconds: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchJobStatus {
    Queued,
    Running,
    Complete,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchAnalysisJob {
    pub id: String,
    pub status: BatchJobStatus,
    pub episodes: Vec<BatchEpisode>,
    pub summary: BatchAnalysisSummary,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgressPayload {
    pub job_id: String,
    pub episode_id: String,
    pub status: BatchEpisodeStatus,
    pub episode: BatchEpisode,
    pub summary: BatchAnalysisSummary,
}
