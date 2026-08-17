# Audio Assessment & Measurement Rules

## 1. Scope & Architecture

PodReady strictly separates **Measurement** from **Assessment**:

```text
Media inspection (ffprobe)
      ↓
Audio measurements (ffmpeg)
      ↓
PodcastProfile (Stereo / Mono V1)
      ↓
Assessment (rules engine - Stage 3)
      ↓
Fix planning (Stage 4)
      ↓
Processing (Stage 4)
      ↓
Verification (Stage 4)
```

- **Measurement (Stage 2 & 2A)**: Captures objective acoustic and container facts using FFmpeg and ffprobe without making judgments.
- **Assessment (Stage 3)**: Interprets objective measurements against an explicit, versioned `PodcastProfile` to provide clear, actionable podcast-readiness judgments.
- **Non-destructive**: Source audio files are never modified.

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

---

## 3. PodReady Assessment Profiles (V1)

PodReady uses explicit, versioned profiles to evaluate audio against podcast publishing industry standards.

### 3.1 Profile Identifiers
- `podcast-stereo-v1` (Version `1.0.0`): Default profile for stereo audio (`channels = 2` or multi-channel).
- `podcast-mono-v1` (Version `1.0.0`): Target profile for single-channel mono audio (`channels = 1`).

---

## 4. Assessment Rules & Thresholds

### 4.1 Integrated Loudness

Loudness is evaluated as a target range rather than requiring a strict exact number.

| Profile | Target | Good Range (`GOOD`) | Attention Range (`ATTENTION`) | Significant Issue (`ISSUE`) | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Podcast — Stereo** | **−16.0 LUFS** | `[−17.5, −14.5]` LUFS (±1.5 LU) | `(−20.0, −17.5)` *(Quiet)*<br>`(−14.5, −13.0]` *(Loud)* | `< −20.0` *(Very Quiet)*<br>`> −13.0` *(Very Loud)* | Standard podcast industry delivery target (Apple Podcasts, Spotify, AES TD1004). ±1.5 LU accommodates natural spoken dynamics. |
| **Podcast — Mono** | **−19.0 LUFS** | `[−20.5, −17.5]` LUFS (±1.5 LU) | `(−23.0, −20.5)` *(Quiet)*<br>`(−17.5, −16.0]` *(Loud)* | `< −23.0` *(Very Quiet)*<br>`> −16.0` *(Very Loud)* | AES / Apple Podcasts recommended mono delivery target. Corresponds acoustically to −16 LUFS stereo playback in acoustic space. |

#### Copy & Messages:
- `GOOD`: `"Safely within recommended range for a {stereo|mono} podcast."`
- `ATTENTION (Quiet)`: `"A little quieter than we'd recommend for a {stereo|mono} podcast."`
- `ATTENTION (Loud)`: `"A little louder than we'd recommend for a {stereo|mono} podcast."`
- `ISSUE (Quiet)`: `"Significantly quieter than standard podcast delivery levels."`
- `ISSUE (Loud)`: `"Significantly louder than standard podcast delivery levels."`
- `UNKNOWN`: `"Loudness could not be measured."`

---

### 4.2 True Peak

True peak checks the reconstructed inter-sample ceiling to avoid digital-to-analogue conversion clipping and MP3 transcoding distortion.

| Metric | Good Range (`GOOD`) | Attention Range (`ATTENTION`) | Significant Issue (`ISSUE`) | Rationale |
| :--- | :--- | :--- | :--- | :--- |
| **True Peak** | `≤ −1.5 dBTP` | `(−1.5, −0.5] dBTP` | `> −0.5 dBTP` | A ceiling of −1.5 dBTP provides safe headroom for downstream lossy encoding (e.g. 128–192 kbps MP3/AAC) without inter-sample distortion. Peaks between −1.5 and −0.5 dBTP leave narrow headroom. Peaks > −0.5 dBTP risk clipping during streaming platform transcoding. |

#### Copy & Messages:
- `GOOD`: `"Safely within range."`
- `ATTENTION`: `"Your peaks are slightly high for a publishing file."`
- `ISSUE`: `"Peak levels exceed recommended ceiling; risk of distortion on streaming platforms."`
- `UNKNOWN`: `"True peak could not be measured."`

---

### 4.3 Boundary Silence (Forgiving)

Boundary silence evaluates leading (opening) and trailing (closing) silence independently. Rules are deliberately forgiving to allow normal room tone and gentle intro/outro music fades.

| Boundary | Good Range (`GOOD`) | Attention Range (`ATTENTION`) | Significant Issue (`ISSUE`) | Rationale |
| :--- | :--- | :--- | :--- | :--- |
| **Opening Silence** | `0.0` to `2.0` sec | `2.1` to `5.0` sec | `> 5.0` sec | Podcasters routinely start with 0.2s to 1.5s of atmosphere or breathing before speaking. > 2.0s is noticeably delayed; > 5.0s is excessive dead air. |
| **Closing Silence** | `0.0` to `4.0` sec | `4.1` to `8.0` sec | `> 8.0` sec | Outros frequently include gentle fades of 2–4 seconds. > 4.0s is slightly prolonged; > 8.0s is excessive dead air at the end of the episode. |

#### Copy & Messages:
- `GOOD`: `"Looks good."`
- `ATTENTION (Opening)`: `"Slightly long opening silence."`
- `ISSUE (Opening)`: `"Excessive opening silence before audio begins."`
- `ATTENTION (Closing)`: `"Slightly long closing silence."`
- `ISSUE (Closing)`: `"Excessive trailing silence at the end of the episode."`

---

### 4.4 Clipping Evidence Mapping

| Evidence | Assessment Status | Display Value | Message | Rationale |
| :--- | :--- | :--- | :--- | :--- |
| `NONE` | `GOOD` | `"None detected"` | `"No obvious clipping detected."` | Waveform shows no consecutive identical flat-topped peak samples. |
| `POSSIBLE` | `ATTENTION` | `"Possible"` or `"Possible ({N} flat samples)"` | `"Some waveform flattening was detected. Review recommended."` | Consecutive identical peak samples detected in uncompressed PCM audio; warrants review without creating panic. |
| `UNCERTAIN` | `INFO` | `"Uncertain (lossy source)"` | `"Uncertain — cannot be determined confidently from this lossy source."` | Lossy MDCT psychoacoustic encoding alters waveforms; uncertainty is reported honestly rather than marking a false pass or false failure. |

---

### 4.5 File Characteristics & Technical Checks

| Check | Good (`GOOD`) | Attention (`ATTENTION`) | Issue (`ISSUE`) | Rationale |
| :--- | :--- | :--- | :--- | :--- |
| **Sample Rate** | `44100` or `48000` Hz | `32000` to `44099` Hz | `< 32000` Hz | 44.1 kHz (CD/podcast standard) and 48 kHz (broadcast/video standard) are universal. < 32 kHz causes audible loss of treble clarity. |
| **Channels** | `1` (Mono) or `2` (Stereo) | `> 2` Channels (Surround) | — | Standard podcast players expect mono or stereo audio. Multi-channel audio is downmixed unpredictably by podcast clients. |
| **Format** | `WAV`, `MP3`, `M4A` | `UNKNOWN` | — | Standard distribution and production formats. Video containers (`MOV`, `MP4`) are marked as `INFO` (audio will be extracted). |
| **Bitrate** | `≥ 96 kbps` (Stereo)<br>`≥ 64 kbps` (Mono) | `< 96 kbps` (Stereo)<br>`< 64 kbps` (Mono) | — | Standard quality bitrates for spoken podcast audio. |

---

## 5. Standards vs. PodReady Opinion

PodReady maintains a clear separation between objective industry standards and subjective product opinions:

### 5.1 Standards & Technical Recommendations
- **Target Loudness**: −16 LUFS (stereo) and −19 LUFS (mono) are grounded in published broadcast & streaming distributor recommendations (Apple Podcasts, Spotify, AES TD1004).
- **True Peak Ceilings**: The −1.5 dBTP target and −0.5 dBTP hard boundary protect against inter-sample peaks and transcoding artifacts produced during lossy distribution compression (MP3/AAC).
- **Format & Channel Architecture**: Mono/Stereo channel topologies and 44.1/48 kHz sample rates align with global audio player decoding standards.

### 5.2 PodReady Opinion & Calibrated Judgement
- **Forgiving Loudness Tolerance**: Tolerating ±1.5 LU before triggering `ATTENTION` respects natural spoken dialogue dynamics instead of enforcing sterile brickwall normalization.
- **Forgiving Boundary Silence**: Allowing up to 2.0s opening silence and 4.0s closing silence accommodates human breath, studio atmosphere, and music fade-outs without penalizing creators.
- **Cautious Clipping Evaluation**: Treating flat-topping as `ATTENTION` rather than an immediate fatal `ISSUE` prevents unnecessary alarm when audio remains fully listenable.
- **Honest Uncertainty**: Lossy files are evaluated as `INFO` (`UNCERTAIN`) for waveform clipping rather than generating false positives or false reassurance.

---

## 6. Overall Episode Readiness

The overall episode status is derived deterministically from individual check statuses:

```text
Are there any ISSUE checks?
  ├─ Yes ──→ NEEDS_ATTENTION ("N things need attention")
  └─ No
       ├─ Are there any ATTENTION checks?
       │    ├─ Yes ──→ ATTENTION ("N thing(s) need attention")
       │    └─ No  ──→ READY ("Ready for publication")
```

- Informational checks (`INFO`) and lossy clipping uncertainty (`UNCERTAIN`) do not cause an otherwise healthy episode to fail.
- The UI renders interpretation first ("Ready", "Attention", "Needs Attention"), supported by technical measurements and profile-driven sparklines.

---

## 7. FixPlan Engine Rules & Safety Guarantees (Stage 4A)

PodReady follows a strict separation of **Decision** (*What should happen?*) from **Execution** (*Actually doing it*). The FixPlan engine consumes the `Assessment` model and deterministically plans corrective actions.

> **Core Principle**:
> *PodReady only automatically plans changes it can make predictably and verify.*

### 7.1 FixAction Decision Matrix

| Source Check | Assessment Trigger | FixAction Generated | Confidence | Modifies Audio | Safety Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Integrated Loudness** | `ATTENTION` or `ISSUE` | `Adjust loudness`<br>Target: Profile target LUFS (−16.0 / −19.0 LUFS) | `HIGH` | Yes | Predictable linear/two-pass gain adjustment that preserves relative speech dynamics and meets standard distribution targets. |
| **True Peak** | `ATTENTION` or `ISSUE` | `Apply peak protection`<br>Ceiling: Profile ceiling (≤ −1.5 dBTP) | `HIGH` | Yes | Transparent true-peak limiting during final encoding prevents inter-sample clipping on streaming transcoders without audible distortion. |
| **Digital Clipping** | `ATTENTION` (`POSSIBLE`) | *None (Review Advisory only)* | `LOW` *(Unsupported)* | No | Automated de-clipping algorithms can introduce synthetic artifacts or alter source timbre. Manual creator review is required. |
| **Boundary Silence** | `ATTENTION` or `ISSUE` | *None (Unsupported in V1)* | `LOW` *(Unsupported)* | No | Trimming opening or trailing silence risks cutting intentional intro/outro music fades, room tone, or creative timing. |
| **Sample Rate / Format** | `ATTENTION` or `ISSUE` | *None in V1 FixPlan* | `LOW` *(Unsupported)* | No | Container and sample-rate transformations are handled during publishing encoding rather than destructive source alteration. |
| **Metadata** | Missing fields | *None in V1 FixPlan* | Future | No | Metadata enrichment is non-audio packaging and will be addressed in metadata publishing stages. |

### 7.2 Confidence Model

- **`HIGH`**: The proposed action is deterministic, mathematically safe, and verifiable against objective delivery standards (e.g. loudness gain, peak headroom).
- **`MEDIUM`**: The proposed action is reasonable but involves subjective editorial judgement (reserved for future optional tools).
- **`LOW` / Unsupported**: The action is not automated because automatic processing carries risk of altering creative intent or damaging audio fidelity.

