use super::profiles::{get_profile_for_channels, PodcastProfile};
use crate::media::analysis::{AudioMeasurements, ClippingAnalysis, ClippingEvidence};
use crate::media::ffprobe::{MediaFormat, MediaInspection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssessmentStatus {
    Good,
    Attention,
    Issue,
    Info,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverallStatus {
    Ready,
    Attention,
    NeedsAttention,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SparklineRange {
    pub from: f64,
    pub to: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SparklineConfig {
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    pub value: f64,
    pub ranges: Vec<SparklineRange>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentCheck {
    pub id: String,
    pub label: String,
    pub status: AssessmentStatus,
    pub display_value: String,
    pub message: String,
    pub fixable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparkline: Option<SparklineConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub overall_status: OverallStatus,
    pub summary: String,
    pub profile_id: String,
    pub profile_version: String,
    pub profile_name: String,
    pub audio_checks: Vec<AssessmentCheck>,
    pub file_checks: Vec<AssessmentCheck>,
}

fn format_loudness(val: Option<f64>) -> String {
    match val {
        Some(v) if v.is_finite() => {
            let sign = if v < 0.0 { "−" } else { "" };
            format!("{}{:.1} LUFS", sign, v.abs())
        }
        _ => "—".to_string(),
    }
}

fn format_true_peak(val: Option<f64>) -> String {
    match val {
        Some(v) if v.is_finite() => {
            let sign = if v < 0.0 { "−" } else { "" };
            format!("{}{:.1} dBTP", sign, v.abs())
        }
        _ => "—".to_string(),
    }
}

fn format_seconds(seconds: f64) -> String {
    format!("{:.1} sec", seconds)
}

pub fn assess_loudness(loudness: Option<f64>, profile: &PodcastProfile) -> AssessmentCheck {
    let mode_desc = if profile.is_mono { "mono" } else { "stereo" };
    match loudness {
        None => AssessmentCheck {
            id: "loudness".to_string(),
            label: "Integrated loudness".to_string(),
            status: AssessmentStatus::Unknown,
            display_value: "—".to_string(),
            message: "Loudness could not be measured.".to_string(),
            fixable: false,
            sparkline: None,
        },
        Some(v) if !v.is_finite() => AssessmentCheck {
            id: "loudness".to_string(),
            label: "Integrated loudness".to_string(),
            status: AssessmentStatus::Unknown,
            display_value: "—".to_string(),
            message: "Loudness could not be measured (silent or extremely short file).".to_string(),
            fixable: false,
            sparkline: None,
        },
        Some(v) => {
            let sparkline = Some(SparklineConfig {
                min: profile.loudness.sparkline_min,
                max: profile.loudness.sparkline_max,
                target: Some(profile.loudness.target_lufs),
                value: v,
                ranges: vec![SparklineRange {
                    from: profile.loudness.good_min_lufs,
                    to: profile.loudness.good_max_lufs,
                }],
            });

            if v >= profile.loudness.good_min_lufs && v <= profile.loudness.good_max_lufs {
                AssessmentCheck {
                    id: "loudness".to_string(),
                    label: "Integrated loudness".to_string(),
                    status: AssessmentStatus::Good,
                    display_value: format_loudness(Some(v)),
                    message: format!("Safely within recommended range for a {} podcast.", mode_desc),
                    fixable: false,
                    sparkline,
                }
            } else if v > profile.loudness.good_max_lufs && v <= profile.loudness.attention_max_lufs {
                AssessmentCheck {
                    id: "loudness".to_string(),
                    label: "Integrated loudness".to_string(),
                    status: AssessmentStatus::Attention,
                    display_value: format_loudness(Some(v)),
                    message: format!("A little louder than we'd recommend for a {} podcast.", mode_desc),
                    fixable: true,
                    sparkline,
                }
            } else if v < profile.loudness.good_min_lufs && v >= profile.loudness.attention_min_lufs {
                AssessmentCheck {
                    id: "loudness".to_string(),
                    label: "Integrated loudness".to_string(),
                    status: AssessmentStatus::Attention,
                    display_value: format_loudness(Some(v)),
                    message: format!("A little quieter than we'd recommend for a {} podcast.", mode_desc),
                    fixable: true,
                    sparkline,
                }
            } else if v > profile.loudness.attention_max_lufs {
                AssessmentCheck {
                    id: "loudness".to_string(),
                    label: "Integrated loudness".to_string(),
                    status: AssessmentStatus::Issue,
                    display_value: format_loudness(Some(v)),
                    message: "Significantly louder than standard podcast delivery levels.".to_string(),
                    fixable: true,
                    sparkline,
                }
            } else {
                AssessmentCheck {
                    id: "loudness".to_string(),
                    label: "Integrated loudness".to_string(),
                    status: AssessmentStatus::Issue,
                    display_value: format_loudness(Some(v)),
                    message: "Significantly quieter than standard podcast delivery levels.".to_string(),
                    fixable: true,
                    sparkline,
                }
            }
        }
    }
}

pub fn assess_true_peak(true_peak: Option<f64>, profile: &PodcastProfile) -> AssessmentCheck {
    match true_peak {
        None => AssessmentCheck {
            id: "true_peak".to_string(),
            label: "True peak".to_string(),
            status: AssessmentStatus::Unknown,
            display_value: "—".to_string(),
            message: "True peak could not be measured.".to_string(),
            fixable: false,
            sparkline: None,
        },
        Some(v) if !v.is_finite() => AssessmentCheck {
            id: "true_peak".to_string(),
            label: "True peak".to_string(),
            status: AssessmentStatus::Unknown,
            display_value: "—".to_string(),
            message: "True peak could not be measured (silent file).".to_string(),
            fixable: false,
            sparkline: None,
        },
        Some(v) => {
            let sparkline = Some(SparklineConfig {
                min: profile.true_peak.sparkline_min,
                max: profile.true_peak.sparkline_max,
                target: Some(profile.true_peak.ceiling_dbtp),
                value: v,
                ranges: vec![SparklineRange {
                    from: profile.true_peak.sparkline_min,
                    to: profile.true_peak.ceiling_dbtp,
                }],
            });

            if v <= profile.true_peak.ceiling_dbtp {
                AssessmentCheck {
                    id: "true_peak".to_string(),
                    label: "True peak".to_string(),
                    status: AssessmentStatus::Good,
                    display_value: format_true_peak(Some(v)),
                    message: "Safely within range.".to_string(),
                    fixable: false,
                    sparkline,
                }
            } else if v <= profile.true_peak.attention_max_dbtp {
                AssessmentCheck {
                    id: "true_peak".to_string(),
                    label: "True peak".to_string(),
                    status: AssessmentStatus::Attention,
                    display_value: format_true_peak(Some(v)),
                    message: "Your peaks are slightly high for a publishing file.".to_string(),
                    fixable: true,
                    sparkline,
                }
            } else {
                AssessmentCheck {
                    id: "true_peak".to_string(),
                    label: "True peak".to_string(),
                    status: AssessmentStatus::Issue,
                    display_value: format_true_peak(Some(v)),
                    message: "Peak levels exceed recommended ceiling; risk of distortion on streaming platforms.".to_string(),
                    fixable: true,
                    sparkline,
                }
            }
        }
    }
}

pub fn assess_leading_silence(seconds: f64, profile: &PodcastProfile) -> AssessmentCheck {
    if seconds <= profile.silence.leading_good_max_seconds {
        AssessmentCheck {
            id: "leading_silence".to_string(),
            label: "Opening silence".to_string(),
            status: AssessmentStatus::Good,
            display_value: format_seconds(seconds),
            message: "Looks good.".to_string(),
            fixable: false,
            sparkline: None,
        }
    } else if seconds <= profile.silence.leading_attention_max_seconds {
        AssessmentCheck {
            id: "leading_silence".to_string(),
            label: "Opening silence".to_string(),
            status: AssessmentStatus::Attention,
            display_value: format_seconds(seconds),
            message: "Slightly long opening silence.".to_string(),
            fixable: true,
            sparkline: None,
        }
    } else {
        AssessmentCheck {
            id: "leading_silence".to_string(),
            label: "Opening silence".to_string(),
            status: AssessmentStatus::Issue,
            display_value: format_seconds(seconds),
            message: "Excessive opening silence before audio begins.".to_string(),
            fixable: true,
            sparkline: None,
        }
    }
}

pub fn assess_trailing_silence(seconds: f64, profile: &PodcastProfile) -> AssessmentCheck {
    if seconds <= profile.silence.trailing_good_max_seconds {
        AssessmentCheck {
            id: "trailing_silence".to_string(),
            label: "Closing silence".to_string(),
            status: AssessmentStatus::Good,
            display_value: format_seconds(seconds),
            message: "Looks good.".to_string(),
            fixable: false,
            sparkline: None,
        }
    } else if seconds <= profile.silence.trailing_attention_max_seconds {
        AssessmentCheck {
            id: "trailing_silence".to_string(),
            label: "Closing silence".to_string(),
            status: AssessmentStatus::Attention,
            display_value: format_seconds(seconds),
            message: "Slightly long closing silence.".to_string(),
            fixable: true,
            sparkline: None,
        }
    } else {
        AssessmentCheck {
            id: "trailing_silence".to_string(),
            label: "Closing silence".to_string(),
            status: AssessmentStatus::Issue,
            display_value: format_seconds(seconds),
            message: "Excessive trailing silence at the end of the episode.".to_string(),
            fixable: true,
            sparkline: None,
        }
    }
}

pub fn assess_clipping(clipping: &ClippingAnalysis) -> AssessmentCheck {
    match clipping.evidence {
        ClippingEvidence::NONE => AssessmentCheck {
            id: "clipping".to_string(),
            label: "Peak clipping".to_string(),
            status: AssessmentStatus::Good,
            display_value: "None detected".to_string(),
            message: "No obvious clipping detected.".to_string(),
            fixable: false,
            sparkline: None,
        },
        ClippingEvidence::POSSIBLE => AssessmentCheck {
            id: "clipping".to_string(),
            label: "Peak clipping".to_string(),
            status: AssessmentStatus::Attention,
            display_value: if clipping.samples_at_ceiling > 0 {
                format!("Possible ({} flat samples)", clipping.samples_at_ceiling)
            } else {
                "Possible".to_string()
            },
            message: "Some waveform flattening was detected. Review recommended.".to_string(),
            fixable: false,
            sparkline: None,
        },
        ClippingEvidence::UNCERTAIN => AssessmentCheck {
            id: "clipping".to_string(),
            label: "Peak clipping".to_string(),
            status: AssessmentStatus::Info,
            display_value: "Uncertain (lossy source)".to_string(),
            message: "Uncertain — cannot be determined confidently from this lossy source.".to_string(),
            fixable: false,
            sparkline: None,
        },
    }
}

pub fn assess_sample_rate(sample_rate: u32) -> AssessmentCheck {
    let hz_str = format!("{:.1} kHz", (sample_rate as f64) / 1000.0).replace(".0", "");
    if sample_rate == 44100 || sample_rate == 48000 {
        AssessmentCheck {
            id: "sample_rate".to_string(),
            label: "Sample rate".to_string(),
            status: AssessmentStatus::Good,
            display_value: hz_str,
            message: "Standard podcast sample rate.".to_string(),
            fixable: false,
            sparkline: None,
        }
    } else if sample_rate >= 32000 {
        AssessmentCheck {
            id: "sample_rate".to_string(),
            label: "Sample rate".to_string(),
            status: AssessmentStatus::Attention,
            display_value: hz_str,
            message: "Lower than standard podcast sample rate (44.1 or 48 kHz recommended).".to_string(),
            fixable: true,
            sparkline: None,
        }
    } else {
        AssessmentCheck {
            id: "sample_rate".to_string(),
            label: "Sample rate".to_string(),
            status: AssessmentStatus::Issue,
            display_value: hz_str,
            message: "Unusually low sample rate for podcast delivery.".to_string(),
            fixable: true,
            sparkline: None,
        }
    }
}

pub fn assess_channels(channels: u32) -> AssessmentCheck {
    let chan_str = match channels {
        1 => "Mono".to_string(),
        2 => "Stereo".to_string(),
        n => format!("{} Channels", n),
    };

    if channels == 1 || channels == 2 {
        AssessmentCheck {
            id: "channels".to_string(),
            label: "Channel configuration".to_string(),
            status: AssessmentStatus::Good,
            display_value: chan_str,
            message: "Standard podcast channel configuration.".to_string(),
            fixable: false,
            sparkline: None,
        }
    } else {
        AssessmentCheck {
            id: "channels".to_string(),
            label: "Channel configuration".to_string(),
            status: AssessmentStatus::Attention,
            display_value: chan_str,
            message: "Multi-channel audio is not recommended for podcast distribution feeds.".to_string(),
            fixable: true,
            sparkline: None,
        }
    }
}

pub fn assess_format(format: &MediaFormat, _codec: &str) -> AssessmentCheck {
    let (fmt_str, status, msg) = match format {
        MediaFormat::WAV => ("WAV", AssessmentStatus::Good, "Uncompressed PCM audio."),
        MediaFormat::MP3 => ("MP3", AssessmentStatus::Good, "Standard distribution format."),
        MediaFormat::M4A => ("M4A / AAC", AssessmentStatus::Good, "High-efficiency audio container."),
        MediaFormat::MOV => ("MOV", AssessmentStatus::Info, "Video container (audio will be extracted)."),
        MediaFormat::MP4 => ("MP4", AssessmentStatus::Info, "Video container (audio will be extracted)."),
        MediaFormat::UNKNOWN => ("Unknown", AssessmentStatus::Attention, "Unrecognized audio container."),
    };

    AssessmentCheck {
        id: "format".to_string(),
        label: "Format".to_string(),
        status,
        display_value: fmt_str.to_string(),
        message: msg.to_string(),
        fixable: false,
        sparkline: None,
    }
}

pub fn assess_bitrate(bitrate: Option<u32>, channels: u32) -> Option<AssessmentCheck> {
    let br = bitrate?;
    let kbps = (br as f64) / 1000.0;
    let kbps_str = format!("{:.0} kbps", kbps);

    let (status, msg) = if channels <= 1 {
        if kbps >= 64.0 {
            (AssessmentStatus::Good, "Appropriate mono delivery bitrate.")
        } else {
            (AssessmentStatus::Attention, "Low bitrate for podcast delivery.")
        }
    } else if kbps >= 96.0 {
        (AssessmentStatus::Good, "Appropriate stereo delivery bitrate.")
    } else {
        (AssessmentStatus::Attention, "Low bitrate for high-quality stereo podcast audio.")
    };

    Some(AssessmentCheck {
        id: "bitrate".to_string(),
        label: "Bitrate".to_string(),
        status,
        display_value: kbps_str,
        message: msg.to_string(),
        fixable: false,
        sparkline: None,
    })
}

pub fn derive_overall_status(
    audio_checks: &[AssessmentCheck],
    file_checks: &[AssessmentCheck],
) -> (OverallStatus, String) {
    let mut issue_count = 0;
    let mut attention_count = 0;

    for check in audio_checks.iter().chain(file_checks.iter()) {
        match check.status {
            AssessmentStatus::Issue => issue_count += 1,
            AssessmentStatus::Attention => attention_count += 1,
            _ => {}
        }
    }

    let total_concerns = issue_count + attention_count;

    if issue_count > 0 {
        let msg = if total_concerns == 1 {
            "1 thing needs attention".to_string()
        } else {
            format!("{} things need attention", total_concerns)
        };
        (OverallStatus::NeedsAttention, msg)
    } else if attention_count > 0 {
        let msg = if attention_count == 1 {
            "1 thing needs attention".to_string()
        } else {
            format!("{} things need attention", attention_count)
        };
        (OverallStatus::Attention, msg)
    } else {
        (OverallStatus::Ready, "Ready for publication".to_string())
    }
}

pub fn assess_media(
    inspection: &MediaInspection,
    measurements: Option<&AudioMeasurements>,
    format: &MediaFormat,
    codec: &str,
) -> Assessment {
    let profile = get_profile_for_channels(inspection.channels);

    let mut audio_checks = Vec::new();
    if let Some(meas) = measurements {
        audio_checks.push(assess_loudness(meas.integrated_loudness_lufs, profile));
        audio_checks.push(assess_true_peak(meas.true_peak_dbtp, profile));
        audio_checks.push(assess_leading_silence(meas.leading_silence_seconds, profile));
        audio_checks.push(assess_trailing_silence(meas.trailing_silence_seconds, profile));
        audio_checks.push(assess_clipping(&meas.clipping));
    }

    let mut file_checks = Vec::new();
    file_checks.push(assess_format(format, codec));
    file_checks.push(assess_sample_rate(inspection.sample_rate));
    file_checks.push(assess_channels(inspection.channels));
    if let Some(br_check) = assess_bitrate(inspection.bitrate, inspection.channels) {
        file_checks.push(br_check);
    }

    let (overall_status, summary) = derive_overall_status(&audio_checks, &file_checks);

    Assessment {
        overall_status,
        summary,
        profile_id: profile.id.to_string(),
        profile_version: profile.version.to_string(),
        profile_name: profile.name.to_string(),
        audio_checks,
        file_checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::profiles::{PODCAST_MONO_V1, PODCAST_STEREO_V1};

    fn make_clipping(evidence: ClippingEvidence, samples: u64, flat_factor: f64) -> ClippingAnalysis {
        ClippingAnalysis {
            sample_peak_dbfs: Some(-0.1),
            samples_at_ceiling: samples,
            flat_factor,
            evidence,
        }
    }

    // Loudness tests
    #[test]
    fn test_loudness_assessment_stereo() {
        let profile = &PODCAST_STEREO_V1;

        // Comfortably within target (-16.0 target, [-17.5, -14.5] good)
        let check = assess_loudness(Some(-16.0), profile);
        assert_eq!(check.status, AssessmentStatus::Good);
        assert!(check.message.contains("Safely within recommended range"));
        assert!(check.sparkline.is_some());

        let check_edge = assess_loudness(Some(-14.5), profile);
        assert_eq!(check_edge.status, AssessmentStatus::Good);

        // Slightly loud (-14.4 to -13.0)
        let check_loud = assess_loudness(Some(-14.2), profile);
        assert_eq!(check_loud.status, AssessmentStatus::Attention);
        assert!(check_loud.message.contains("A little louder"));

        // Materially loud (> -13.0)
        let check_mat_loud = assess_loudness(Some(-11.5), profile);
        assert_eq!(check_mat_loud.status, AssessmentStatus::Issue);
        assert!(check_mat_loud.message.contains("Significantly louder"));

        // Slightly quiet (-17.6 to -20.0)
        let check_quiet = assess_loudness(Some(-18.5), profile);
        assert_eq!(check_quiet.status, AssessmentStatus::Attention);
        assert!(check_quiet.message.contains("A little quieter"));

        // Materially quiet (< -20.0)
        let check_mat_quiet = assess_loudness(Some(-22.0), profile);
        assert_eq!(check_mat_quiet.status, AssessmentStatus::Issue);
        assert!(check_mat_quiet.message.contains("Significantly quieter"));

        // Unavailable / None
        let check_none = assess_loudness(None, profile);
        assert_eq!(check_none.status, AssessmentStatus::Unknown);

        // Infinite / NaN
        let check_inf = assess_loudness(Some(f64::NEG_INFINITY), profile);
        assert_eq!(check_inf.status, AssessmentStatus::Unknown);
    }

    #[test]
    fn test_loudness_assessment_mono() {
        let profile = &PODCAST_MONO_V1;

        // Comfortably within target (-19.0 target, [-20.5, -17.5] good)
        let check = assess_loudness(Some(-19.0), profile);
        assert_eq!(check.status, AssessmentStatus::Good);
        assert!(check.message.contains("mono podcast"));

        // Slightly loud (-17.4 to -16.0)
        let check_loud = assess_loudness(Some(-17.0), profile);
        assert_eq!(check_loud.status, AssessmentStatus::Attention);

        // Materially loud (> -16.0)
        let check_mat_loud = assess_loudness(Some(-14.0), profile);
        assert_eq!(check_mat_loud.status, AssessmentStatus::Issue);

        // Slightly quiet (-20.6 to -23.0)
        let check_quiet = assess_loudness(Some(-21.0), profile);
        assert_eq!(check_quiet.status, AssessmentStatus::Attention);

        // Materially quiet (< -23.0)
        let check_mat_quiet = assess_loudness(Some(-25.0), profile);
        assert_eq!(check_mat_quiet.status, AssessmentStatus::Issue);
    }

    // True Peak tests
    #[test]
    fn test_true_peak_assessment() {
        let profile = &PODCAST_STEREO_V1;

        // Safely below target (<= -1.5 dBTP)
        let check_safe = assess_true_peak(Some(-1.9), profile);
        assert_eq!(check_safe.status, AssessmentStatus::Good);
        assert!(check_safe.message.contains("Safely within range"));

        // Approaching limit (-1.4 to -0.5 dBTP)
        let check_warn = assess_true_peak(Some(-1.0), profile);
        assert_eq!(check_warn.status, AssessmentStatus::Attention);
        assert!(check_warn.message.contains("peaks are slightly high"));

        // Over limit (> -0.5 dBTP)
        let check_over = assess_true_peak(Some(-0.2), profile);
        assert_eq!(check_over.status, AssessmentStatus::Issue);
        assert!(check_over.message.contains("recommended ceiling"));

        // Unavailable
        let check_none = assess_true_peak(None, profile);
        assert_eq!(check_none.status, AssessmentStatus::Unknown);
    }

    // Silence tests
    #[test]
    fn test_silence_assessment() {
        let profile = &PODCAST_STEREO_V1;

        // Normal small boundary silence
        assert_eq!(assess_leading_silence(0.2, profile).status, AssessmentStatus::Good);
        assert_eq!(assess_trailing_silence(0.8, profile).status, AssessmentStatus::Good);

        // Moderately long opening silence (2.1 to 5.0)
        let lead_attn = assess_leading_silence(3.5, profile);
        assert_eq!(lead_attn.status, AssessmentStatus::Attention);
        assert!(lead_attn.message.contains("Slightly long"));

        // Excessive opening silence (> 5.0)
        let lead_issue = assess_leading_silence(6.2, profile);
        assert_eq!(lead_issue.status, AssessmentStatus::Issue);
        assert!(lead_issue.message.contains("Excessive"));

        // Moderately long closing silence (4.1 to 8.0)
        let trail_attn = assess_trailing_silence(5.5, profile);
        assert_eq!(trail_attn.status, AssessmentStatus::Attention);
        assert!(trail_attn.message.contains("Slightly long"));

        // Excessive closing silence (> 8.0)
        let trail_issue = assess_trailing_silence(10.0, profile);
        assert_eq!(trail_issue.status, AssessmentStatus::Issue);
        assert!(trail_issue.message.contains("Excessive"));
    }

    // Clipping tests
    #[test]
    fn test_clipping_assessment() {
        // NONE
        let check_none = assess_clipping(&make_clipping(ClippingEvidence::NONE, 0, 0.0));
        assert_eq!(check_none.status, AssessmentStatus::Good);
        assert_eq!(check_none.display_value, "None detected");
        assert!(check_none.message.contains("No obvious clipping detected"));

        // POSSIBLE -> Attention (waveform flattening detected)
        let check_poss = assess_clipping(&make_clipping(ClippingEvidence::POSSIBLE, 3200, 14.5));
        assert_eq!(check_poss.status, AssessmentStatus::Attention);
        assert!(check_poss.message.contains("Some waveform flattening was detected. Review recommended."));

        // UNCERTAIN -> Info (lossy source)
        let check_unc = assess_clipping(&make_clipping(ClippingEvidence::UNCERTAIN, 120, 0.0));
        assert_eq!(check_unc.status, AssessmentStatus::Info);
        assert!(check_unc.message.contains("cannot be determined confidently"));
    }

    // Calibration Fixture Set: 6 deterministic scenarios (isolated from FFmpeg)
    #[test]
    fn test_calibration_scenario_1_healthy_podcast() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: Some(192000),
            file_size_bytes: 7200000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.2,
            clipping: make_clipping(ClippingEvidence::NONE, 0, 0.0),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::MP3, "mp3");
        assert_eq!(assessment.overall_status, OverallStatus::Ready);
        assert_eq!(assessment.summary, "Ready for publication");
    }

    #[test]
    fn test_calibration_scenario_2_slightly_loud_podcast() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: Some(192000),
            file_size_bytes: 7200000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-14.0), // Attention range for stereo: (-14.5, -13.0]
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.2,
            clipping: make_clipping(ClippingEvidence::NONE, 0, 0.0),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::MP3, "mp3");
        assert_eq!(assessment.overall_status, OverallStatus::Attention);
        assert_eq!(assessment.summary, "1 thing needs attention");
        let l_check = assessment.audio_checks.iter().find(|c| c.id == "loudness").unwrap();
        assert_eq!(l_check.status, AssessmentStatus::Attention);
        assert!(l_check.message.contains("A little louder"));
    }

    #[test]
    fn test_calibration_scenario_3_slightly_high_true_peak() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 48000,
            channels: 2,
            bitrate: None,
            file_size_bytes: 28800000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-1.0), // Attention range: (-1.5, -0.5] dBTP
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.2,
            clipping: make_clipping(ClippingEvidence::NONE, 0, 0.0),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::WAV, "pcm_s16le");
        assert_eq!(assessment.overall_status, OverallStatus::Attention);
        assert_eq!(assessment.summary, "1 thing needs attention");
        let tp_check = assessment.audio_checks.iter().find(|c| c.id == "true_peak").unwrap();
        assert_eq!(tp_check.status, AssessmentStatus::Attention);
        assert!(tp_check.message.contains("Your peaks are slightly high for a publishing file."));
    }

    #[test]
    fn test_calibration_scenario_4_possible_clipping_evidence() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: None,
            file_size_bytes: 26460000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.2,
            clipping: make_clipping(ClippingEvidence::POSSIBLE, 1200, 8.5),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::WAV, "pcm_s16le");
        assert_eq!(assessment.overall_status, OverallStatus::Attention);
        assert_eq!(assessment.summary, "1 thing needs attention");
        let clip_check = assessment.audio_checks.iter().find(|c| c.id == "clipping").unwrap();
        assert_eq!(clip_check.status, AssessmentStatus::Attention);
        assert!(clip_check.message.contains("Some waveform flattening was detected. Review recommended."));
    }

    #[test]
    fn test_calibration_scenario_5_confirmed_severe_issue() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: None,
            file_size_bytes: 26460000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-11.5), // Issue (> -13.0)
            true_peak_dbtp: Some(0.3),            // Issue (> -0.5 dBTP)
            leading_silence_seconds: 6.5,         // Issue (> 5.0s)
            trailing_silence_seconds: 10.0,       // Issue (> 8.0s)
            clipping: make_clipping(ClippingEvidence::POSSIBLE, 5000, 25.0),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::WAV, "pcm_s16le");
        assert_eq!(assessment.overall_status, OverallStatus::NeedsAttention);
        assert!(assessment.summary.contains("things need attention"));
    }

    #[test]
    fn test_calibration_scenario_6_lossy_uncertain_clipping_case() {
        let inspection = MediaInspection {
            duration_seconds: 300.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: Some(128000),
            file_size_bytes: 4800000,
        };
        let measurements = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-1.8),
            leading_silence_seconds: 0.8,
            trailing_silence_seconds: 1.5,
            clipping: make_clipping(ClippingEvidence::UNCERTAIN, 180, 0.0),
        };
        let assessment = assess_media(&inspection, Some(&measurements), &MediaFormat::MP3, "mp3");
        // UNCERTAIN is INFO, does not fail or trigger attention if rest of episode is healthy
        assert_eq!(assessment.overall_status, OverallStatus::Ready);
        assert_eq!(assessment.summary, "Ready for publication");
        let clip_check = assessment.audio_checks.iter().find(|c| c.id == "clipping").unwrap();
        assert_eq!(clip_check.status, AssessmentStatus::Info);
        assert!(clip_check.message.contains("cannot be determined confidently"));
    }

    // Overall assessment tests
    #[test]
    fn test_overall_status_combinations() {
        let inspection = MediaInspection {
            duration_seconds: 180.0,
            sample_rate: 44100,
            channels: 2,
            bitrate: Some(320000),
            file_size_bytes: 5000000,
        };

        // Healthy episode -> READY
        let healthy_meas = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.0,
            clipping: make_clipping(ClippingEvidence::NONE, 0, 0.0),
        };
        let healthy_assessment = assess_media(&inspection, Some(&healthy_meas), &MediaFormat::MP3, "mp3");
        assert_eq!(healthy_assessment.overall_status, OverallStatus::Ready);
        assert_eq!(healthy_assessment.summary, "Ready for publication");

        // Uncertain lossy clipping should NOT cause failure -> READY
        let uncertain_lossy_meas = AudioMeasurements {
            integrated_loudness_lufs: Some(-16.0),
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.0,
            clipping: make_clipping(ClippingEvidence::UNCERTAIN, 150, 0.0),
        };
        let uncertain_assessment = assess_media(&inspection, Some(&uncertain_lossy_meas), &MediaFormat::MP3, "mp3");
        assert_eq!(uncertain_assessment.overall_status, OverallStatus::Ready);

        // Moderate concern (e.g. slightly loud loudness) -> ATTENTION
        let attn_meas = AudioMeasurements {
            integrated_loudness_lufs: Some(-14.2),
            true_peak_dbtp: Some(-2.0),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.0,
            clipping: make_clipping(ClippingEvidence::NONE, 0, 0.0),
        };
        let attn_assessment = assess_media(&inspection, Some(&attn_meas), &MediaFormat::MP3, "mp3");
        assert_eq!(attn_assessment.overall_status, OverallStatus::Attention);
        assert_eq!(attn_assessment.summary, "1 thing needs attention");

        // Significant concern (e.g. true peak clipping / excessive loudness) -> NEEDS_ATTENTION
        let issue_meas = AudioMeasurements {
            integrated_loudness_lufs: Some(-11.0),
            true_peak_dbtp: Some(-0.1),
            leading_silence_seconds: 0.5,
            trailing_silence_seconds: 1.0,
            clipping: make_clipping(ClippingEvidence::POSSIBLE, 2500, 10.0),
        };
        let issue_assessment = assess_media(&inspection, Some(&issue_meas), &MediaFormat::MP3, "mp3");
        assert_eq!(issue_assessment.overall_status, OverallStatus::NeedsAttention);
        assert!(issue_assessment.summary.contains("need attention"));
    }

    #[test]
    fn test_real_audio_pipeline_e2e() {
        use crate::media::analysis::analyse_audio;
        use crate::media::ffprobe::inspect_media;
        use std::process::Command;

        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_stage3_real_file.wav");

        let gen = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=2.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen {
            if output.status.success() {
                let inspected = inspect_media(&test_wav).expect("inspect_media should succeed");
                assert_eq!(inspected.format, MediaFormat::WAV);
                assert_eq!(inspected.inspection.sample_rate, 44100);
                assert_eq!(inspected.inspection.channels, 2);

                let measurements = analyse_audio(&test_wav, inspected.inspection.duration_seconds)
                    .expect("analyse_audio should succeed");
                
                let assessment = assess_media(
                    &inspected.inspection,
                    Some(&measurements),
                    &inspected.format,
                    &inspected.codec,
                );

                assert_eq!(assessment.profile_id, "podcast-stereo-v1");
                assert_eq!(assessment.profile_version, "1.0.0");
                assert_eq!(assessment.profile_name, "Podcast — Stereo");
                assert!(!assessment.audio_checks.is_empty());
                assert!(!assessment.file_checks.is_empty());

                let _ = std::fs::remove_file(test_wav);
            }
        }
    }
}
