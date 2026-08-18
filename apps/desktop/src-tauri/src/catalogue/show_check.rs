use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::assessment::engine::SparklineConfig;
use crate::assessment::engine::SparklineRange;
use crate::catalogue::baseline::{BaselineMaturity, ShowBaseline};
use crate::catalogue::models::CatalogueEpisode;
use crate::catalogue::stats::{CategoricalBaselineMetric, ContinuousBaselineMetric};
use crate::media::ffprobe::MediaSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShowCheckStatus {
    Typical,
    Different,
    InsufficientData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetricComparisonStatus {
    Typical,
    SlightlyDifferent,
    Different,
    NotAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetricDirection {
    BelowUsual,
    WithinUsual,
    AboveUsual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShowCheckContinuousMetric {
    pub id: String,
    pub label: String,
    pub unit: String,
    pub candidate_value: f64,
    pub typical_value: f64,
    pub usual_low: f64,
    pub usual_high: f64,
    pub status: MetricComparisonStatus,
    pub direction: MetricDirection,
    pub message: String,
    pub sample_count: usize,
    pub sparkline: Option<SparklineConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShowCheckCategoricalMetric {
    pub id: String,
    pub label: String,
    pub candidate_value: String,
    pub typical_value: String,
    pub dominant_proportion: f64,
    pub status: MetricComparisonStatus,
    pub message: String,
    pub sample_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShowCheck {
    pub show_id: String,
    pub show_name: String,
    pub baseline_maturity: BaselineMaturity,
    pub baseline_episode_count: usize,
    pub status: ShowCheckStatus,
    pub summary: String,
    pub is_stale: bool,
    pub metrics: Vec<ShowCheckContinuousMetric>,
    pub categorical_metrics: Vec<ShowCheckCategoricalMetric>,
    pub generated_at: String,
}

/// Normalized candidate measurements extracted from an analyzed MediaSource or CatalogueEpisode.
#[derive(Debug, Clone)]
pub struct CandidateMeasurements {
    pub duration_seconds: f64,
    pub format: String,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: Option<f64>,
    pub integrated_loudness_lufs: Option<f64>,
    pub true_peak_dbtp: Option<f64>,
    pub leading_silence_seconds: Option<f64>,
    pub trailing_silence_seconds: Option<f64>,
}

impl CandidateMeasurements {
    pub fn from_media_source(media: &MediaSource) -> Self {
        let measurements = media.measurements.as_ref();
        Self {
            duration_seconds: media.inspection.duration_seconds,
            format: format!("{:?}", media.format),
            codec: media.codec.clone(),
            sample_rate: media.inspection.sample_rate,
            channels: media.inspection.channels,
            bitrate: media.inspection.bitrate.map(|b| b as f64),
            integrated_loudness_lufs: measurements.and_then(|m| m.integrated_loudness_lufs),
            true_peak_dbtp: measurements.and_then(|m| m.true_peak_dbtp),
            leading_silence_seconds: measurements.map(|m| m.leading_silence_seconds),
            trailing_silence_seconds: measurements.map(|m| m.trailing_silence_seconds),
        }
    }

    pub fn from_catalogue_episode(ep: &CatalogueEpisode) -> Self {
        Self {
            duration_seconds: ep.duration_seconds,
            format: format!("{:?}", ep.format),
            codec: ep.codec.clone(),
            sample_rate: ep.sample_rate,
            channels: ep.channels as u32,
            bitrate: ep.bitrate.map(|b| b as f64),
            integrated_loudness_lufs: ep.integrated_loudness_lufs,
            true_peak_dbtp: ep.true_peak_dbtp,
            leading_silence_seconds: Some(ep.leading_silence_seconds),
            trailing_silence_seconds: Some(ep.trailing_silence_seconds),
        }
    }
}

/// Minimum comparison tolerances used for zero-IQR stability and numerical degeneracy.
/// NOTE: These are descriptive comparison tolerances only, NOT publishing standards.
fn get_metric_min_tolerance(metric_id: &str) -> f64 {
    match metric_id {
        "loudness" => 0.5,                                      // 0.5 LU
        "truePeak" | "true_peak" => 0.3,                        // 0.3 dBTP
        "duration" => 5.0,                                      // 5.0 seconds
        "leadingSilence" | "leading_silence" => 0.2,            // 0.2 seconds
        "trailingSilence" | "trailing_silence" => 0.5,          // 0.5 seconds
        "bitrate" => 16.0,                                      // 16.0 kbps
        _ => 0.1,
    }
}

/// Compares a single continuous metric value against baseline quartiles.
fn compare_continuous_metric(
    candidate_opt: Option<f64>,
    baseline_opt: Option<&ContinuousBaselineMetric>,
    maturity: BaselineMaturity,
) -> Option<ShowCheckContinuousMetric> {
    let baseline = baseline_opt?;
    let candidate = candidate_opt?;

    let q1 = baseline.q1;
    let q3 = baseline.q3;
    let median = baseline.median;
    let iqr = q3 - q1;
    let min_tol = get_metric_min_tolerance(&baseline.id);
    let effective_band = if iqr > min_tol { iqr } else { min_tol };

    let (status, direction) = if candidate >= q1 && candidate <= q3 {
        (MetricComparisonStatus::Typical, MetricDirection::WithinUsual)
    } else if candidate < q1 {
        let diff = q1 - candidate;
        let st = if diff <= effective_band {
            MetricComparisonStatus::SlightlyDifferent
        } else {
            MetricComparisonStatus::Different
        };
        (st, MetricDirection::BelowUsual)
    } else {
        let diff = candidate - q3;
        let st = if diff <= effective_band {
            MetricComparisonStatus::SlightlyDifferent
        } else {
            MetricComparisonStatus::Different
        };
        (st, MetricDirection::AboveUsual)
    };

    let message = generate_continuous_message(&baseline.id, status, direction, maturity);

    // Build BulletSparkline configuration
    let pad = effective_band.max(1.0);
    let min_bound = candidate.min(baseline.min).min(q1) - pad;
    let max_bound = candidate.max(baseline.max).max(q3) + pad;

    let sparkline = Some(SparklineConfig {
        min: min_bound,
        max: max_bound,
        target: Some(median),
        value: candidate,
        ranges: vec![SparklineRange {
            from: q1,
            to: q3,
        }],
    });

    Some(ShowCheckContinuousMetric {
        id: baseline.id.clone(),
        label: baseline.label.clone(),
        unit: baseline.unit.clone(),
        candidate_value: candidate,
        typical_value: median,
        usual_low: q1,
        usual_high: q3,
        status,
        direction,
        message,
        sample_count: baseline.sample_count,
        sparkline,
    })
}

/// Generates human copy for continuous metric comparisons tailored to baseline maturity.
fn generate_continuous_message(
    metric_id: &str,
    status: MetricComparisonStatus,
    direction: MetricDirection,
    maturity: BaselineMaturity,
) -> String {
    match metric_id {
        "loudness" => match (status, direction) {
            (MetricComparisonStatus::Typical, _) => match maturity {
                BaselineMaturity::Early => "Within the loudness range of current episodes.".to_string(),
                _ => "Within this Show's usual loudness range.".to_string(),
            },
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::BelowUsual) => match maturity {
                BaselineMaturity::Early => "A little quieter than current episodes in this Show.".to_string(),
                _ => "A little quieter than this Show usually runs.".to_string(),
            },
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::AboveUsual) => match maturity {
                BaselineMaturity::Early => "A little louder than current episodes in this Show.".to_string(),
                _ => "A little louder than this Show usually runs.".to_string(),
            },
            (MetricComparisonStatus::Different, MetricDirection::BelowUsual) => match maturity {
                BaselineMaturity::Early => "Quieter than the episodes currently in this Show.".to_string(),
                _ => "Quieter than this Show usually runs.".to_string(),
            },
            (MetricComparisonStatus::Different, MetricDirection::AboveUsual) => match maturity {
                BaselineMaturity::Early => "Louder than the episodes currently in this Show.".to_string(),
                _ => "Louder than this Show usually runs.".to_string(),
            },
            _ => "Within expected historical range.".to_string(),
        },
        "truePeak" | "true_peak" => match (status, direction) {
            (MetricComparisonStatus::Typical, _) => "Within this Show's usual peak range.".to_string(),
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::BelowUsual) => {
                "Peak levels slightly lower than usual.".to_string()
            }
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::AboveUsual) => {
                "Peak levels slightly higher than usual.".to_string()
            }
            (MetricComparisonStatus::Different, MetricDirection::BelowUsual) => {
                "Peaks are lower than this Show's history.".to_string()
            }
            (MetricComparisonStatus::Different, MetricDirection::AboveUsual) => {
                "Peaks are higher than this Show's history.".to_string()
            }
            _ => "Peak levels within usual range.".to_string(),
        },
        "duration" => match (status, direction) {
            (MetricComparisonStatus::Typical, _) => "Within this Show's usual episode length.".to_string(),
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::BelowUsual) => match maturity {
                BaselineMaturity::Early => "Slightly shorter than current episodes in this Show.".to_string(),
                _ => "Slightly shorter than the current Show baseline.".to_string(),
            },
            (MetricComparisonStatus::SlightlyDifferent, MetricDirection::AboveUsual) => match maturity {
                BaselineMaturity::Early => "Slightly longer than current episodes in this Show.".to_string(),
                _ => "Slightly longer than the current Show baseline.".to_string(),
            },
            (MetricComparisonStatus::Different, MetricDirection::BelowUsual) => match maturity {
                BaselineMaturity::Early => "Shorter than the episodes currently in this Show.".to_string(),
                _ => "Shorter than most episodes in this Show.".to_string(),
            },
            (MetricComparisonStatus::Different, MetricDirection::AboveUsual) => match maturity {
                BaselineMaturity::Early => "Longer than the episodes currently in this Show.".to_string(),
                _ => "Longer than most episodes in this Show.".to_string(),
            },
            _ => "Episode length matches usual duration.".to_string(),
        },
        "leadingSilence" | "leading_silence" => match (status, direction) {
            (MetricComparisonStatus::Typical, _) => "Opening silence matches this Show's usual pacing.".to_string(),
            (MetricComparisonStatus::SlightlyDifferent | MetricComparisonStatus::Different, MetricDirection::BelowUsual) => {
                "Opening silence is shorter than usual.".to_string()
            }
            (MetricComparisonStatus::SlightlyDifferent | MetricComparisonStatus::Different, MetricDirection::AboveUsual) => {
                "Opening silence is longer than usual.".to_string()
            }
            _ => "Opening silence within usual range.".to_string(),
        },
        "trailingSilence" | "trailing_silence" => match (status, direction) {
            (MetricComparisonStatus::Typical, _) => "Closing silence matches this Show's usual pacing.".to_string(),
            (MetricComparisonStatus::SlightlyDifferent | MetricComparisonStatus::Different, MetricDirection::BelowUsual) => {
                "Closing silence is shorter than usual.".to_string()
            }
            (MetricComparisonStatus::SlightlyDifferent | MetricComparisonStatus::Different, MetricDirection::AboveUsual) => {
                "Closing silence is longer than usual.".to_string()
            }
            _ => "Closing silence within usual range.".to_string(),
        },
        "bitrate" => match status {
            MetricComparisonStatus::Typical => "Bitrate matches this Show's usual encoding.".to_string(),
            _ => "Bitrate differs from this Show's usual encoding.".to_string(),
        },
        _ => "Within historical range.".to_string(),
    }
}

/// Compares a single categorical characteristic against the dominant modal value.
fn compare_categorical_metric(
    candidate_val: &str,
    baseline_opt: Option<&CategoricalBaselineMetric>,
) -> Option<ShowCheckCategoricalMetric> {
    let baseline = baseline_opt?;
    let dominant = &baseline.dominant_value;

    let is_match = candidate_val.eq_ignore_ascii_case(dominant);
    let status = if is_match {
        MetricComparisonStatus::Typical
    } else {
        MetricComparisonStatus::Different
    };

    let message = match baseline.id.as_str() {
        "format" => {
            if is_match {
                format!("Matches this Show's usual {} format.", dominant)
            } else {
                format!("Different from this Show's usual {} delivery format.", dominant)
            }
        }
        "channels" => {
            let candidate_label = if candidate_val == "1" || candidate_val.eq_ignore_ascii_case("mono") {
                "mono"
            } else {
                "stereo"
            };
            let dominant_label = if dominant == "1" || dominant.eq_ignore_ascii_case("mono") {
                "mono"
            } else {
                "stereo"
            };

            if is_match {
                format!("Matches this Show's usual {} delivery.", dominant_label)
            } else {
                format!("This episode is {}; this Show is usually {}.", candidate_label, dominant_label)
            }
        }
        "sampleRate" | "sample_rate" => {
            if is_match {
                format!("Matches this Show's usual sample rate ({}).", dominant)
            } else {
                format!("Sample rate ({}) differs from Show usual ({}).", candidate_val, dominant)
            }
        }
        "codec" => {
            if is_match {
                format!("Matches this Show's usual {} codec.", dominant)
            } else {
                format!("Codec ({}) differs from Show usual ({}).", candidate_val, dominant)
            }
        }
        _ => {
            if is_match {
                format!("Matches dominant historical value ({}).", dominant)
            } else {
                format!("Differs from dominant historical value ({}).", dominant)
            }
        }
    };

    Some(ShowCheckCategoricalMetric {
        id: baseline.id.clone(),
        label: baseline.label.clone(),
        candidate_value: candidate_val.to_string(),
        typical_value: dominant.clone(),
        dominant_proportion: baseline.dominant_proportion,
        status,
        message,
        sample_count: baseline.sample_count,
    })
}

/// Pure deterministic Show Check comparison engine.
/// Consumes candidate episode measurements and a ShowBaseline.
/// Zero SQLite writes, zero FFmpeg/FFprobe/Whisper calls.
pub fn run_show_check(
    baseline: &ShowBaseline,
    candidate: &CandidateMeasurements,
    is_stale: bool,
) -> ShowCheck {
    if baseline.maturity == BaselineMaturity::NoData || baseline.eligible_episodes == 0 {
        return ShowCheck {
            show_id: baseline.show_id.clone(),
            show_name: baseline.show_name.clone(),
            baseline_maturity: BaselineMaturity::NoData,
            baseline_episode_count: 0,
            status: ShowCheckStatus::InsufficientData,
            summary: "No baseline history available for comparison.".to_string(),
            is_stale,
            metrics: Vec::new(),
            categorical_metrics: Vec::new(),
            generated_at: Utc::now().to_rfc3339(),
        };
    }

    let maturity = baseline.maturity;
    let mut continuous_metrics = Vec::new();

    // 1. Loudness
    if let Some(m) = compare_continuous_metric(candidate.integrated_loudness_lufs, baseline.loudness.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // 2. True Peak
    if let Some(m) = compare_continuous_metric(candidate.true_peak_dbtp, baseline.true_peak.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // 3. Duration
    if let Some(m) = compare_continuous_metric(Some(candidate.duration_seconds), baseline.duration.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // 4. Leading Silence
    if let Some(m) = compare_continuous_metric(candidate.leading_silence_seconds, baseline.leading_silence.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // 5. Trailing Silence
    if let Some(m) = compare_continuous_metric(candidate.trailing_silence_seconds, baseline.trailing_silence.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // 6. Bitrate (if available in candidate and baseline)
    if let Some(m) = compare_continuous_metric(candidate.bitrate, baseline.bitrate.as_ref(), maturity) {
        continuous_metrics.push(m);
    }

    // Categorical metrics
    let mut categorical_metrics = Vec::new();

    // 1. Format
    if let Some(c) = compare_categorical_metric(&candidate.format.to_uppercase(), baseline.format.as_ref()) {
        categorical_metrics.push(c);
    }

    // 2. Channels
    let candidate_channel_label = match candidate.channels {
        1 => "Mono".to_string(),
        2 => "Stereo".to_string(),
        n => format!("{} Channels", n),
    };
    if let Some(c) = compare_categorical_metric(&candidate_channel_label, baseline.channels.as_ref()) {
        categorical_metrics.push(c);
    }

    // 3. Sample Rate
    let candidate_sr_label = format!("{} Hz", candidate.sample_rate);
    if let Some(c) = compare_categorical_metric(&candidate_sr_label, baseline.sample_rate.as_ref()) {
        categorical_metrics.push(c);
    }

    // 4. Codec
    if let Some(c) = compare_categorical_metric(&candidate.codec.to_uppercase(), baseline.codec.as_ref()) {
        categorical_metrics.push(c);
    }

    // Determine overall ShowCheckStatus
    let has_different = continuous_metrics
        .iter()
        .any(|m| m.status == MetricComparisonStatus::Different)
        || categorical_metrics
            .iter()
            .any(|m| m.status == MetricComparisonStatus::Different);

    let status = if has_different {
        ShowCheckStatus::Different
    } else {
        ShowCheckStatus::Typical
    };

    // Construct human editorial summary
    let summary = generate_deterministic_summary(
        &continuous_metrics,
        &categorical_metrics,
        maturity,
        status,
    );

    ShowCheck {
        show_id: baseline.show_id.clone(),
        show_name: baseline.show_name.clone(),
        baseline_maturity: maturity,
        baseline_episode_count: baseline.eligible_episodes,
        status,
        summary,
        is_stale,
        metrics: continuous_metrics,
        categorical_metrics,
        generated_at: Utc::now().to_rfc3339(),
    }
}

/// Generates a concise deterministic summary from structured comparison results.
/// Prioritises the strongest differences: primary audio characteristics, channel delivery,
/// other delivery formats, and avoids speculative causal claims or robotic listings.
pub fn generate_deterministic_summary(
    continuous_metrics: &[ShowCheckContinuousMetric],
    categorical_metrics: &[ShowCheckCategoricalMetric],
    maturity: BaselineMaturity,
    status: ShowCheckStatus,
) -> String {
    match status {
        ShowCheckStatus::InsufficientData => "No baseline history available for comparison.".to_string(),
        ShowCheckStatus::Typical => {
            let has_slightly_different = continuous_metrics
                .iter()
                .any(|m| m.status == MetricComparisonStatus::SlightlyDifferent);

            if has_slightly_different {
                match maturity {
                    BaselineMaturity::Early => "Broadly consistent with current episodes in this Show.".to_string(),
                    _ => "Within normal variation for this Show with minor differences.".to_string(),
                }
            } else {
                match maturity {
                    BaselineMaturity::Early => "Matches current episodes in this Show.".to_string(),
                    _ => "Matches this Show's usual historical characteristics.".to_string(),
                }
            }
        }
        ShowCheckStatus::Different => {
            let mut clauses: Vec<String> = Vec::new();

            // 1. Primary continuous audio differences
            if let Some(l) = continuous_metrics.iter().find(|m| m.id == "loudness" && m.status == MetricComparisonStatus::Different) {
                let text = match (l.direction, maturity) {
                    (MetricDirection::BelowUsual, BaselineMaturity::Early) => "quieter than current episodes in this Show",
                    (MetricDirection::BelowUsual, _) => "noticeably quieter than this Show usually runs",
                    (MetricDirection::AboveUsual, BaselineMaturity::Early) => "louder than current episodes in this Show",
                    (MetricDirection::AboveUsual, _) => "noticeably louder than this Show usually runs",
                    _ => "has different loudness than usual",
                };
                clauses.push(text.to_string());
            }

            if let Some(d) = continuous_metrics.iter().find(|m| m.id == "duration" && m.status == MetricComparisonStatus::Different) {
                let text = match (d.direction, maturity) {
                    (MetricDirection::BelowUsual, BaselineMaturity::Early) => "shorter than current episodes in this Show",
                    (MetricDirection::BelowUsual, _) => "noticeably shorter than typical episodes in this Show",
                    (MetricDirection::AboveUsual, BaselineMaturity::Early) => "longer than current episodes in this Show",
                    (MetricDirection::AboveUsual, _) => "noticeably longer than typical episodes in this Show",
                    _ => "has different duration than usual",
                };
                clauses.push(text.to_string());
            }

            if let Some(tp) = continuous_metrics.iter().find(|m| (m.id == "truePeak" || m.id == "true_peak") && m.status == MetricComparisonStatus::Different) {
                let text = match tp.direction {
                    MetricDirection::AboveUsual => "peaks higher than this Show's usual range",
                    MetricDirection::BelowUsual => "peaks lower than this Show's usual range",
                    _ => "peak levels differ from Show history",
                };
                clauses.push(text.to_string());
            }

            // 2. Channel delivery difference
            if let Some(ch) = categorical_metrics.iter().find(|m| m.id == "channels" && m.status == MetricComparisonStatus::Different) {
                let cand_is_mono = ch.candidate_value.eq_ignore_ascii_case("mono") || ch.candidate_value == "1";
                let dominant_is_stereo = ch.typical_value.eq_ignore_ascii_case("stereo") || ch.typical_value == "2";
                let cand_is_stereo = ch.candidate_value.eq_ignore_ascii_case("stereo") || ch.candidate_value == "2";
                let dominant_is_mono = ch.typical_value.eq_ignore_ascii_case("mono") || ch.typical_value == "1";

                let text = if cand_is_mono && dominant_is_stereo {
                    "uses mono rather than the usual stereo delivery"
                } else if cand_is_stereo && dominant_is_mono {
                    "uses stereo rather than the usual mono delivery"
                } else {
                    "uses a different channel configuration than usual"
                };
                clauses.push(text.to_string());
            }

            // 3. Format / Sample Rate differences
            if let Some(fmt) = categorical_metrics.iter().find(|m| m.id == "format" && m.status == MetricComparisonStatus::Different) {
                let text = format!("uses {} rather than the usual {} format", fmt.candidate_value, fmt.typical_value);
                clauses.push(text);
            }

            if let Some(sr) = categorical_metrics.iter().find(|m| (m.id == "sampleRate" || m.id == "sample_rate") && m.status == MetricComparisonStatus::Different) {
                let text = format!("sample rate ({}) differs from Show usual ({})", sr.candidate_value, sr.typical_value);
                clauses.push(text);
            }

            // 4. Secondary continuous differences if no primary clauses
            if clauses.is_empty() {
                for m in continuous_metrics.iter().filter(|m| m.status == MetricComparisonStatus::Different) {
                    if m.id.contains("silence") || m.id == "bitrate" {
                        clauses.push(format!("{} differs from Show history", m.label.to_lowercase()));
                    }
                }
            }

            if clauses.is_empty() {
                match maturity {
                    BaselineMaturity::Early => "Differs from the episodes currently in this Show.".to_string(),
                    _ => "Differs from this Show's usual historical characteristics.".to_string(),
                }
            } else {
                let top_clauses: Vec<String> = clauses.into_iter().take(2).collect();
                let combined = top_clauses.join(" and ");
                let mut chars = combined.chars();
                match chars.next() {
                    None => "Differs from this Show's historical characteristics.".to_string(),
                    Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
                }
            }
        }
    }
}
