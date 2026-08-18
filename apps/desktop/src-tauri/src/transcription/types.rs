use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResult {
    pub text: String,
    pub language: Option<String>,
    pub duration_seconds: Option<f64>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionBenchmark {
    pub audio_duration_seconds: f64,
    pub prep_seconds: f64,
    pub runtime_startup_seconds: f64,
    pub model_init_seconds: f64,
    pub inference_seconds: f64,
    pub output_processing_seconds: f64,
    pub total_seconds: f64,
    pub real_time_factor: f64,
    pub model_name: String,
    pub model_size_bytes: u64,
    pub detected_language: Option<String>,
}

impl TranscriptionBenchmark {
    #[allow(dead_code)]
    pub fn formatted_report(&self) -> String {
        format!(
            "TRANSCRIPTION BENCHMARK\n\
             =======================\n\
             Model:                {}\n\
             Audio duration:       {:.1} sec ({:.2} min)\n\n\
             Audio preparation:    {:.2} sec\n\
             Runtime startup:      {:.2} sec\n\
             Model initialisation: {:.2} sec\n\
             Inference:            {:.2} sec\n\
             Output processing:    {:.2} sec\n\n\
             Total:                {:.2} sec\n\
             Real-time factor:     {:.3}x",
            self.model_name,
            self.audio_duration_seconds,
            self.audio_duration_seconds / 60.0,
            self.prep_seconds,
            self.runtime_startup_seconds,
            self.model_init_seconds,
            self.inference_seconds,
            self.output_processing_seconds,
            self.total_seconds,
            self.real_time_factor
        )
    }
}
