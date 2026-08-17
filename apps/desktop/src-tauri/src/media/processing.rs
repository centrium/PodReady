use crate::assessment::engine::{assess_media, Assessment};
use crate::assessment::profiles::get_profile_for_channels;
use crate::error::AppError;
use crate::fixplan::engine::{FixActionType, FixPlan};
use crate::media::analysis::{analyse_audio, AudioMeasurements};
use crate::media::binaries::ffmpeg_cmd;
use crate::media::ffprobe::inspect_media;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppliedAction {
    pub action_type: FixActionType,
    pub title: String,
    pub success: bool,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_value: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingResult {
    pub success: bool,
    pub output_path: String,
    pub actions_applied: Vec<AppliedAction>,
    pub review_advisories: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAudioResponse {
    pub result: ProcessingResult,
    pub candidate_path: String,
    pub candidate_filename: String,
    pub before_measurements: Option<AudioMeasurements>,
    pub before_assessment: Option<Assessment>,
    pub after_measurements: AudioMeasurements,
    pub after_assessment: Assessment,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LoudnormStats {
    pub input_i: String,
    pub input_tp: String,
    pub input_lra: String,
    pub input_thresh: String,
    pub target_offset: String,
}

/// Parses the JSON output produced by FFmpeg's `loudnorm=print_format=json` filter in stderr.
pub fn parse_loudnorm_pass1_json(stderr: &str) -> Result<LoudnormStats, AppError> {
    let start_idx = stderr
        .rfind('{')
        .ok_or_else(|| AppError::ProcessingFailed("No JSON block found in loudnorm output.".to_string()))?;
    let end_idx = stderr[start_idx..]
        .find('}')
        .ok_or_else(|| AppError::ProcessingFailed("Incomplete JSON block in loudnorm output.".to_string()))?
        + start_idx
        + 1;

    let json_str = &stderr[start_idx..end_idx];

    #[derive(Deserialize)]
    struct RawLoudnormStats {
        input_i: String,
        input_tp: String,
        input_lra: String,
        input_thresh: String,
        target_offset: String,
    }

    let parsed: RawLoudnormStats = serde_json::from_str(json_str).map_err(|e| {
        AppError::ProcessingFailed(format!("Failed to parse loudnorm analysis stats: {}", e))
    })?;

    Ok(LoudnormStats {
        input_i: parsed.input_i,
        input_tp: parsed.input_tp,
        input_lra: parsed.input_lra,
        input_thresh: parsed.input_thresh,
        target_offset: parsed.target_offset,
    })
}

/// Executes the approved FixPlan against a source media file.
/// Source files are strictly preserved and never modified.
/// The candidate audio is produced in a temporary workspace, then independently measured and assessed.
pub fn execute_fix_plan(
    source_path: &str,
    plan: &FixPlan,
    before_measurements: Option<AudioMeasurements>,
    before_assessment: Option<Assessment>,
) -> Result<ProcessAudioResponse, AppError> {
    let source_file = Path::new(source_path);
    if !source_file.exists() {
        return Err(AppError::ProcessingFailed(format!(
            "Source file does not exist: {}",
            source_path
        )));
    }

    // Validate actions: Reject any unsupported action
    for action in &plan.actions {
        match action.action_type {
            FixActionType::LoudnessAdjustment | FixActionType::PeakProtection => {}
            #[allow(unreachable_patterns)]
            _ => {
                return Err(AppError::UnsupportedAction(format!(
                    "Unsupported processing action: {:?}",
                    action.action_type
                )));
            }
        }
    }

    // Inspect source to determine channel count and target profile
    let initial_inspection = inspect_media(source_path)?;
    let profile = get_profile_for_channels(initial_inspection.inspection.channels);

    // If no actions modify audio, return early (no-op)
    let audio_actions: Vec<_> = plan.actions.iter().filter(|a| a.changes_audio).collect();
    if audio_actions.is_empty() {
        let current_meas = match before_measurements {
            Some(m) => m,
            None => analyse_audio(source_path, initial_inspection.inspection.duration_seconds)?,
        };
        let current_assessment = match before_assessment {
            Some(a) => a,
            None => assess_media(
                &initial_inspection.inspection,
                Some(&current_meas),
                &initial_inspection.format,
                &initial_inspection.codec,
            ),
        };

        let candidate_filename = source_file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("episode")
            .to_string();

        return Ok(ProcessAudioResponse {
            result: ProcessingResult {
                success: true,
                output_path: source_path.to_string(),
                actions_applied: vec![],
                review_advisories: plan.review_advisories.clone(),
                warnings: vec![],
                errors: vec![],
            },
            candidate_path: source_path.to_string(),
            candidate_filename,
            before_measurements: Some(current_meas.clone()),
            before_assessment: Some(current_assessment.clone()),
            after_measurements: current_meas,
            after_assessment: current_assessment,
        });
    }

    // Setup isolated temporary workspace
    let workspace_dir = std::env::temp_dir().join("podready_workspace");
    std::fs::create_dir_all(&workspace_dir).map_err(|e| {
        AppError::SystemError(format!("Failed to create temporary workspace directory: {}", e))
    })?;

    let file_stem = source_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("episode");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let candidate_filename = format!("{}_podready_candidate_{}.wav", file_stem, timestamp);
    let candidate_path: PathBuf = workspace_dir.join(&candidate_filename);
    let candidate_path_str = candidate_path.to_string_lossy().to_string();

    let target_i = profile.loudness.target_lufs;
    let target_tp = profile.true_peak.ceiling_dbtp;

    // Pass 1: Analyze audio with loudnorm filter to gather exact input statistics
    let pass1_filter = format!(
        "loudnorm=I={:.1}:TP={:.1}:LRA=11:print_format=json",
        target_i, target_tp
    );

    let pass1_output = ffmpeg_cmd()
        .args([
            "-hide_banner",
            "-nostats",
            "-i",
            source_path,
            "-af",
            &pass1_filter,
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| AppError::ProcessingFailed(format!("Failed to execute FFmpeg pass 1: {}", e)))?;

    if !pass1_output.status.success() {
        return Err(AppError::ProcessingFailed(
            "We couldn't analyze the audio during processing.".to_string(),
        ));
    }

    let pass1_stderr = String::from_utf8_lossy(&pass1_output.stderr);
    let stats = parse_loudnorm_pass1_json(&pass1_stderr)?;

    // Pass 2: Apply linear normalization & peak limiting using measured statistics
    let pass2_filter = format!(
        "loudnorm=I={:.1}:TP={:.1}:LRA=11:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        target_i,
        target_tp,
        stats.input_i,
        stats.input_tp,
        stats.input_lra,
        stats.input_thresh,
        stats.target_offset
    );

    let sample_rate_str = initial_inspection.inspection.sample_rate.to_string();

    let pass2_output = ffmpeg_cmd()
        .args([
            "-y",
            "-hide_banner",
            "-nostats",
            "-i",
            source_path,
            "-af",
            &pass2_filter,
            "-ar",
            &sample_rate_str,
            &candidate_path_str,
        ])
        .output()
        .map_err(|e| AppError::ProcessingFailed(format!("Failed to execute FFmpeg pass 2: {}", e)))?;

    if !pass2_output.status.success() {
        return Err(AppError::ProcessingFailed(
            "We couldn't render the processed candidate audio.".to_string(),
        ));
    }

    // Build list of applied actions directly from plan actions
    let mut actions_applied = Vec::new();
    for action in &plan.actions {
        actions_applied.push(AppliedAction {
            action_type: action.action_type,
            title: action.title.clone(),
            success: true,
            description: action.description.clone(),
            from_value: action.from_value.clone(),
            to_value: action.to_value.clone(),
        });
    }

    // Verification Step: Re-measure and re-assess the candidate output file independently
    let candidate_inspection = inspect_media(&candidate_path_str)?;
    let after_measurements = analyse_audio(
        &candidate_path_str,
        candidate_inspection.inspection.duration_seconds,
    )?;
    let after_assessment = assess_media(
        &candidate_inspection.inspection,
        Some(&after_measurements),
        &candidate_inspection.format,
        &candidate_inspection.codec,
    );

    let result = ProcessingResult {
        success: true,
        output_path: candidate_path_str.clone(),
        actions_applied,
        review_advisories: plan.review_advisories.clone(),
        warnings: vec![],
        errors: vec![],
    };

    Ok(ProcessAudioResponse {
        result,
        candidate_path: candidate_path_str,
        candidate_filename,
        before_measurements,
        before_assessment,
        after_measurements,
        after_assessment,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::engine::OverallStatus;
    use crate::assessment::profiles::PODCAST_STEREO_V1;
    use crate::fixplan::engine::{generate_fix_plan, FixAction, FixConfidence};
    use std::process::Command;

    #[test]
    fn test_parse_loudnorm_pass1_json() {
        let sample_stderr = r#"
[Parsed_loudnorm_0 @ 0x7fa281008000] 
{
	"input_i" : "-14.20",
	"input_tp" : "-1.00",
	"input_lra" : "8.50",
	"input_thresh" : "-24.50",
	"output_i" : "-16.00",
	"output_tp" : "-1.50",
	"output_lra" : "8.50",
	"output_thresh" : "-26.50",
	"normalization_type" : "dynamic",
	"target_offset" : "0.00"
}
[out#0/null @ 0x7fa280704780] video:0KiB audio:214KiB subtitle:0KiB
"#;
        let stats = parse_loudnorm_pass1_json(sample_stderr).expect("Should parse valid JSON");
        assert_eq!(stats.input_i, "-14.20");
        assert_eq!(stats.input_tp, "-1.00");
        assert_eq!(stats.input_lra, "8.50");
        assert_eq!(stats.input_thresh, "-24.50");
        assert_eq!(stats.target_offset, "0.00");
    }

    #[test]
    fn test_noop_when_no_audio_actions() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_noop.wav");

        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.0,volume=0.3,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let plan = FixPlan {
            summary: "No changes required".to_string(),
            actions: vec![],
            review_advisories: vec![],
            changes_audio: false,
            total_fixes: 0,
        };

        let res = execute_fix_plan(test_wav.to_str().unwrap(), &plan, None, None);
        assert!(res.is_ok());
        let response = res.unwrap();
        assert!(response.result.success);
        assert!(response.result.actions_applied.is_empty());
        assert_eq!(response.candidate_path, test_wav.to_str().unwrap());

        let _ = std::fs::remove_file(test_wav);
    }

    #[test]
    fn test_source_integrity_preserved() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_integrity.wav");

        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=1.5,volume=0.9,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let original_bytes = std::fs::read(&test_wav).expect("Should read original file");

        let plan = FixPlan {
            summary: "1 change recommended".to_string(),
            actions: vec![FixAction {
                id: "adjust_loudness".to_string(),
                action_type: FixActionType::LoudnessAdjustment,
                source_check_id: "loudness".to_string(),
                title: "Adjust loudness".to_string(),
                description: "Adjust loudness to -16.0 LUFS".to_string(),
                reason: "A little loud".to_string(),
                confidence: FixConfidence::High,
                changes_audio: true,
                from_value: Some("-10.0 LUFS".to_string()),
                to_value: Some("-16.0 LUFS".to_string()),
            }],
            review_advisories: vec![],
            changes_audio: true,
            total_fixes: 1,
        };

        let res = execute_fix_plan(test_wav.to_str().unwrap(), &plan, None, None)
            .expect("Processing should succeed");

        assert!(res.result.success);
        assert_ne!(res.candidate_path, test_wav.to_str().unwrap());

        // Verify original file byte contents remain identical
        let after_bytes = std::fs::read(&test_wav).expect("Should read original file after");
        assert_eq!(original_bytes, after_bytes);

        let _ = std::fs::remove_file(&test_wav);
        let _ = std::fs::remove_file(res.candidate_path);
    }

    #[test]
    fn test_e2e_loudness_adjustment_and_peak_protection() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_e2e_loud.wav");

        // Generate audio that is louder than target (-16 LUFS) with high true peak
        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=3.0,volume=0.9,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let inspected = inspect_media(test_wav.to_str().unwrap()).expect("Inspect should succeed");
        let meas_before = analyse_audio(test_wav.to_str().unwrap(), inspected.inspection.duration_seconds)
            .expect("Analyse before should succeed");

        let assessment_before = assess_media(
            &inspected.inspection,
            Some(&meas_before),
            &inspected.format,
            &inspected.codec,
        );

        // Plan should recommend fixes
        let plan = generate_fix_plan(&assessment_before);
        assert!(plan.changes_audio);
        assert!(plan.total_fixes >= 1);

        // Execute FixPlan
        let response = execute_fix_plan(
            test_wav.to_str().unwrap(),
            &plan,
            Some(meas_before.clone()),
            Some(assessment_before.clone()),
        )
        .expect("Execution should succeed");

        assert!(response.result.success);
        assert!(!response.result.actions_applied.is_empty());

        // Post-processing assessment should verify
        let after_lufs = response.after_measurements.integrated_loudness_lufs.unwrap();
        let after_tp = response.after_measurements.true_peak_dbtp.unwrap();

        // Output loudness should be moved safely toward -16.0 LUFS
        assert!(
            (after_lufs - PODCAST_STEREO_V1.loudness.target_lufs).abs() <= 1.5,
            "After loudness ({:.1}) should be within ±1.5 LUFS of target ({:.1})",
            after_lufs,
            PODCAST_STEREO_V1.loudness.target_lufs
        );

        // Output true peak should be below ceiling (-1.5 dBTP)
        assert!(
            after_tp <= PODCAST_STEREO_V1.true_peak.ceiling_dbtp + 0.1,
            "After true peak ({:.1}) should be <= ceiling ({:.1})",
            after_tp,
            PODCAST_STEREO_V1.true_peak.ceiling_dbtp
        );

        assert_eq!(response.after_assessment.overall_status, OverallStatus::Ready);

        let _ = std::fs::remove_file(&test_wav);
        let _ = std::fs::remove_file(response.candidate_path);
    }

    #[test]
    fn test_process_loudness_adjustment_mono() {
        use crate::assessment::profiles::PODCAST_MONO_V1;

        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_mono_quiet.wav");

        // Generate quiet mono audio (-25 LUFS)
        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=3.0,volume=0.08,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=mono",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let inspected = inspect_media(test_wav.to_str().unwrap()).expect("Inspect should succeed");
        assert_eq!(inspected.inspection.channels, 1);

        let meas_before = analyse_audio(test_wav.to_str().unwrap(), inspected.inspection.duration_seconds)
            .expect("Analyse before should succeed");

        let assessment_before = assess_media(
            &inspected.inspection,
            Some(&meas_before),
            &inspected.format,
            &inspected.codec,
        );

        assert_eq!(assessment_before.profile_id, "podcast-mono-v1");

        let plan = generate_fix_plan(&assessment_before);
        assert!(plan.changes_audio);

        let response = execute_fix_plan(
            test_wav.to_str().unwrap(),
            &plan,
            Some(meas_before),
            Some(assessment_before),
        )
        .expect("Execution should succeed");

        assert!(response.result.success);
        let after_lufs = response.after_measurements.integrated_loudness_lufs.unwrap();

        // Output loudness should be moved toward -19.0 LUFS target (mono profile)
        assert!(
            (after_lufs - PODCAST_MONO_V1.loudness.target_lufs).abs() <= 1.5,
            "After loudness ({:.1}) should be within ±1.5 LUFS of mono target ({:.1})",
            after_lufs,
            PODCAST_MONO_V1.loudness.target_lufs
        );

        let _ = std::fs::remove_file(&test_wav);
        let _ = std::fs::remove_file(response.candidate_path);
    }

    #[test]
    fn test_successful_processing_with_remaining_review_item() {
        let temp_dir = std::env::temp_dir();

        let test_wav = temp_dir.join("podready_test_clipping_advisory.wav");

        // Generate hard-clipped audio using aggressive volume to produce flat top clipping
        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=3.0,volume=10.0,asoftclip=type=hard,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let inspected = inspect_media(test_wav.to_str().unwrap()).expect("Inspect should succeed");
        let meas_before = analyse_audio(test_wav.to_str().unwrap(), inspected.inspection.duration_seconds)
            .expect("Analyse before should succeed");

        let assessment_before = assess_media(
            &inspected.inspection,
            Some(&meas_before),
            &inspected.format,
            &inspected.codec,
        );

        let plan = generate_fix_plan(&assessment_before);

        // FixPlan should produce audio fix actions for loudness/peak AND review advisories for clipping
        assert!(plan.changes_audio);
        assert!(!plan.review_advisories.is_empty(), "Should generate review advisory for clipping");

        let response = execute_fix_plan(
            test_wav.to_str().unwrap(),
            &plan,
            Some(meas_before),
            Some(assessment_before),
        )
        .expect("Execution should succeed");

        // Processing status must be SUCCESS
        assert!(response.result.success);
        assert!(!response.result.actions_applied.is_empty());

        // Review advisories must be preserved
        assert_eq!(response.result.review_advisories.len(), plan.review_advisories.len());
        assert!(response.result.review_advisories[0].contains("Automatic clipping repair is not supported"));

        let _ = std::fs::remove_file(&test_wav);
        let _ = std::fs::remove_file(response.candidate_path);
    }


    #[test]
    fn test_applied_action_reporting_fidelity() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_reporting_fidelity.wav");

        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=2.0,volume=0.8,aformat=sample_fmts=s16:sample_rates=44100:channel_layouts=stereo",
                test_wav.to_str().unwrap(),
            ])
            .output();

        let plan = FixPlan {
            summary: "2 changes recommended".to_string(),
            actions: vec![
                FixAction {
                    id: "adjust_loudness".to_string(),
                    action_type: FixActionType::LoudnessAdjustment,
                    source_check_id: "loudness".to_string(),
                    title: "Adjust loudness".to_string(),
                    description: "Adjust integrated loudness from −14.2 LUFS to −16.0 LUFS target.".to_string(),
                    reason: "A little loud".to_string(),
                    confidence: FixConfidence::High,
                    changes_audio: true,
                    from_value: Some("−14.2 LUFS".to_string()),
                    to_value: Some("−16.0 LUFS target".to_string()),
                },
                FixAction {
                    id: "peak_protection".to_string(),
                    action_type: FixActionType::PeakProtection,
                    source_check_id: "true_peak".to_string(),
                    title: "Apply peak protection".to_string(),
                    description: "Apply peak protection during final encoding to ensure safe headroom.".to_string(),
                    reason: "Peak levels exceed recommended ceiling".to_string(),
                    confidence: FixConfidence::High,
                    changes_audio: true,
                    from_value: Some("−0.8 dBTP".to_string()),
                    to_value: Some("≤ −1.5 dBTP ceiling".to_string()),
                },
            ],
            review_advisories: vec!["Sample advisory".to_string()],
            changes_audio: true,
            total_fixes: 2,
        };

        let response = execute_fix_plan(test_wav.to_str().unwrap(), &plan, None, None)
            .expect("Execution should succeed");

        assert_eq!(response.result.actions_applied.len(), 2);
        assert_eq!(response.result.actions_applied[0].title, "Adjust loudness");
        assert_eq!(response.result.actions_applied[0].from_value, Some("−14.2 LUFS".to_string()));
        assert_eq!(response.result.actions_applied[0].to_value, Some("−16.0 LUFS target".to_string()));
        assert_eq!(response.result.actions_applied[1].title, "Apply peak protection");
        assert_eq!(response.result.actions_applied[1].from_value, Some("−0.8 dBTP".to_string()));
        assert_eq!(response.result.actions_applied[1].to_value, Some("≤ −1.5 dBTP ceiling".to_string()));
        assert_eq!(response.result.review_advisories, vec!["Sample advisory".to_string()]);
        assert!(!response.candidate_filename.is_empty());

        let _ = std::fs::remove_file(&test_wav);
        let _ = std::fs::remove_file(response.candidate_path);
    }

    #[test]
    fn test_failed_processing_clean_error_message() {
        let plan = FixPlan {
            summary: "1 change recommended".to_string(),
            actions: vec![FixAction {
                id: "adjust_loudness".to_string(),
                action_type: FixActionType::LoudnessAdjustment,
                source_check_id: "loudness".to_string(),
                title: "Adjust loudness".to_string(),
                description: "Adjust loudness".to_string(),
                reason: "Loud".to_string(),
                confidence: FixConfidence::High,
                changes_audio: true,
                from_value: None,
                to_value: None,
            }],
            review_advisories: vec![],
            changes_audio: true,
            total_fixes: 1,
        };

        let err = execute_fix_plan("/nonexistent/path/file.wav", &plan, None, None)
            .expect_err("Should fail for nonexistent file");

        let err_msg = err.to_string();
        assert!(err_msg.contains("Source file does not exist") || err_msg.contains("Audio processing failed"));
        // Ensure no raw shell command details are in the user message
        assert!(!err_msg.contains("ffmpeg -hide_banner"));
    }
}



