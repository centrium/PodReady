# PodReady — Product & Technical Specification v1.0

**Status:** MVP specification  
**Target price:** £19.99, one-time purchase  
**Platform:** macOS first  
**Product type:** Local-first desktop application

---

## 1. Product Definition

### Product

**PodReady**

### Positioning

**Finished podcast in. Ready-to-publish pack out.**

PodReady is a local desktop application for podcasters who have finished creating and editing an episode and want confidence that the resulting media is technically ready for publication.

PodReady is **not an audio or video editor**.

It accepts a finished audio or video episode, analyses its podcast audio, identifies technical issues, optionally applies safe finishing changes, verifies the resulting output and produces a simple PodReady Pack containing the publishing MP3 and extracted transcript.

For video podcasts, PodReady automatically extracts the podcast audio before processing.

Everything is processed locally.

### Core promise

> **Know your podcast is ready.**

The user should not need to understand LUFS, true peak, codecs, sample rates or audio mastering.

PodReady understands those things for them.

---

# 2. Product Principles

## 2.1 The input is already finished

PodReady assumes creative editing has been completed before import.

It does not provide:

- waveform editing
- timeline editing
- cutting
- trimming content
- filler-word removal
- transcript editing
- rearranging
- recording
- video editing

Boundary silence may be corrected as a technical finishing operation where appropriate.

## 2.2 Simple answer first

PodReady should answer:

**Is this ready?**

Technical measurements are supporting information.

Users should see:

**GOOD**

rather than needing to interpret:

**−16.1 LUFS**

Both may be displayed, but the interpretation comes first.

## 2.3 Opinionated defaults

PodReady should avoid exposing audio-engineering configuration.

Avoid controls such as:

- target LUFS
- compressor ratio
- EQ frequency
- limiter threshold
- attack/release
- codec parameters

PodReady chooses sensible podcast defaults.

## 2.4 Non-destructive

The original source file must never be modified.

Every operation creates a new output.

## 2.5 Verify, don't assume

Any audio processed by PodReady must be analysed again after processing.

A file may only receive **PodReady / Ready** status based on the actual output file.

## 2.6 Local first

Audio, video and transcripts remain on the user's machine.

Core operation requires:

- no account
- no cloud upload
- no backend
- no subscription
- no external processing API

## 2.7 No generative content

Transcription is extraction.

PodReady does not generate:

- show notes
- summaries
- titles
- descriptions
- blog posts
- social posts
- chapters
- marketing copy

The transcript represents the spoken content of the source.

---

# 3. Target Customer

Primary customer:

**Independent podcasters serious enough to care about quality but without a dedicated audio engineer.**

Examples include:

- solo podcasters
- small independent shows
- video podcasters
- two-person shows
- small businesses
- consultants
- coaches
- clubs
- organisations
- small production teams

PodReady is not primarily intended for professional broadcast facilities or large podcast networks with established mastering workflows.

---

# 4. Primary User Journey

## 4.1 Audio podcast

Input:

`episode-037.wav`

Pipeline:

```text
Import
  ↓
Inspect
  ↓
Analyse
  ↓
PodReady Report
  ↓
Choose Finish
  ↓
Apply Safe Corrections
  ↓
Encode Publishing MP3
  ↓
Verify Output
  ↓
Extract Transcript
  ↓
Apply Metadata
  ↓
PodReady Pack
```

Output:

```text
Episode 037 - PodReady/

  episode-037.mp3
  episode-037-transcript.txt
```

## 4.2 Video podcast

Input:

`episode-037.mp4`

Pipeline:

```text
Import
  ↓
Identify Video
  ↓
Extract Audio
  ↓
Inspect
  ↓
Analyse
  ↓
PodReady Report
  ↓
Choose Finish
  ↓
Apply Safe Corrections
  ↓
Encode Publishing MP3
  ↓
Verify Output
  ↓
Extract Transcript
  ↓
Apply Metadata
  ↓
PodReady Pack
```

Output:

```text
Episode 037 - PodReady/

  episode-037.mp3
  episode-037-transcript.txt
```

Both input types therefore converge on the same internal audio pipeline.

---

# 5. Supported Input

Initial target formats:

### Audio

- WAV
- MP3
- M4A/AAC

### Video

- MP4
- MOV

The architecture should permit additional FFmpeg-supported formats later without changing the domain model.

---

# 6. Episode Preflight

PodReady analyses the audio without modifying the source.

## 6.1 Audio measurements

At minimum:

- integrated loudness
- true peak
- clipping
- leading silence
- trailing silence
- duration
- channel count
- sample rate
- codec
- bitrate where applicable

Additional useful measurements may be captured internally if essentially free to obtain, but they should not automatically appear in the interface.

## 6.2 File inspection

Capture:

- source format
- audio codec
- sample rate
- channels
- bitrate
- duration
- file size

For video also capture enough information to identify the source correctly, but PodReady 1.0 does not perform video-quality certification.

---

# 7. Assessment Engine

Measurement and assessment must be separate systems.

```text
Source
  ↓
Analyzer
  ↓
Measurements
  ↓
PodReady Profile
  ↓
Assessment
  ↓
Report
```

FFmpeg determines facts.

PodReady determines what those facts mean.

Example:

```text
Measurement

integratedLoudness = -20.7
```

PodReady rules may interpret this as:

```text
status = warning
message = "Quieter than we'd recommend for a podcast."
```

This separation is mandatory.

---

# 8. PodReady Profiles

V1 should be opinionated.

Initial profiles:

### Podcast — Stereo

Default for stereo podcast audio.

### Podcast — Mono

Appropriate mono podcast delivery profile.

The architecture should allow future profiles without rewriting the analyzer.

Potential future profiles include:

- spoken word
- music-heavy podcast
- custom professional profile

These are not required for V1.

---

# 9. PodReady Report

The report is the primary product experience.

Example:

```text
EPISODE 037

78
ALMOST READY

3 things need attention


SOUND

✓ Overall loudness        −16.1 LUFS
⚠ Peak level              −0.2 dBTP
✓ Clipping                None detected
⚠ Opening silence          5.8 sec


FILE

✓ WAV
✓ 48 kHz
✓ Stereo
✓ 52:18


EPISODE DETAILS

✓ Podcast
✓ Title
○ Artwork not embedded


3 safe fixes available

[ MAKE PODREADY ]
```

The interface should avoid presenting every technical measurement with equal importance.

---

# 10. Status Model

Individual checks should use statuses such as:

- Good
- Attention
- Significant issue
- Informational
- Not applicable

The overall episode should use human-readable states such as:

- **Ready**
- **Almost Ready**
- **Needs Attention**

Avoid implying formal certification by Apple, Spotify or other platforms.

PodReady may state that measurements fall within published recommendations where appropriate, but must not claim platform approval.

---

# 11. PodReady Score

A simple score may be shown to make the report immediately understandable.

Example:

**94 — READY**

The score must not combine unrelated items naively.

In particular:

**missing optional metadata must not make technically excellent audio appear poor.**

Audio quality and optional completeness should remain conceptually separate.

The precise scoring model should be implemented as a versioned rules configuration so it can be changed later.

---

# 12. Sexy Sparklines

PodReady should use Sexy Sparklines where visualization communicates information faster than text.

Rule:

> Every sparkline must answer a question.

Suitable uses:

### Loudness

**Is this episode in the desired range?**

Use a Range/Bullet Sparkline.

### True peak

**Is the peak safely below the recommended limit?**

Use a Range/Bullet Sparkline.

### Show consistency

**Does this episode differ from the rest of the show?**

Use Dot/Distribution/Line Sparklines.

Example:

```text
SHOW LOUDNESS

      ●  ●     ●
 ● ●        ●     ● ●
──────────────────────── expected
                       ●
                       E34
```

Charts must remain supporting information.

PodReady must not become an analytics dashboard.

---

# 13. Safe Finishing

PodReady may correct technical delivery issues.

The processing engine must create an explicit `FixPlan`.

Example:

```text
FixPlan

- Correct loudness
- Reduce true peak
- Remove excess opening silence
- Encode publishing MP3
- Embed metadata
```

The user should be able to understand what PodReady intends to change.

The system should avoid processing audio unnecessarily.

---

# 14. Voice Finish Presets

PodReady provides four deliberately simple finishing choices:

### Original

Preserve the tonal character of the source.

Only necessary delivery processing is applied.

### Natural

Balanced, clean and faithful.

Intended as the default PodReady finishing profile.

### Rich

Subtly warmer and fuller vocal presentation.

### Bright

Subtly clearer and more present vocal presentation.

These presets are fixed, professionally designed processing chains.

Users do **not** receive individual EQ/compression controls.

Conceptually:

```text
ORIGINAL
Delivery corrections → loudness → limiter

NATURAL
Gentle levelling → subtle tonal balance → loudness → limiter

RICH
Subtle warmth → gentle compression → loudness → limiter

BRIGHT
Subtle presence → gentle compression → loudness → limiter
```

Exact DSP values require listening tests before release.

---

# 15. Finish Preview

Voice presets justify minimal audio playback.

PodReady may create a short representative speech preview.

Example:

```text
VOICE FINISH

Original   Natural   Rich   Bright
              ●

▶ Preview 10 seconds

Natural
Balanced, clear and consistent.
```

This is **not** an audio player/editor.

There is:

- no waveform
- no timeline
- no editing
- no manual seeking requirement

The preview exists solely to compare finishes.

---

# 16. Processing Pipeline

Conceptually:

```text
Source
  ↓
Initial Analysis
  ↓
Assessment
  ↓
FixPlan
  ↓
Voice Finish
  ↓
Delivery Processing
  ↓
Encode
  ↓
Candidate MP3
  ↓
FINAL ANALYSIS
  ↓
Verification
  ↓
PodReady Output
```

Verification is mandatory.

If the resulting candidate does not satisfy the intended profile, PodReady must not display a successful Ready result.

---

# 17. Video Audio Extraction

When the source contains video, PodReady extracts its audio stream automatically.

The user should not need to perform an explicit conversion step.

Conceptually:

```text
episode.mp4
     ↓
Extract audio stream
     ↓
PodReady audio pipeline
     ↓
episode-podready.mp3
```

The resulting MP3 is not merely extracted audio.

It is the extracted audio after the complete PodReady finishing and verification pipeline.

UI language should favour:

**Create podcast audio**

rather than:

**Extract audio stream**

---

# 18. Transcript Extraction

PodReady can extract spoken words into text using local speech-to-text.

V1 output:

`episode-037-transcript.txt`

Optional additional formats can include:

- Markdown
- SRT
- VTT

TXT is the required V1 format.

The transcript should remain faithful to the spoken content.

PodReady does not provide transcript editing.

PodReady does not use the transcript to generate new content.

---

# 19. Transcription Architecture

Preferred approach:

**whisper.cpp or equivalent local Whisper implementation**

Requirements:

- completely local inference
- no API dependency
- no account
- no uploading audio
- model management handled by PodReady
- progress exposed to the UI
- transcription failure must not prevent creation of the podcast MP3

Transcription should not block initial audio analysis.

Conceptually:

```text
                    ┌→ Audio analysis → Report
Source → Audio ─────┤
                    └→ Transcription → TXT
```

---

# 20. Metadata

Metadata is optional enrichment rather than a condition of audio readiness.

PodReady should inspect and preserve existing metadata where possible.

Supported editable fields should include:

- episode/title
- podcast/show name
- episode number
- year/date
- genre
- comments
- embedded cover artwork

For MP3 output this will primarily be represented using ID3 metadata.

The UI should use podcast terminology rather than exposing ID3 field names.

Example:

```text
EPISODE DETAILS                     OPTIONAL

Podcast
[ The Sunday Running Show ]

Episode title
[ Why We Keep Running ]

Episode
[ 37 ]

Year
[ 2026 ]

Genre
[ Podcast ]

Artwork
[ cover.jpg ]       3000 × 3000
```

Missing metadata should not cause an otherwise valid episode to fail PodReady.

---

# 21. Metadata Preservation

Existing source metadata should be preserved wherever sensible.

PodReady should avoid replacing valid existing information unless the user explicitly changes it.

If only metadata needs to change, audio should not be unnecessarily re-encoded.

---

# 22. Show Identity

PodReady may remember show-level information locally:

```text
Show

Name
Artwork
Preferred finish
Preferred output profile
Typical loudness
```

This allows subsequent episodes to require less configuration.

Example:

```text
THE SUNDAY RUNNING SHOW

Preferred finish       Rich
Output                  Podcast MP3
Artwork                 show-cover.jpg
```

---

# 23. PodReady Pack

Successful processing produces a simple output package.

Audio input:

```text
Episode 037 - PodReady/

  episode-037.mp3
  episode-037-transcript.txt
```

Video input produces the same user-facing result.

Potential optional exports:

```text
episode-037.srt
episode-037.vtt
```

The original source remains untouched.

---

# 24. Show Check

Users may drop multiple episodes or a podcast archive into PodReady.

PodReady analyses the collection for consistency.

Measurements may include:

- loudness
- true peak
- encoding
- sample rate
- channel configuration
- artwork
- metadata completeness
- duration
- transcript availability

Example:

```text
YOUR SHOW

47 episodes analysed

LOUDNESS CONSISTENCY

E41    −16.2    ✓
E42    −15.8    ✓
E43    −20.7    ⚠
E44    −16.1    ✓
E45    −14.4    ⚠

43 consistent
4 need attention
```

---

# 25. Show-relative Assessment

PodReady should eventually distinguish between:

### Podcast profile

Does the episode meet PodReady's technical expectations?

and:

### Your show

Does this episode behave like the user's other episodes?

Example:

```text
Technically acceptable                 ✓

Different from your usual episodes     ⚠
```

This is a core differentiator.

---

# 26. Local Catalogue

PodReady maintains a lightweight local technical catalogue of analysed episodes.

This is **not** a content-management system.

The catalogue exists so PodReady can understand the historical output of a show.

Example:

```text
THE SUNDAY RUNNING SHOW

84 episodes
68h 42m

Quality                     94

Loudness consistency        Excellent
Metadata completeness       96%
Artwork consistency         Excellent
Encoding consistency        Good
```

Episode records may contain:

- source fingerprint/path reference
- title
- episode number
- duration
- analysis measurements
- assessment
- output details
- transcript status
- artwork characteristics
- processing date
- selected finish

---

# 27. Show Health

Catalogue analysis can detect:

- loudness outliers
- peak outliers
- encoding changes
- sample-rate changes
- mono/stereo changes
- artwork inconsistencies
- metadata gaps
- duplicate episode numbers
- missing episode numbers
- unusual duration
- transcript availability

The system should distinguish technical problems from informational inconsistencies.

---

# 28. Archive Completion

Show Health may present completion information such as:

```text
ARCHIVE

84 episodes

Audio checked        84 / 84
Metadata             79 / 84
Artwork              84 / 84
Transcripts          31 / 84

5 episodes need attention
```

This gives PodReady recurring utility beyond processing a single episode.

---

# 29. Explicitly Out of Scope

PodReady 1.0 must not implement:

- waveform editing
- timeline editing
- transcript editing
- word deletion
- filler-word removal
- silence editing within content
- recording
- video editing
- clip generation
- social content generation
- show-note generation
- summarisation
- title generation
- chapter generation
- podcast hosting
- RSS publishing
- scheduling
- guest management
- episode planning
- playlists
- user-created tagging
- cloud accounts
- collaborative editing

If a proposed feature requires manipulating the creative content of the episode, it probably belongs somewhere else.

---

# 30. Technical Architecture

Recommended V1 stack:

```text
PodReady
│
├── Tauri 2
│
├── React
│   ├── TypeScript
│   ├── Vite
│   ├── Tailwind
│   └── Sexy Sparklines
│
├── Rust
│   ├── file handling
│   ├── job orchestration
│   ├── FFmpeg process management
│   ├── analysis
│   ├── assessment
│   ├── fix planning
│   ├── catalogue
│   ├── verification
│   └── packaging
│
├── FFmpeg
│   ├── audio extraction
│   ├── loudness analysis
│   ├── silence analysis
│   ├── statistics
│   ├── finishing
│   └── encoding
│
├── ffprobe
│   └── media inspection
│
└── whisper.cpp
    └── local transcription
```

---

# 31. Frontend

Recommended:

- React
- TypeScript
- Vite
- Tailwind
- Sexy Sparklines
- minimal animation library only where useful

Frontend responsibilities:

- file drop
- job progress
- report rendering
- sparkline visualization
- finish selection
- preview playback
- metadata input
- show catalogue
- export actions

The frontend should not perform audio processing.

---

# 32. Rust Core

Rust owns:

- filesystem access
- temporary workspaces
- process execution
- FFmpeg/ffprobe invocation
- transcription invocation
- parsing
- assessment
- fix planning
- processing
- verification
- catalogue persistence
- output packaging

The UI communicates with the Rust domain layer rather than directly invoking FFmpeg.

---

# 33. FFmpeg

FFmpeg provides the underlying media engine.

Likely functionality includes:

- `ffprobe`
- `loudnorm`
- `ebur128`
- `silencedetect`
- `astats`
- encoding
- stream extraction
- metadata embedding
- artwork embedding

PodReady must not expose FFmpeg terminology directly to ordinary users.

FFmpeg output should be parsed into structured internal types.

---

# 34. Core Domain Models

Illustrative structures:

```text
MediaSource

id
path
mediaType
duration
streams
metadata
```

```text
AudioMeasurements

integratedLoudness
truePeak
clipping
leadingSilence
trailingSilence
sampleRate
channels
codec
bitrate
duration
```

```text
Assessment

overallStatus
audioStatus
issues[]
information[]
score
profileVersion
```

```text
Issue

id
severity
measurement
value
message
fixable
suggestedFix
```

```text
FixPlan

sourceId
finishPreset
operations[]
outputProfile
metadataChanges
```

```text
Verification

outputMeasurements
assessment
passed
```

```text
EpisodeRecord

id
showId
sourceFingerprint
metadata
measurements
assessment
finish
output
transcript
processedAt
```

---

# 35. Job Model

Long-running work must not block the UI.

Examples:

```text
AnalyzeJob
ProcessJob
TranscriptionJob
BatchAnalysisJob
PreviewJob
```

Each job should expose:

- ID
- state
- progress
- stage
- failure information
- cancellation where practical

Example stages:

```text
Checking source
Analysing loudness
Checking peaks
Checking silence
Preparing audio
Encoding
Verifying
Transcribing
Packaging
```

---

# 36. Temporary Workspace

Every processing job receives an isolated temporary workspace.

Conceptually:

```text
PodReady/cache/<job-id>/

analysis.json
preview-original.*
preview-natural.*
preview-rich.*
preview-bright.*
candidate.mp3
verification.json
transcript.txt
```

Temporary artifacts should be cleaned safely.

The source is referenced but never overwritten.

---

# 37. Local Persistence

PodReady does not require a server database.

A lightweight local database such as SQLite is appropriate for:

- shows
- episode catalogue
- measurements
- assessments
- processing history
- preferences

Binary media should not be stored in the database.

The catalogue should reference files and retain technical measurements.

---

# 38. File Identity

PodReady should avoid treating the same file as a completely new episode every time it is dropped.

A lightweight fingerprint can combine appropriate values such as:

- file size
- modification information
- media duration
- partial/full content hash where required

The exact strategy should balance reliability and performance.

---

# 39. Bundled Dependencies

Users must not install FFmpeg, ffprobe or transcription tools separately.

Installation experience:

```text
Download PodReady
↓
Install
↓
Open
↓
Drop episode
```

Tauri sidecars/resources should package required native binaries.

Commercial distribution must use an FFmpeg build/configuration compatible with PodReady's licensing and distribution requirements.

This must be validated before release.

---

# 40. Failure Handling

Failures must be understandable.

Avoid:

```text
FFmpeg exited with code 1
```

Prefer:

```text
We couldn't read the audio in this file.

Your original hasn't been changed.

[ TRY AGAIN ]
```

Detailed diagnostics may be available separately.

No failure should damage the source media.

---

# 41. Privacy

Core privacy promise:

> **Your unreleased podcast stays on your Mac.**

PodReady should not upload source audio/video or transcripts.

If telemetry is ever introduced, it must never contain:

- source media
- transcripts
- episode audio
- filenames without explicit justification
- embedded metadata content

Core functionality cannot depend on telemetry.

---

# 42. Performance

Initial goals:

- file recognition should feel immediate
- report UI should progressively update where useful
- analysis should begin immediately after drop
- transcription should run independently of initial report generation
- UI must remain responsive during FFmpeg/Whisper operations
- batch analysis should use a bounded job queue rather than launching unlimited processes

Exact performance targets should be established using representative 30-, 60- and 120-minute podcast files.

---

# 43. Seven-Day MVP

The full V1 specification defines the product direction, but the first release experiment should be built vertically.

## Priority 1 — Complete single-file pipeline

Must work before anything else:

```text
Drop
→ Inspect
→ Analyse
→ Report
→ Finish
→ Encode
→ Verify
→ MP3
```

## Priority 2 — Transcript

```text
Audio
→ Local transcription
→ TXT
```

## Priority 3 — Metadata

Read, preserve, optionally add and embed metadata/artwork.

## Priority 4 — Voice presets

Original / Natural / Rich / Bright with short preview.

## Priority 5 — Show Check

Multi-file analysis and consistency visualization.

## Priority 6 — Catalogue

Persist show/episode technical history locally.

If schedule pressure occurs, later priorities must not compromise the correctness of Priority 1.

---

# 44. Definition of Done for the Core Pipeline

A representative finished WAV can be dropped into PodReady.

PodReady:

1. identifies the media correctly;
2. measures its relevant audio characteristics;
3. presents a comprehensible report;
4. identifies legitimate technical issues;
5. creates an explicit safe processing plan;
6. produces a publishing MP3;
7. analyses the produced MP3;
8. confirms the output meets the selected PodReady profile;
9. leaves the original untouched;
10. produces a usable final file.

The same workflow must work for a representative video podcast, with audio extraction occurring automatically.

---

# 45. Product Success Test

The MVP is not validated merely because it works technically.

The commercial hypothesis is:

> An independent podcaster will pay approximately £19.99 to remove uncertainty and repetitive technical work between finishing an episode and publishing it.

The product should therefore optimise for the moment:

```text
DROP

↓

"Oh — I didn't know that."

↓

MAKE PODREADY

↓

READY TO PUBLISH
```

The report must demonstrate value before the user is asked to pay.

A potential commercial model is:

**Analysis / Show Check:** free

**Finishing, verification, transcription and PodReady Pack:** £19.99 unlock

This should be tested rather than assumed.

---

# 46. Future Candidates — Not V1

Potential later additions:

- watched folder
- RSS-feed archive import
- additional transcript formats
- archive report export
- additional PodReady profiles
- batch finishing
- Windows support
- automatic show recognition
- more sophisticated show-relative baselines

These features must not delay V1.

---

# 47. Final Scope Rule

When evaluating any new feature, ask:

> **Does this help determine whether a finished podcast is ready, safely prepare it for delivery, or extract a useful publishing asset from it?**

If yes, it may belong in PodReady.

If it changes, rearranges or creates the podcast's content, it does not.

---

# 48. Product Summary

**PodReady is the last step, not the editing step.**

```text
FINISHED PODCAST
       ↓
     INSPECT
       ↓
      FINISH
       ↓
     EXTRACT
       ↓
      VERIFY
       ↓
     PACKAGE
       ↓
   READY TO PUBLISH
```

The core product advantage is not FFmpeg, Whisper, metadata or sparklines individually.

It is turning several technical podcast-finishing tasks into one understandable decision:

> **Your episode is ready.**
