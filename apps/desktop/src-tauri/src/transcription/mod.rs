pub mod engine;
pub mod types;
pub mod whisper;

#[allow(unused_imports)]
pub use engine::{transcribe_audio, transcribe_audio_with_benchmark};
#[allow(unused_imports)]
pub use types::{TranscriptResult, TranscriptSegment, TranscriptionBenchmark};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::binaries::{get_resources_dir, resolve_default_model, resolve_test_model};

    #[test]
    fn test_transcribe_speech_fixture() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let fixture_path = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        if !fixture_path.exists() {
            eprintln!("Fixture not present at {:?}, skipping test", fixture_path);
            return;
        }

        let model_path = resolve_test_model().expect("test model should resolve");
        let result = transcribe_audio(
            fixture_path.to_str().unwrap(),
            Some(&model_path),
        )
        .expect("Transcription should succeed on valid speech audio fixture");

        assert!(!result.text.is_empty(), "Transcribed text should not be empty");
        let lower = result.text.to_lowercase();
        assert!(
            lower.contains("fellow americans") || lower.contains("country"),
            "Expected speech phrases in transcript, got: {}",
            result.text
        );
    }

    #[test]
    fn test_transcription_benchmark_instrumentation() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let fixture_path = res_dir.join("fixtures").join("spoken_jfk_16k.wav");

        if !fixture_path.exists() {
            return;
        }

        let model_path = resolve_default_model().expect("default model should resolve");
        let (result, benchmark) = transcribe_audio_with_benchmark(
            fixture_path.to_str().unwrap(),
            Some(&model_path),
        )
        .expect("Transcription benchmark should succeed");

        assert!(!result.text.is_empty());
        assert!(benchmark.audio_duration_seconds > 0.0);
        assert!(benchmark.total_seconds > 0.0);
        assert!(benchmark.real_time_factor > 0.0);

        println!("\n{}", benchmark.formatted_report());
    }

    #[test]
    fn test_benchmark_model_matrix() {
        let _guard = crate::TEST_GLOBAL_ENV_LOCK.lock().unwrap();

        let res_dir = get_resources_dir().expect("resources dir exists");
        let mcd_wav = std::path::Path::new("/Users/matt/Desktop/McDonalds_LNG_061019.wav");
        if !mcd_wav.exists() {
            return;
        }

        let models = vec![
            ("small", res_dir.join("models").join("ggml-small.bin")),
            ("large-v3-turbo", res_dir.join("models").join("ggml-large-v3-turbo.bin")),
            ("medium", res_dir.join("models").join("ggml-medium.bin")),
            ("base", res_dir.join("models").join("ggml-base.bin")),
        ];

        println!("\n=================== MODEL MATRIX BENCHMARK (85.5s AUDIO) ===================");
        println!("{:<16} | {:<8} | {:<8} | {:<8} | {:<8} | {:<8} | {:<8}", "Model", "Audio", "Prep", "Inference", "Total", "RTF", "Size (MB)");
        println!("{:-<16}-|-{:-<8}-|-{:-<8}-|-{:-<8}-|-{:-<8}-|-{:-<8}-|-{:-<8}", "", "", "", "", "", "", "");

        for (name, path) in models {
            if path.exists() {
                if let Ok((_res, bench)) = transcribe_audio_with_benchmark(mcd_wav.to_str().unwrap(), Some(&path)) {
                    println!(
                        "{:<16} | {:<6.1}s | {:<6.2}s | {:<6.2}s   | {:<6.2}s | {:<6.3}x | {:<6.1} MB",
                        name,
                        bench.audio_duration_seconds,
                        bench.prep_seconds,
                        bench.inference_seconds,
                        bench.total_seconds,
                        bench.real_time_factor,
                        bench.model_size_bytes as f64 / 1024.0 / 1024.0
                    );
                }
            }
        }
        println!("============================================================================\n");
    }
}
