use serde::{Deserialize, Serialize};
use crate::assessment::Assessment;
use crate::media::ffprobe::MediaFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceAvailability {
    Available,
    Missing,
    Changed,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AddEpisodeStatus {
    Added,
    AlreadyExists,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub episode_count: usize,
    pub last_analysed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEpisode {
    pub id: String,
    pub show_id: String,
    pub source_path: String,
    pub filename: String,
    pub file_size_bytes: i64,
    pub duration_seconds: f64,
    pub format: MediaFormat,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: Option<u32>,
    pub integrated_loudness_lufs: Option<f64>,
    pub true_peak_dbtp: Option<f64>,
    pub leading_silence_seconds: f64,
    pub trailing_silence_seconds: f64,
    pub clipping_evidence: String,
    pub overall_assessment_status: String,
    pub assessment_profile_id: String,
    pub assessment_profile_version: String,
    pub analysed_at: String,
    pub source_modified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub assessment_json: Option<String>,
    pub assessment: Option<Assessment>,
    pub source_availability: SourceAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowWithEpisodes {
    pub show: Show,
    pub episodes: Vec<CatalogueEpisode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddEpisodeOutcome {
    pub episode_id: String,
    pub filename: String,
    pub status: AddEpisodeStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBatchEpisodesResult {
    pub show_id: String,
    pub show_name: String,
    pub total_processed: usize,
    pub added: usize,
    pub updated: usize,
    pub already_exists: usize,
    pub skipped_failed: usize,
    pub outcomes: Vec<AddEpisodeOutcome>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShowInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateShowInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}
