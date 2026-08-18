use crate::error::AppError;
use crate::export::types::{ExportedFile, PublishingJsonReport};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generates a machine-readable JSON verification report.
pub fn generate_json_report(
    output_report_path: &str,
    report_data: &PublishingJsonReport,
) -> Result<ExportedFile, AppError> {
    let out_path = Path::new(output_report_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::SystemError(format!("Failed to create directory for report: {}", e))
        })?;
    }

    let json_bytes = serde_json::to_vec_pretty(report_data).map_err(|e| {
        AppError::SystemError(format!("Failed to serialize publishing report JSON: {}", e))
    })?;

    let mut file = File::create(out_path).map_err(|e| {
        AppError::SystemError(format!("Failed to create report file: {}", e))
    })?;

    file.write_all(&json_bytes).map_err(|e| {
        AppError::SystemError(format!("Failed to write report file content: {}", e))
    })?;

    let file_size = file
        .metadata()
        .map(|m| m.len())
        .unwrap_or(json_bytes.len() as u64);

    let filename = out_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("report.json")
        .to_string();

    Ok(ExportedFile {
        path: output_report_path.to_string(),
        filename,
        file_size_bytes: file_size,
        file_type: "report".to_string(),
    })
}
