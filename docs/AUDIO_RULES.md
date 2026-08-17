# Audio Assessment & Measurement Rules

## 1. Scope & Architecture

PodReady strictly separates **Measurement** from **Assessment**:
```text
Media inspection (ffprobe)
      ↓
Audio measurements (ffmpeg)
      ↓
Assessment (rules engine - Stage 3)
      ↓
Fix planning (Stage 4)
      ↓
Processing (Stage 4)
      ↓
Verification (Stage 4)
```

At this stage (Stage 2: Audio Analysis Engine), PodReady measures objective acoustic facts. It does not judge quality or apply pass/fail thresholds.

---

## 2. Measurement Methodology

PodReady uses FFmpeg audio filters (`ebur128`, `silencedetect`, and `astats`) executed in a single pass to obtain objective measurements.

### 2.1 Integrated Loudness (LUFS)
- **Standard**: ITU-R BS.1770-4 / EBU R128.
- **Filter**: `ebur128=peak=true:framelog=quiet`
- **Metric**: Integrated Loudness (`I`) in LUFS.
- **Details**: Measures programme loudness across the entire audio file using K-weighting filters and gating (absolute threshold at -70 LUFS, relative threshold at -10 LU below ungated loudness).

### 2.2 True Peak (dBTP)
- **Standard**: ITU-R BS.1770-4 / EBU R128 True Peak meter.
- **Filter**: `ebur128=peak=true:framelog=quiet`
- **Metric**: Peak in dBTP (reported as dBFS in EBU R128 meter summary).
- **Details**: Uses 4x oversampling interpolation to catch inter-sample peaks that would exceed the 0 dBFS boundary during digital-to-analogue conversion or lossy encoding.

### 2.3 Boundary Silence (Opening & Trailing Silence)
- **Filter**: `silencedetect=noise=-50dB:d=0.1`
- **Metrics**:
  - `leadingSilenceSeconds`: Duration of silence detected starting at `t = 0.0s` before the first audio event.
  - `trailingSilenceSeconds`: Duration of silence extending until the end of the programme.
- **Threshold**: Noise floor of -50 dBFS with a minimum duration of 0.1s.
- **Scope**: Only silence at the boundaries of the file is measured; interior pauses during conversational speech are not treated as boundary silence.

### 2.4 Digital Clipping & Peak Evidence
- **Filter**: `astats`
- **Objective Metrics**:
  - `samplePeakDbfs`: Maximum absolute sample level in dBFS.
  - `samplesAtCeiling`: Number of samples reaching within 0.001 dB of full scale (0.0 dBFS).
  - `flatFactor`: Measurement of consecutive identical peak samples (flat-topping).
- **Distinction from True Peak**:
  - `truePeakDbtp` (from `ebur128`) measures reconstructed inter-sample analog peaks.
  - `clipping` (from `astats`) evaluates whether the source waveform exhibits flat-top truncation.
  - Having a high true peak (e.g. > 0 dBTP) does not by itself prove waveform clipping.
- **Evidence Interpretation**:
  - **Uncompressed PCM**:
    - `evidence = POSSIBLE` only when `flatFactor > 0.0` (indicating flat-topped waveform truncation).
    - Loud, limited, or normalized material with high sample peaks but `flatFactor == 0.0` is classified as `NONE` (no clipping evidence).
  - **Lossy Audio (MP3 / AAC / M4A)**:
    - Transform encoding (MDCT) and psychoacoustic filtering alter waveforms, often destroying flat tops and introducing ripples.
    - If `samplesAtCeiling > 100` with near 0 dBFS peaks but no distinct flat factor, evidence is classified as `UNCERTAIN`.
    - Otherwise classified conservatively as `NONE`.
- **User-Facing Presentation**:
  - `No obvious clipping detected`
  - `Possible clipping detected`
  - `Uncertain (lossy source)`

---

## 3. Known Limitations & Technical Nuances

1. **Mastered / Limited Material**: Audio processed through a transparent brickwall limiter will have repeated peaks at 0 dBFS or -0.1 dBFS without flat clipping distortion. PodReady relies on the flat-factor metric rather than raw peak count to avoid falsely flagging mastered audio.
2. **Lossy Codec Artifacts**: MP3 and AAC compression introduces overshoot and alters sample peaks. True peak and sample peak values on lossy files reflect post-decoder waveforms rather than raw pre-encode PCM.
3. **Short Files**: EBU R128 integrated loudness requires sufficient sample duration for meaningful measurement; very short files (< 0.5s) may report `-inf LUFS`.
4. **Pure Silence**: Completely silent tracks report `-inf LUFS` and `-inf dBTP`, which PodReady safely maps to `None` / `null` rather than magic sentinel numbers.
5. **Mono vs Stereo**: Channels are handled natively according to BS.1770-4 weighting.

---

*(Assessment rules, target profile ranges, and scoring heuristics will be introduced in Stage 3).*

