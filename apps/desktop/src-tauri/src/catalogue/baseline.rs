use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::catalogue::models::{CatalogueEpisode, Show, SourceAvailability};
use crate::catalogue::stats::{
    calculate_categorical_metric, calculate_continuous_metric, CategoricalBaselineMetric,
    ContinuousBaselineMetric,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BaselineMaturity {
    NoData,
    Early,
    Developing,
    Established,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaselineExclusionSummary {
    pub changed_source_count: usize,
    pub missing_measurement_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClippingBaselineSummary {
    pub total_checked: usize,
    pub none_count: usize,
    pub possible_count: usize,
    pub uncertain_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalMetricPoint {
    pub episode_id: String,
    pub filename: String,
    pub analysed_at: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShowBaseline {
    pub show_id: String,
    pub show_name: String,
    pub maturity: BaselineMaturity,
    pub total_episodes: usize,
    pub eligible_episodes: usize,
    pub excluded_episodes: usize,
    pub exclusion_summary: BaselineExclusionSummary,
    pub generated_at: String,

    // Continuous metrics
    pub loudness: Option<ContinuousBaselineMetric>,
    pub true_peak: Option<ContinuousBaselineMetric>,
    pub duration: Option<ContinuousBaselineMetric>,
    pub leading_silence: Option<ContinuousBaselineMetric>,
    pub trailing_silence: Option<ContinuousBaselineMetric>,
    pub bitrate: Option<ContinuousBaselineMetric>,

    // Categorical metrics
    pub format: Option<CategoricalBaselineMetric>,
    pub sample_rate: Option<CategoricalBaselineMetric>,
    pub channels: Option<CategoricalBaselineMetric>,
    pub codec: Option<CategoricalBaselineMetric>,

    // Qualitative clipping frequency
    pub clipping: ClippingBaselineSummary,

    // Lightweight ordered history foundation for Stage 5D
    pub loudness_history: Vec<HistoricalMetricPoint>,
    pub true_peak_history: Vec<HistoricalMetricPoint>,
}

/// Evaluates baseline maturity based on eligible episode count.
pub fn evaluate_maturity(eligible_count: usize) -> BaselineMaturity {
    match eligible_count {
        0 => BaselineMaturity::NoData,
        1..=2 => BaselineMaturity::Early,
        3..=4 => BaselineMaturity::Developing,
        _ => BaselineMaturity::Established,
    }
}

/// Pure deterministic computation of a Show Baseline from catalogue facts.
/// Zero FFmpeg / FFprobe / Whisper invocations.
pub fn compute_show_baseline(show: &Show, episodes: &[CatalogueEpisode]) -> ShowBaseline {
    let total_episodes = episodes.len();
    let mut eligible_episodes_list: Vec<&CatalogueEpisode> = Vec::new();
    let mut changed_count = 0;

    for ep in episodes {
        match ep.source_availability {
            SourceAvailability::Available | SourceAvailability::Missing => {
                eligible_episodes_list.push(ep);
            }
            SourceAvailability::Changed => {
                changed_count += 1;
            }
        }
    }

    let eligible_count = eligible_episodes_list.len();
    let excluded_count = total_episodes - eligible_count;
    let maturity = evaluate_maturity(eligible_count);

    if eligible_count == 0 {
        return ShowBaseline {
            show_id: show.id.clone(),
            show_name: show.name.clone(),
            maturity,
            total_episodes,
            eligible_episodes: 0,
            excluded_episodes: excluded_count,
            exclusion_summary: BaselineExclusionSummary {
                changed_source_count: changed_count,
                missing_measurement_count: 0,
            },
            generated_at: Utc::now().to_rfc3339(),
            loudness: None,
            true_peak: None,
            duration: None,
            leading_silence: None,
            trailing_silence: None,
            bitrate: None,
            format: None,
            sample_rate: None,
            channels: None,
            codec: None,
            clipping: ClippingBaselineSummary {
                total_checked: 0,
                none_count: 0,
                possible_count: 0,
                uncertain_count: 0,
            },
            loudness_history: Vec::new(),
            true_peak_history: Vec::new(),
        };
    }

    // Collect continuous metric values
    let mut loudness_values = Vec::new();
    let mut true_peak_values = Vec::new();
    let mut duration_values = Vec::new();
    let mut leading_silence_values = Vec::new();
    let mut trailing_silence_values = Vec::new();
    let mut bitrate_values = Vec::new();

    // Collect categorical values
    let mut format_values = Vec::new();
    let mut sample_rate_values = Vec::new();
    let mut channel_values = Vec::new();
    let mut codec_values = Vec::new();

    // Clipping counts
    let mut clipping_none = 0;
    let mut clipping_possible = 0;
    let mut clipping_uncertain = 0;
    let mut clipping_total = 0;

    // Historical points for trend foundation
    let mut loudness_history = Vec::new();
    let mut true_peak_history = Vec::new();

    let mut missing_measurements = 0;

    for ep in &eligible_episodes_list {
        // Continuous
        if let Some(lufs) = ep.integrated_loudness_lufs {
            loudness_values.push(lufs);
            loudness_history.push(HistoricalMetricPoint {
                episode_id: ep.id.clone(),
                filename: ep.filename.clone(),
                analysed_at: ep.analysed_at.clone(),
                value: lufs,
            });
        } else {
            missing_measurements += 1;
        }

        if let Some(tp) = ep.true_peak_dbtp {
            true_peak_values.push(tp);
            true_peak_history.push(HistoricalMetricPoint {
                episode_id: ep.id.clone(),
                filename: ep.filename.clone(),
                analysed_at: ep.analysed_at.clone(),
                value: tp,
            });
        }

        if ep.duration_seconds > 0.0 {
            duration_values.push(ep.duration_seconds);
        }

        leading_silence_values.push(ep.leading_silence_seconds);
        trailing_silence_values.push(ep.trailing_silence_seconds);

        if let Some(br) = ep.bitrate {
            if br > 0 {
                bitrate_values.push(br as f64);
            }
        }

        // Categorical
        format_values.push(format!("{:?}", ep.format).to_uppercase());

        if ep.sample_rate > 0 {
            sample_rate_values.push(format!("{} Hz", ep.sample_rate));
        }

        if ep.channels > 0 {
            let ch_label = match ep.channels {
                1 => "Mono".to_string(),
                2 => "Stereo".to_string(),
                n => format!("{} Channels", n),
            };
            channel_values.push(ch_label);
        }

        if !ep.codec.is_empty() {
            codec_values.push(ep.codec.to_uppercase());
        }

        // Clipping
        clipping_total += 1;
        match ep.clipping_evidence.to_uppercase().as_str() {
            "POSSIBLE" => clipping_possible += 1,
            "UNCERTAIN" => clipping_uncertain += 1,
            _ => clipping_none += 1,
        }
    }

    // Sort historical series chronologically by analysed_at ASC
    loudness_history.sort_by(|a, b| a.analysed_at.cmp(&b.analysed_at));
    true_peak_history.sort_by(|a, b| a.analysed_at.cmp(&b.analysed_at));

    let loudness = calculate_continuous_metric(&loudness_values, "loudness", "Integrated Loudness", "LUFS");
    let true_peak = calculate_continuous_metric(&true_peak_values, "truePeak", "True Peak", "dBTP");
    let duration = calculate_continuous_metric(&duration_values, "duration", "Duration", "seconds");
    let leading_silence = calculate_continuous_metric(
        &leading_silence_values,
        "leadingSilence",
        "Opening Silence",
        "seconds",
    );
    let trailing_silence = calculate_continuous_metric(
        &trailing_silence_values,
        "trailingSilence",
        "Closing Silence",
        "seconds",
    );
    let bitrate = calculate_continuous_metric(&bitrate_values, "bitrate", "Bitrate", "bps");

    let format = calculate_categorical_metric(&format_values, "format", "Format");
    let sample_rate = calculate_categorical_metric(&sample_rate_values, "sampleRate", "Sample Rate");
    let channels = calculate_categorical_metric(&channel_values, "channels", "Channels");
    let codec = calculate_categorical_metric(&codec_values, "codec", "Codec");

    ShowBaseline {
        show_id: show.id.clone(),
        show_name: show.name.clone(),
        maturity,
        total_episodes,
        eligible_episodes: eligible_count,
        excluded_episodes: excluded_count,
        exclusion_summary: BaselineExclusionSummary {
            changed_source_count: changed_count,
            missing_measurement_count: missing_measurements,
        },
        generated_at: Utc::now().to_rfc3339(),
        loudness,
        true_peak,
        duration,
        leading_silence,
        trailing_silence,
        bitrate,
        format,
        sample_rate,
        channels,
        codec,
        clipping: ClippingBaselineSummary {
            total_checked: clipping_total,
            none_count: clipping_none,
            possible_count: clipping_possible,
            uncertain_count: clipping_uncertain,
        },
        loudness_history,
        true_peak_history,
    }
}
