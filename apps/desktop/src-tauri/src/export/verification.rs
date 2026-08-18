use crate::assessment::engine::{assess_media, OverallStatus};
use crate::error::AppError;
use crate::export::types::ExportVerificationResult;
use crate::media::analysis::analyse_audio;
use crate::media::ffprobe::inspect_media;

/// Independently inspects, measures, and assesses the exported MP3 file
/// to guarantee PodReady compliance on the final deliverable.
pub fn verify_exported_mp3(exported_mp3_path: &str) -> Result<ExportVerificationResult, AppError> {
    let inspection_res = inspect_media(exported_mp3_path)?;
    let measurements = analyse_audio(
        exported_mp3_path,
        inspection_res.inspection.duration_seconds,
    )?;

    let assessment = assess_media(
        &inspection_res.inspection,
        Some(&measurements),
        &inspection_res.format,
        &inspection_res.codec,
    );

    let passed = assessment.overall_status == OverallStatus::Ready
        || assessment.overall_status == OverallStatus::Attention;

    Ok(ExportVerificationResult {
        passed,
        overall_status: assessment.overall_status,
        summary: assessment.summary.clone(),
        measurements,
        assessment,
    })
}
