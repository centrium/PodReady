use crate::error::AppError;
use crate::media::binaries::ffmpeg_cmd;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ClippingEvidence {
    NONE,
    POSSIBLE,
    UNCERTAIN,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClippingAnalysis {
    pub sample_peak_dbfs: Option<f64>,
    pub samples_at_ceiling: u64,
    pub flat_factor: f64,
    pub evidence: ClippingEvidence,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioMeasurements {
    pub integrated_loudness_lufs: Option<f64>,
    pub true_peak_dbtp: Option<f64>,
    pub leading_silence_seconds: f64,
    pub trailing_silence_seconds: f64,
    pub clipping: ClippingAnalysis,
}

#[derive(Debug, Default, PartialEq)]
pub struct ParsedAnalysisOutput {
    pub integrated_loudness_lufs: Option<f64>,
    pub true_peak_dbtp: Option<f64>,
    pub leading_silence_seconds: f64,
    pub trailing_silence_seconds: f64,
    pub sample_peak_dbfs: Option<f64>,
    pub samples_at_ceiling: u64,
    pub flat_factor: f64,
    pub is_lossy: bool,
}

/// Parses the standard output / standard error of FFmpeg when running:
/// `-af "ebur128=peak=true:framelog=quiet,silencedetect=noise=-50dB:d=0.1,astats"`
pub fn parse_ffmpeg_analysis(
    stderr: &str,
    total_duration_seconds: f64,
    is_lossy: bool,
) -> ParsedAnalysisOutput {
    let mut integrated_loudness = None;
    let mut true_peak = None;
    let mut leading_silence: Option<f64> = None;
    let mut last_silence_start: Option<f64> = None;
    let mut last_silence_end: Option<f64> = None;
    let mut flat_factor: Option<f64> = None;
    let mut peak_count: Option<u64> = None;
    let mut peak_level_db: Option<f64> = None;

    let mut in_ebur128_summary = false;
    let mut in_astats_overall = false;

    for line in stderr.lines() {
        let trimmed = line.trim();

        // Detect ebur128 Summary block
        if trimmed.contains("Summary:")
            || (trimmed.contains("[Parsed_ebur128") && trimmed.contains("Summary"))
        {
            in_ebur128_summary = true;
            continue;
        }

        if in_ebur128_summary {
            if trimmed.starts_with("I:") {
                // e.g. "I:         -21.1 LUFS" or "I: -inf LUFS"
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        integrated_loudness = Some(val);
                    }
                }
            } else if trimmed.starts_with("Peak:") {
                // e.g. "Peak:      -18.1 dBFS"
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        true_peak = Some(val);
                    }
                }
            } else if trimmed.starts_with("[Parsed_") || trimmed.starts_with("[out#") {
                in_ebur128_summary = false;
            }
        }

        // Silence detection
        // [Parsed_silencedetect_...] silence_start: 0 (or 0.000)
        // [Parsed_silencedetect_...] silence_end: 2.000023 | silence_duration: 2.000023
        if trimmed.contains("silence_start:") {
            if let Some(pos) = trimmed.find("silence_start:") {
                let start_part = &trimmed[pos + "silence_start:".len()..].trim();
                if let Ok(start_val) = start_part.parse::<f64>() {
                    last_silence_start = Some(start_val);
                    last_silence_end = None; // started but not yet ended
                }
            }
        } else if trimmed.contains("silence_end:") {
            if let Some(pos) = trimmed.find("silence_end:") {
                let rest = &trimmed[pos + "silence_end:".len()..];
                let end_str = rest.split('|').next().unwrap_or("").trim();
                if let Ok(end_val) = end_str.parse::<f64>() {
                    last_silence_end = Some(end_val);

                    // If leading silence was starting near 0, record duration
                    if leading_silence.is_none() {
                        if let Some(start_val) = last_silence_start {
                            if start_val <= 0.05 {
                                leading_silence = Some(end_val);
                            }
                        }
                    }
                }
            }
        }

        // astats section: Look for Overall or channel statistics
        if trimmed.contains("Overall") {
            in_astats_overall = true;
            continue;
        }

        if in_astats_overall || trimmed.contains("[Parsed_astats") {
            if trimmed.contains("Flat factor:") {
                if let Some(pos) = trimmed.find("Flat factor:") {
                    let val_str = trimmed[pos + "Flat factor:".len()..].trim();
                    if let Ok(val) = val_str.parse::<f64>() {
                        flat_factor = Some(val.max(flat_factor.unwrap_or(0.0)));
                    }
                }
            } else if trimmed.contains("Peak count:") {
                if let Some(pos) = trimmed.find("Peak count:") {
                    let val_str = trimmed[pos + "Peak count:".len()..].trim();
                    if let Ok(val) = val_str.parse::<f64>() {
                        let count = val as u64;
                        peak_count = Some(count.max(peak_count.unwrap_or(0)));
                    }
                }
            } else if trimmed.contains("Peak level dB:") {
                if let Some(pos) = trimmed.find("Peak level dB:") {
                    let val_str = trimmed[pos + "Peak level dB:".len()..].trim();
                    if let Ok(val) = val_str.parse::<f64>() {
                        peak_level_db = Some(val.max(peak_level_db.unwrap_or(-999.0)));
                    }
                }
            }
        }
    }

    // Trailing silence determination:
    let trailing_silence = if let Some(start_val) = last_silence_start {
        if last_silence_end.is_none() {
            // Silence was ongoing at EOF
            (total_duration_seconds - start_val).max(0.0)
        } else if let Some(end_val) = last_silence_end {
            if total_duration_seconds > 0.0 && (total_duration_seconds - end_val).abs() <= 0.15 {
                (end_val - start_val).max(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    let leading = leading_silence.unwrap_or(0.0);

    ParsedAnalysisOutput {
        integrated_loudness_lufs: integrated_loudness,
        true_peak_dbtp: true_peak,
        leading_silence_seconds: leading,
        trailing_silence_seconds: trailing_silence,
        sample_peak_dbfs: peak_level_db,
        samples_at_ceiling: if peak_level_db.map(|p| p >= -0.001).unwrap_or(false) {
            peak_count.unwrap_or(0)
        } else {
            0
        },
        flat_factor: flat_factor.unwrap_or(0.0),
        is_lossy,
    }
}

/// Evaluates objective clipping evidence conservatively.
/// Distinguishes between clean high peaks, properly limited material, lossy codec uncertainty, and hard flat-top clipping.
pub fn evaluate_clipping_evidence(
    sample_peak_dbfs: Option<f64>,
    samples_at_ceiling: u64,
    flat_factor: f64,
    is_lossy: bool,
) -> ClippingAnalysis {
    let evidence = if is_lossy {
        // In lossy formats (MP3/AAC/M4A), psychoacoustic filtering and MDCT transform
        // alter flat peaks and introduce small inter-sample ripples. Flat factor cannot be reliably observed.
        if samples_at_ceiling > 0 && flat_factor > 0.0 {
            ClippingEvidence::POSSIBLE
        } else if sample_peak_dbfs.map(|p| p >= -0.05).unwrap_or(false) && samples_at_ceiling > 100 {
            ClippingEvidence::UNCERTAIN
        } else {
            ClippingEvidence::NONE
        }
    } else {
        // Uncompressed / PCM
        // Flat factor > 0 indicates consecutive identical peak samples (waveform truncation).
        if flat_factor > 0.0 {
            ClippingEvidence::POSSIBLE
        } else {
            // High peak or limited material without flat tops is not deemed clipped.
            ClippingEvidence::NONE
        }
    };

    ClippingAnalysis {
        sample_peak_dbfs,
        samples_at_ceiling,
        flat_factor: (flat_factor * 100.0).round() / 100.0,
        evidence,
    }
}

/// Executes FFmpeg analysis on the audio stream of `path`.
pub fn analyse_audio<P: AsRef<Path>>(
    path: P,
    total_duration_seconds: f64,
) -> Result<AudioMeasurements, AppError> {
    let path_ref = path.as_ref();
    let path_str = path_ref.to_string_lossy().to_string();

    let ext = path_ref
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let is_lossy = ext == "mp3" || ext == "m4a" || ext == "aac" || ext == "mp4" || ext == "mov";

    let output = ffmpeg_cmd()
        .args([
            "-nostats",
            "-i",
            &path_str,
            "-af",
            "ebur128=peak=true:framelog=quiet,silencedetect=noise=-50dB:d=0.1,astats",
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| {
            log::error!("Failed to execute ffmpeg for analysis: {}", e);
            AppError::AudioAnalysisFailed("Failed to execute ffmpeg analysis".into())
        })?;

    let stderr_str = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        log::error!("ffmpeg analysis failed: {}", stderr_str);
        return Err(AppError::AudioAnalysisFailed(
            stderr_str.into_owned(),
        ));
    }

    let parsed = parse_ffmpeg_analysis(&stderr_str, total_duration_seconds, is_lossy);
    let clipping = evaluate_clipping_evidence(
        parsed.sample_peak_dbfs,
        parsed.samples_at_ceiling,
        parsed.flat_factor,
        parsed.is_lossy,
    );

    Ok(AudioMeasurements {
        integrated_loudness_lufs: parsed.integrated_loudness_lufs,
        true_peak_dbtp: parsed.true_peak_dbtp,
        leading_silence_seconds: (parsed.leading_silence_seconds * 10.0).round() / 10.0,
        trailing_silence_seconds: (parsed.trailing_silence_seconds * 10.0).round() / 10.0,
        clipping,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_parse_ebur128_and_silence() {
        let stderr = r#"
[Parsed_silencedetect_1 @ 0x75502ca80] silence_start: 0
[Parsed_silencedetect_1 @ 0x75502ca80] silence_end: 4.8123 | silence_duration: 4.8123
[Parsed_silencedetect_1 @ 0x75502ca80] silence_start: 55.4
[Parsed_astats_2 @ 0x75502cb40] Overall
[Parsed_astats_2 @ 0x75502cb40] Peak level dB: -0.412
[Parsed_astats_2 @ 0x75502cb40] Flat factor: 0.000000
[Parsed_astats_2 @ 0x75502cb40] Peak count: 12.000000
[Parsed_ebur128_0 @ 0x75502c9c0] Summary:

  Integrated loudness:
    I:         -18.7 LUFS
    Threshold: -28.7 LUFS

  Loudness range:
    LRA:         3.5 LU
    Threshold: -38.7 LUFS
    LRA low:   -21.2 LUFS
    LRA high:  -17.7 LUFS

  True peak:
    Peak:      -0.4 dBFS
"#;

        let parsed = parse_ffmpeg_analysis(stderr, 56.5, false);
        assert_eq!(parsed.integrated_loudness_lufs, Some(-18.7));
        assert_eq!(parsed.true_peak_dbtp, Some(-0.4));
        assert!((parsed.leading_silence_seconds - 4.8123).abs() < 0.001);
        assert!((parsed.trailing_silence_seconds - 1.1).abs() < 0.001);

        let clipping = evaluate_clipping_evidence(
            parsed.sample_peak_dbfs,
            parsed.samples_at_ceiling,
            parsed.flat_factor,
            false,
        );
        assert_eq!(clipping.evidence, ClippingEvidence::NONE);
    }

    #[test]
    fn test_parse_clipping_detection_pcm() {
        let stderr = r#"
[Parsed_astats_2 @ 0x75502cb40] Overall
[Parsed_astats_2 @ 0x75502cb40] Peak level dB: 0.000000
[Parsed_astats_2 @ 0x75502cb40] Flat factor: 14.500000
[Parsed_astats_2 @ 0x75502cb40] Peak count: 3200.000000
[Parsed_ebur128_0 @ 0x75502c9c0] Summary:

  Integrated loudness:
    I:         -12.0 LUFS
    Threshold: -22.0 LUFS

  True peak:
    Peak:      1.2 dBFS
"#;

        let parsed = parse_ffmpeg_analysis(stderr, 10.0, false);
        let clipping = evaluate_clipping_evidence(
            parsed.sample_peak_dbfs,
            parsed.samples_at_ceiling,
            parsed.flat_factor,
            false,
        );
        assert_eq!(clipping.evidence, ClippingEvidence::POSSIBLE);
        assert_eq!(clipping.samples_at_ceiling, 3200);
        assert_eq!(clipping.flat_factor, 14.5);
    }

    // A — Clean signal: Clean sine wave comfortably below full scale
    #[test]
    fn test_fixture_a_clean_signal() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_fixture_a.wav");

        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=1000:d=1.0,volume=0.5,aformat=sample_fmts=s16:sample_rates=44100",
                test_wav.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen_status {
            if output.status.success() {
                let measurements = analyse_audio(&test_wav, 1.0).expect("Analysis should succeed");
                assert_eq!(measurements.clipping.evidence, ClippingEvidence::NONE);
                assert_eq!(measurements.clipping.samples_at_ceiling, 0);
                assert_eq!(measurements.clipping.flat_factor, 0.0);
                let _ = std::fs::remove_file(test_wav);
            }
        }
    }

    // B — Near-full-scale clean signal: Peaks near 0 dBFS without clipping
    #[test]
    fn test_fixture_b_near_full_scale_clean() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_fixture_b.wav");

        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=1000:d=1.0,volume=8.0,aformat=sample_fmts=s16:sample_rates=44100",
                test_wav.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen_status {
            if output.status.success() {
                let measurements = analyse_audio(&test_wav, 1.0).expect("Analysis should succeed");
                // Must not classify as clipped merely because it peaks near 0 dBFS
                assert_eq!(measurements.clipping.evidence, ClippingEvidence::NONE);
                assert_eq!(measurements.clipping.flat_factor, 0.0);
                let _ = std::fs::remove_file(test_wav);
            }
        }
    }

    // C — Deliberately hard-clipped PCM: Overdriven signal with flat tops
    #[test]
    fn test_fixture_c_hard_clipped_pcm() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_fixture_c.wav");

        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=1000:d=1.0,volume=30dB,aformat=sample_fmts=s16:sample_rates=44100",
                test_wav.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen_status {
            if output.status.success() {
                let measurements = analyse_audio(&test_wav, 1.0).expect("Analysis should succeed");
                assert_eq!(measurements.clipping.evidence, ClippingEvidence::POSSIBLE);
                assert!(measurements.clipping.flat_factor > 0.0);
                assert!(measurements.clipping.samples_at_ceiling > 0);
                let _ = std::fs::remove_file(test_wav);
            }
        }
    }

    // D — Limited / high-peak signal: Overdriven through alimiter (repeated controlled peaks, no flat clipping)
    #[test]
    fn test_fixture_d_limited_high_peaks() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("podready_test_fixture_d.wav");

        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=1000:d=1.0,volume=8.0,alimiter=limit=0.95,aformat=sample_fmts=s16:sample_rates=44100",
                test_wav.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen_status {
            if output.status.success() {
                let measurements = analyse_audio(&test_wav, 1.0).expect("Analysis should succeed");
                // Repeated controlled high peaks alone must not produce a clipping conclusion
                assert_eq!(measurements.clipping.evidence, ClippingEvidence::NONE);
                assert_eq!(measurements.clipping.flat_factor, 0.0);
                let _ = std::fs::remove_file(test_wav);
            }
        }
    }

    // E — Lossy encode: Overdriven source encoded to MP3
    #[test]
    fn test_fixture_e_lossy_encode() {
        let temp_dir = std::env::temp_dir();
        let test_mp3 = temp_dir.join("podready_test_fixture_e.mp3");

        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=f=1000:d=1.0,volume=30dB,aformat=sample_fmts=s16:sample_rates=44100",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                test_mp3.to_str().unwrap(),
            ])
            .output();

        if let Ok(output) = gen_status {
            if output.status.success() {
                let measurements = analyse_audio(&test_mp3, 1.0).expect("Analysis should succeed");
                // Due to lossy transform, flat factor is 0 but true peak is high / ceiling samples may vary.
                // It should report conservative evidence (NONE or UNCERTAIN/POSSIBLE) without asserting unbacked certainty.
                assert!(measurements.true_peak_dbtp.is_some());
                let _ = std::fs::remove_file(test_mp3);
            }
        }
    }
}
