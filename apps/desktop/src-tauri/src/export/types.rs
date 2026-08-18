use crate::assessment::engine::{Assessment, OverallStatus};
use crate::fixplan::engine::FixActionType;
use crate::media::analysis::AudioMeasurements;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub episode_number: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub artwork_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub destination_directory: String,
    pub include_audio: bool,
    pub include_transcript: bool,
    pub include_report: bool,
    pub metadata: Option<EpisodeMetadata>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    pub path: String,
    pub filename: String,
    pub file_size_bytes: u64,
    pub file_type: String, // "audio" | "transcript" | "report"
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportVerificationResult {
    pub passed: bool,
    pub overall_status: OverallStatus,
    pub summary: string_or_default::FormattedSummary,
    pub measurements: AudioMeasurements,
    pub assessment: Assessment,
}

mod string_or_default {
    pub type FormattedSummary = String;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PodReadyPackage {
    pub package_directory: String,
    pub package_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_file: Option<ExportedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_file: Option<ExportedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_file: Option<ExportedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<EpisodeMetadata>,
    pub artwork_embedded: bool,
    pub verification_result: ExportVerificationResult,
    pub generation_duration_seconds: f64,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportActionItem {
    pub action_type: FixActionType,
    pub title: String,
    pub description: String,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportTranscriptionInfo {
    pub requested: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PublishingJsonReport {
    pub podready_version: String,
    pub created_at: String,
    pub package_name: String,
    pub source_filename: String,
    pub metadata: Option<EpisodeMetadata>,
    pub actions_applied: Vec<ReportActionItem>,
    pub before_measurements: Option<AudioMeasurements>,
    pub before_assessment: Option<Assessment>,
    pub final_mp3_measurements: AudioMeasurements,
    pub final_mp3_assessment: Assessment,
    pub verification_passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription: Option<ReportTranscriptionInfo>,
}
