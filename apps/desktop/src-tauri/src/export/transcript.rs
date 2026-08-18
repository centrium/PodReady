use crate::error::AppError;
use crate::export::types::ExportedFile;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Writes the companion transcript text to a plain TXT file.
pub fn write_transcript_file(
    transcript_text: &str,
    output_txt_path: &str,
) -> Result<ExportedFile, AppError> {
    let out_path = Path::new(output_txt_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::SystemError(format!("Failed to create directory for transcript: {}", e))
        })?;
    }

    let mut file = File::create(out_path).map_err(|e| {
        AppError::SystemError(format!("Failed to create transcript file: {}", e))
    })?;

    file.write_all(transcript_text.as_bytes()).map_err(|e| {
        AppError::SystemError(format!("Failed to write transcript content: {}", e))
    })?;

    let file_size = file
        .metadata()
        .map(|m| m.len())
        .unwrap_or(transcript_text.len() as u64);

    let filename = out_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("transcript.txt")
        .to_string();

    Ok(ExportedFile {
        path: output_txt_path.to_string(),
        filename,
        file_size_bytes: file_size,
        file_type: "transcript".to_string(),
    })
}
