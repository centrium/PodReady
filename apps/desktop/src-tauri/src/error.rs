use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("We couldn't read the audio in this file.")]
    MediaInspectionFailed(String),

    #[error("We couldn't analyse the audio in this file.")]
    AudioAnalysisFailed(String),

    #[error("The media format is not supported.")]
    UnsupportedFormat,

    #[error("An unexpected system error occurred.")]
    SystemError(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
            code: String,
        }

        let response = match self {
            AppError::MediaInspectionFailed(_) => ErrorResponse {
                message: self.to_string(),
                code: "MEDIA_INSPECTION_FAILED".to_string(),
            },
            AppError::AudioAnalysisFailed(_) => ErrorResponse {
                message: self.to_string(),
                code: "AUDIO_ANALYSIS_FAILED".to_string(),
            },
            AppError::UnsupportedFormat => ErrorResponse {
                message: self.to_string(),
                code: "UNSUPPORTED_FORMAT".to_string(),
            },
            AppError::SystemError(_) => ErrorResponse {
                message: self.to_string(),
                code: "SYSTEM_ERROR".to_string(),
            },
        };

        response.serialize(serializer)
    }
}
