use crate::assessment::engine::{Assessment, AssessmentStatus};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FixConfidence {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FixActionType {
    LoudnessAdjustment,
    PeakProtection,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixAction {
    pub id: String,
    pub action_type: FixActionType,
    pub source_check_id: String,
    pub title: String,
    pub description: String,
    pub reason: String,
    pub confidence: FixConfidence,
    pub changes_audio: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_value: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixPlan {
    pub summary: String,
    pub actions: Vec<FixAction>,
    pub review_advisories: Vec<String>,
    pub changes_audio: bool,
    pub total_fixes: usize,
}

/// Generates a deterministic FixPlan from an Assessment.
/// The FixPlan engine consumes the assessment results and decides what changes
/// can be safely and predictably planned.
pub fn generate_fix_plan(assessment: &Assessment) -> FixPlan {
    let mut actions = Vec::new();
    let mut review_advisories = Vec::new();

    for check in &assessment.audio_checks {
        match check.id.as_str() {
            "loudness" => {
                if (check.status == AssessmentStatus::Attention
                    || check.status == AssessmentStatus::Issue)
                    && check.display_value != "—"
                {
                    let target_str = if let Some(sparkline) = &check.sparkline {
                        if let Some(target) = sparkline.target {
                            let sign = if target < 0.0 { "−" } else { "" };
                            format!("{}{:.1} LUFS target", sign, target.abs())
                        } else {
                            "Profile target LUFS".to_string()
                        }
                    } else {
                        "Profile target LUFS".to_string()
                    };

                    actions.push(FixAction {
                        id: "adjust_loudness".to_string(),
                        action_type: FixActionType::LoudnessAdjustment,
                        source_check_id: check.id.clone(),
                        title: "Adjust loudness".to_string(),
                        description: format!(
                            "Adjust integrated loudness from {} to {}.",
                            check.display_value, target_str
                        ),
                        reason: check.message.clone(),
                        confidence: FixConfidence::High,
                        changes_audio: true,
                        from_value: Some(check.display_value.clone()),
                        to_value: Some(target_str),
                    });
                }
            }
            "true_peak" => {
                if (check.status == AssessmentStatus::Attention
                    || check.status == AssessmentStatus::Issue)
                    && check.display_value != "—"
                {
                    let ceiling_str = "≤ −1.5 dBTP ceiling".to_string();

                    actions.push(FixAction {
                        id: "peak_protection".to_string(),
                        action_type: FixActionType::PeakProtection,
                        source_check_id: check.id.clone(),
                        title: "Apply peak protection".to_string(),
                        description:
                            "Apply peak protection during final encoding to ensure safe headroom."
                                .to_string(),
                        reason: check.message.clone(),
                        confidence: FixConfidence::High,
                        changes_audio: true,
                        from_value: Some(check.display_value.clone()),
                        to_value: Some(ceiling_str),
                    });
                }
            }
            "clipping"
                if check.status == AssessmentStatus::Attention
                    || check.status == AssessmentStatus::Issue =>
            {
                // Clipping is never automatically fixed; generate review advisory instead.
                review_advisories.push(
                    "Possible waveform flattening detected. Automatic clipping repair is not supported because it may alter the original recording."
                        .to_string(),
                );
            }
            _ => {
                // Other checks like opening/closing silence, format, sample rate, etc.
                // are intentionally not automated in V1 FixPlan.
            }
        }
    }

    let total_fixes = actions.len();
    let changes_audio = actions.iter().any(|a| a.changes_audio);

    let summary = if total_fixes == 0 {
        if review_advisories.is_empty() {
            "No changes required. Your episode already meets the PodReady profile.".to_string()
        } else {
            "No automatic changes available. Review recommended.".to_string()
        }
    } else if total_fixes == 1 {
        "1 change recommended".to_string()
    } else {
        format!("{} changes recommended", total_fixes)
    };

    FixPlan {
        summary,
        actions,
        review_advisories,
        changes_audio,
        total_fixes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::engine::{AssessmentCheck, OverallStatus, SparklineConfig, SparklineRange};

    fn make_check(
        id: &str,
        label: &str,
        status: AssessmentStatus,
        display_value: &str,
        message: &str,
        target: Option<f64>,
    ) -> AssessmentCheck {
        AssessmentCheck {
            id: id.to_string(),
            label: label.to_string(),
            status,
            display_value: display_value.to_string(),
            message: message.to_string(),
            fixable: true,
            sparkline: target.map(|t| SparklineConfig {
                min: -30.0,
                max: -10.0,
                target: Some(t),
                value: -16.0,
                ranges: vec![SparklineRange {
                    from: -17.5,
                    to: -14.5,
                }],
            }),
        }
    }

    fn make_assessment(audio_checks: Vec<AssessmentCheck>, file_checks: Vec<AssessmentCheck>) -> Assessment {
        Assessment {
            overall_status: OverallStatus::Ready,
            summary: "Ready for publication".to_string(),
            profile_id: "podcast-stereo-v1".to_string(),
            profile_version: "1.0.0".to_string(),
            profile_name: "Podcast — Stereo".to_string(),
            audio_checks,
            file_checks,
        }
    }

    #[test]
    fn test_healthy_episode_no_actions() {
        let assessment = make_assessment(
            vec![
                make_check("loudness", "Integrated loudness", AssessmentStatus::Good, "−16.1 LUFS", "Safely within recommended range.", Some(-16.0)),
                make_check("true_peak", "True peak", AssessmentStatus::Good, "−2.1 dBTP", "Safely within range.", None),
                make_check("clipping", "Clipping", AssessmentStatus::Good, "None detected", "No obvious clipping detected.", None),
                make_check("leading_silence", "Opening silence", AssessmentStatus::Good, "0.5 sec", "Looks good.", None),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 0);
        assert!(plan.actions.is_empty());
        assert!(plan.review_advisories.is_empty());
        assert!(!plan.changes_audio);
        assert_eq!(
            plan.summary,
            "No changes required. Your episode already meets the PodReady profile."
        );
    }

    #[test]
    fn test_loud_episode_one_action() {
        let assessment = make_assessment(
            vec![
                make_check(
                    "loudness",
                    "Integrated loudness",
                    AssessmentStatus::Attention,
                    "−14.2 LUFS",
                    "A little louder than we'd recommend for a stereo podcast.",
                    Some(-16.0),
                ),
                make_check("true_peak", "True peak", AssessmentStatus::Good, "−2.0 dBTP", "Safely within range.", None),
                make_check("clipping", "Clipping", AssessmentStatus::Good, "None detected", "No obvious clipping detected.", None),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 1);
        assert_eq!(plan.summary, "1 change recommended");
        assert!(plan.changes_audio);
        assert_eq!(plan.actions.len(), 1);

        let action = &plan.actions[0];
        assert_eq!(action.id, "adjust_loudness");
        assert_eq!(action.action_type, FixActionType::LoudnessAdjustment);
        assert_eq!(action.source_check_id, "loudness");
        assert_eq!(action.title, "Adjust loudness");
        assert_eq!(action.confidence, FixConfidence::High);
        assert!(action.changes_audio);
        assert_eq!(action.from_value, Some("−14.2 LUFS".to_string()));
        assert_eq!(action.to_value, Some("−16.0 LUFS target".to_string()));
        assert_eq!(action.reason, "A little louder than we'd recommend for a stereo podcast.");
    }

    #[test]
    fn test_quiet_episode_one_action_mono_target() {
        let assessment = make_assessment(
            vec![
                make_check(
                    "loudness",
                    "Integrated loudness",
                    AssessmentStatus::Issue,
                    "−24.5 LUFS",
                    "Significantly quieter than standard podcast delivery levels.",
                    Some(-19.0),
                ),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 1);
        let action = &plan.actions[0];
        assert_eq!(action.action_type, FixActionType::LoudnessAdjustment);
        assert_eq!(action.from_value, Some("−24.5 LUFS".to_string()));
        assert_eq!(action.to_value, Some("−19.0 LUFS target".to_string()));
        assert_eq!(action.confidence, FixConfidence::High);
    }

    #[test]
    fn test_high_true_peak_action() {
        let assessment = make_assessment(
            vec![
                make_check("loudness", "Integrated loudness", AssessmentStatus::Good, "−16.0 LUFS", "Safely within range.", Some(-16.0)),
                make_check(
                    "true_peak",
                    "True peak",
                    AssessmentStatus::Attention,
                    "−0.8 dBTP",
                    "Your peaks are slightly high for a publishing file.",
                    None,
                ),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 1);
        assert_eq!(plan.summary, "1 change recommended");
        assert!(plan.changes_audio);

        let action = &plan.actions[0];
        assert_eq!(action.id, "peak_protection");
        assert_eq!(action.action_type, FixActionType::PeakProtection);
        assert_eq!(action.title, "Apply peak protection");
        assert_eq!(action.confidence, FixConfidence::High);
        assert_eq!(action.from_value, Some("−0.8 dBTP".to_string()));
        assert_eq!(action.to_value, Some("≤ −1.5 dBTP ceiling".to_string()));
    }

    #[test]
    fn test_possible_clipping_creates_review_advisory_not_automatic_fix() {
        let assessment = make_assessment(
            vec![
                make_check("loudness", "Integrated loudness", AssessmentStatus::Good, "−16.0 LUFS", "Safely within range.", Some(-16.0)),
                make_check("true_peak", "True peak", AssessmentStatus::Good, "−2.0 dBTP", "Safely within range.", None),
                make_check(
                    "clipping",
                    "Clipping",
                    AssessmentStatus::Attention,
                    "Possible (12 flat samples)",
                    "Some waveform flattening was detected. Review recommended.",
                    None,
                ),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 0);
        assert!(plan.actions.is_empty());
        assert!(!plan.changes_audio);
        assert_eq!(plan.review_advisories.len(), 1);
        assert!(plan.review_advisories[0].contains("Automatic clipping repair is not supported"));
        assert_eq!(
            plan.summary,
            "No automatic changes available. Review recommended."
        );
    }

    #[test]
    fn test_mixed_issues_ordered_actions_and_advisories() {
        let assessment = make_assessment(
            vec![
                make_check(
                    "loudness",
                    "Integrated loudness",
                    AssessmentStatus::Attention,
                    "−14.2 LUFS",
                    "A little louder than we'd recommend.",
                    Some(-16.0),
                ),
                make_check(
                    "true_peak",
                    "True peak",
                    AssessmentStatus::Issue,
                    "+0.4 dBTP",
                    "Peak levels exceed recommended ceiling.",
                    None,
                ),
                make_check(
                    "clipping",
                    "Clipping",
                    AssessmentStatus::Attention,
                    "Possible",
                    "Some waveform flattening was detected.",
                    None,
                ),
                make_check(
                    "leading_silence",
                    "Opening silence",
                    AssessmentStatus::Attention,
                    "3.5 sec",
                    "Slightly long opening silence.",
                    None,
                ),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 2);
        assert_eq!(plan.summary, "2 changes recommended");
        assert!(plan.changes_audio);
        assert_eq!(plan.actions.len(), 2);
        assert_eq!(plan.actions[0].action_type, FixActionType::LoudnessAdjustment);
        assert_eq!(plan.actions[1].action_type, FixActionType::PeakProtection);
        assert_eq!(plan.review_advisories.len(), 1);
    }

    #[test]
    fn test_unknown_and_info_states_create_no_unsafe_actions() {
        let assessment = make_assessment(
            vec![
                make_check(
                    "loudness",
                    "Integrated loudness",
                    AssessmentStatus::Unknown,
                    "—",
                    "Loudness could not be measured.",
                    None,
                ),
                make_check(
                    "true_peak",
                    "True peak",
                    AssessmentStatus::Unknown,
                    "—",
                    "True peak could not be measured.",
                    None,
                ),
                make_check(
                    "clipping",
                    "Clipping",
                    AssessmentStatus::Info,
                    "Uncertain (lossy source)",
                    "Uncertain — cannot be determined confidently.",
                    None,
                ),
            ],
            vec![],
        );

        let plan = generate_fix_plan(&assessment);
        assert_eq!(plan.total_fixes, 0);
        assert!(plan.actions.is_empty());
        assert!(plan.review_advisories.is_empty());
        assert!(!plan.changes_audio);
        assert_eq!(
            plan.summary,
            "No changes required. Your episode already meets the PodReady profile."
        );
    }
}
