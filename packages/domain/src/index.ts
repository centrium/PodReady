export type MediaFormat = 'WAV' | 'MP3' | 'M4A' | 'MOV' | 'MP4' | 'UNKNOWN';

export interface MediaInspection {
  durationSeconds: number;
  sampleRate: number;
  channels: number;
  bitrate?: number;
  fileSizeBytes: number;
}

export type ClippingEvidence = 'NONE' | 'POSSIBLE' | 'UNCERTAIN';

export interface ClippingAnalysis {
  samplePeakDbfs?: number;
  samplesAtCeiling: number;
  flatFactor: number;
  evidence: ClippingEvidence;
}

export interface AudioMeasurements {
  integratedLoudnessLufs: number | null;
  truePeakDbtp: number | null;
  leadingSilenceSeconds: number;
  trailingSilenceSeconds: number;
  clipping: ClippingAnalysis;
}

export type AssessmentStatus = 'GOOD' | 'ATTENTION' | 'ISSUE' | 'INFO' | 'UNKNOWN';

export type OverallStatus = 'READY' | 'ATTENTION' | 'NEEDS_ATTENTION';

export interface SparklineRange {
  from: number;
  to: number;
}

export interface SparklineConfig {
  min: number;
  max: number;
  target?: number;
  value: number;
  ranges: SparklineRange[];
}

export interface AssessmentCheck {
  id: string;
  label: string;
  status: AssessmentStatus;
  displayValue: string;
  message: string;
  fixable: boolean;
  sparkline?: SparklineConfig;
}

export interface Assessment {
  overallStatus: OverallStatus;
  summary: string;
  profileId: string;
  profileVersion: string;
  profileName: string;
  audioChecks: AssessmentCheck[];
  fileChecks: AssessmentCheck[];
}

export interface MediaSource {
  path: string;
  filename: string;
  format: MediaFormat;
  codec: string;
  inspection: MediaInspection;
  measurements?: AudioMeasurements;
  assessment?: Assessment;
  fixPlan?: FixPlan;
}

export type FixConfidence = 'HIGH' | 'MEDIUM' | 'LOW';

export type FixActionType = 'LOUDNESS_ADJUSTMENT' | 'PEAK_PROTECTION';

export interface FixAction {
  id: string;
  actionType: FixActionType;
  sourceCheckId: string;
  title: string;
  description: string;
  reason: string;
  confidence: FixConfidence;
  changesAudio: boolean;
  fromValue?: string;
  toValue?: string;
}

export interface FixPlan {
  summary: string;
  actions: FixAction[];
  reviewAdvisories: string[];
  changesAudio: boolean;
  totalFixes: number;
}

export interface AppliedAction {
  actionType: FixActionType;
  title: string;
  success: boolean;
  description: string;
  fromValue?: string;
  toValue?: string;
}

export interface ProcessingResult {
  success: boolean;
  outputPath: string;
  actionsApplied: AppliedAction[];
  reviewAdvisories: string[];
  warnings: string[];
  errors: string[];
}

export interface ProcessAudioResponse {
  result: ProcessingResult;
  candidatePath: string;
  candidateFilename: string;
  beforeMeasurements?: AudioMeasurements;
  beforeAssessment?: Assessment;
  afterMeasurements: AudioMeasurements;
  afterAssessment: Assessment;
}


export interface AppError {
  message: string;
  code?: string;
}

export interface EpisodeMetadata {
  title?: string;
  artist?: string;
  album?: string;
  episodeNumber?: string;
  year?: string;
  genre?: string;
  artworkPath?: string;
}

export interface ExportOptions {
  destinationDirectory: string;
  includeAudio: boolean;
  includeTranscript: boolean;
  includeReport: boolean;
  metadata?: EpisodeMetadata;
}

export interface ExportedFile {
  path: string;
  filename: string;
  fileSizeBytes: number;
  fileType: 'audio' | 'transcript' | 'report';
}

export interface ExportVerificationResult {
  passed: boolean;
  overallStatus: OverallStatus;
  summary: string;
  measurements: AudioMeasurements;
  assessment: Assessment;
}

export interface TranscriptSegment {
  startSec: number;
  endSec: number;
  text: string;
}

export interface TranscriptResult {
  text: string;
  language?: string;
  durationSeconds?: number;
  segments: TranscriptSegment[];
}

export interface PodReadyPackage {
  packageDirectory: string;
  packageName: string;
  audioFile?: ExportedFile;
  transcriptFile?: ExportedFile;
  transcriptLanguage?: string;
  transcriptError?: string;
  reportFile?: ExportedFile;
  metadata?: EpisodeMetadata;
  artworkEmbedded: boolean;
  verificationResult: ExportVerificationResult;
  generationDurationSeconds?: number;
  userElapsedSeconds?: number;
  createdAt: string;
}

export function formatPackageDuration(seconds: number): string {
  if (isNaN(seconds) || seconds < 0) {
    return '0.0 seconds';
  }
  return `${seconds.toFixed(1)} seconds`;
}

export type BatchEpisodeStatus =
  | 'WAITING'
  | 'INSPECTING'
  | 'ANALYSING'
  | 'ASSESSING'
  | 'COMPLETE'
  | 'FAILED'
  | 'CANCELLED';

export interface BatchEpisode {
  id: string;
  sourcePath: string;
  filename: string;
  status: BatchEpisodeStatus;
  format?: MediaFormat;
  codec?: string;
  inspection?: MediaInspection;
  measurements?: AudioMeasurements;
  assessment?: Assessment;
  durationSeconds?: number;
  elapsedSeconds?: number;
  error?: string;
}

export interface BatchAnalysisSummary {
  total: number;
  complete: number;
  failed: number;
  cancelled: number;
  ready: number;
  attention: number;
  needsAttention: number;
  elapsedSeconds: number;
}

export type BatchJobStatus = 'QUEUED' | 'RUNNING' | 'COMPLETE' | 'CANCELLED';

export interface BatchAnalysisJob {
  id: string;
  status: BatchJobStatus;
  episodes: BatchEpisode[];
  summary: BatchAnalysisSummary;
  createdAt: string;
}

export interface BatchProgressPayload {
  jobId: string;
  episodeId: string;
  status: BatchEpisodeStatus;
  episode: BatchEpisode;
  summary: BatchAnalysisSummary;
}

export function formatBatchDuration(seconds: number): string {
  if (isNaN(seconds) || seconds < 0) {
    return '0.0 seconds';
  }
  return `${seconds.toFixed(1)} seconds`;
}

export function formatAudioDuration(seconds?: number | null): string {
  if (seconds === null || seconds === undefined || isNaN(seconds) || seconds <= 0) {
    return '—';
  }
  const totalSecs = Math.round(seconds);
  if (totalSecs === 0 && seconds > 0) {
    return '< 1s';
  }
  const hrs = Math.floor(totalSecs / 3600);
  const mins = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;

  if (hrs > 0) {
    return `${hrs}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

// Stage 5B: Local Show Library & Episode Catalogue

export type SourceAvailability = 'AVAILABLE' | 'MISSING' | 'CHANGED';


export type AddEpisodeStatus = 'ADDED' | 'ALREADY_EXISTS' | 'UPDATED';

export interface Show {
  id: string;
  name: string;
  description?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ShowSummary {
  id: string;
  name: string;
  description?: string;
  episodeCount: number;
  lastAnalysedAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface CatalogueEpisode {
  id: string;
  showId: string;
  sourcePath: string;
  filename: string;
  fileSizeBytes: number;
  durationSeconds: number;
  format: MediaFormat;
  codec: string;
  sampleRate: number;
  channels: number;
  bitrate?: number;
  integratedLoudnessLufs?: number | null;
  truePeakDbtp?: number | null;
  leadingSilenceSeconds: number;
  trailingSilenceSeconds: number;
  clippingEvidence: ClippingEvidence;
  overallAssessmentStatus: OverallStatus;
  assessmentProfileId: string;
  assessmentProfileVersion: string;
  analysedAt: string;
  sourceModifiedAt?: string;
  createdAt: string;
  updatedAt: string;
  assessmentJson?: string;
  assessment?: Assessment;
  sourceAvailability: SourceAvailability;
}

export interface ShowWithEpisodes {
  show: Show;
  episodes: CatalogueEpisode[];
}

export interface AddEpisodeOutcome {
  episodeId: string;
  filename: string;
  status: AddEpisodeStatus;
  message?: string;
}

export interface AddBatchEpisodesResult {
  showId: string;
  showName: string;
  totalProcessed: number;
  added: number;
  updated: number;
  alreadyExists: number;
  skippedFailed: number;
  outcomes: AddEpisodeOutcome[];
}

export interface CreateShowInput {
  name: string;
  description?: string;
}

export interface UpdateShowInput {
  id: string;
  name: string;
  description?: string;
}

// Stage 5C: Show Baseline & Historical Characteristics

export type BaselineMaturity = 'NO_DATA' | 'EARLY' | 'DEVELOPING' | 'ESTABLISHED';

export interface ContinuousBaselineMetric {
  id: string;
  label: string;
  unit: string;
  sampleCount: number;
  median: number;
  q1: number;
  q3: number;
  min: number;
  max: number;
}

export interface CategoricalDistributionItem {
  value: string;
  count: number;
  proportion: number;
}

export interface CategoricalBaselineMetric {
  id: string;
  label: string;
  sampleCount: number;
  dominantValue: string;
  dominantCount: number;
  dominantProportion: number;
  distribution: CategoricalDistributionItem[];
}

export interface BaselineExclusionSummary {
  changedSourceCount: number;
  missingMeasurementCount: number;
}

export interface ClippingBaselineSummary {
  totalChecked: number;
  noneCount: number;
  possibleCount: number;
  uncertainCount: number;
}

export interface HistoricalMetricPoint {
  episodeId: string;
  filename: string;
  analysedAt: string;
  value: number;
}

export interface ShowBaseline {
  showId: string;
  showName: string;
  maturity: BaselineMaturity;
  totalEpisodes: number;
  eligibleEpisodes: number;
  excludedEpisodes: number;
  exclusionSummary: BaselineExclusionSummary;
  generatedAt: string;
  loudness?: ContinuousBaselineMetric | null;
  truePeak?: ContinuousBaselineMetric | null;
  duration?: ContinuousBaselineMetric | null;
  leadingSilence?: ContinuousBaselineMetric | null;
  trailingSilence?: ContinuousBaselineMetric | null;
  bitrate?: ContinuousBaselineMetric | null;
  format?: CategoricalBaselineMetric | null;
  sampleRate?: CategoricalBaselineMetric | null;
  channels?: CategoricalBaselineMetric | null;
  codec?: CategoricalBaselineMetric | null;
  clipping: ClippingBaselineSummary;
  loudnessHistory: HistoricalMetricPoint[];
  truePeakHistory: HistoricalMetricPoint[];
}

export function formatBaselineLoudness(metric?: ContinuousBaselineMetric | null): { typical: string; range: string } {
  if (!metric) {
    return { typical: '—', range: '—' };
  }
  const sign = metric.median < 0 ? '−' : '';
  const typical = `${sign}${Math.abs(metric.median).toFixed(1)} LUFS`;
  const q1Sign = metric.q1 < 0 ? '−' : '';
  const q3Sign = metric.q3 < 0 ? '−' : '';
  const range = `${q1Sign}${Math.abs(metric.q1).toFixed(1)} → ${q3Sign}${Math.abs(metric.q3).toFixed(1)} LUFS`;
  return { typical, range };
}

export function formatBaselinePeak(metric?: ContinuousBaselineMetric | null): { typical: string; range: string } {
  if (!metric) {
    return { typical: '—', range: '—' };
  }
  const sign = metric.median < 0 ? '−' : '';
  const typical = `${sign}${Math.abs(metric.median).toFixed(1)} dBTP`;
  const q1Sign = metric.q1 < 0 ? '−' : '';
  const q3Sign = metric.q3 < 0 ? '−' : '';
  const range = `${q1Sign}${Math.abs(metric.q1).toFixed(1)} → ${q3Sign}${Math.abs(metric.q3).toFixed(1)} dBTP`;
  return { typical, range };
}

export function formatBaselineDuration(metric?: ContinuousBaselineMetric | null): { typical: string; range: string } {
  if (!metric) {
    return { typical: '—', range: '—' };
  }
  const typical = formatAudioDuration(metric.median);
  const range = `${formatAudioDuration(metric.q1)} → ${formatAudioDuration(metric.q3)}`;
  return { typical, range };
}

// Stage 5D: Show Check / Episode-to-Show Comparison Engine

export type ShowCheckStatus = 'TYPICAL' | 'DIFFERENT' | 'INSUFFICIENT_DATA';

export type MetricComparisonStatus =
  | 'TYPICAL'
  | 'SLIGHTLY_DIFFERENT'
  | 'DIFFERENT'
  | 'NOT_AVAILABLE';

export type MetricDirection = 'BELOW_USUAL' | 'WITHIN_USUAL' | 'ABOVE_USUAL';

export interface ShowCheckContinuousMetric {
  id: string;
  label: string;
  unit: string;
  candidateValue: number;
  typicalValue: number;
  usualLow: number;
  usualHigh: number;
  status: MetricComparisonStatus;
  direction: MetricDirection;
  message: string;
  sampleCount: number;
  sparkline?: SparklineConfig;
}

export interface ShowCheckCategoricalMetric {
  id: string;
  label: string;
  candidateValue: string;
  typicalValue: string;
  dominantProportion: number;
  status: MetricComparisonStatus;
  message: string;
  sampleCount: number;
}

export interface ShowCheck {
  showId: string;
  showName: string;
  baselineMaturity: BaselineMaturity;
  baselineEpisodeCount: number;
  status: ShowCheckStatus;
  summary: string;
  isStale: boolean;
  metrics: ShowCheckContinuousMetric[];
  categoricalMetrics: ShowCheckCategoricalMetric[];
  generatedAt: string;
}

export function formatShowCheckLoudness(metric?: ShowCheckContinuousMetric | null): { candidate: string; typical: string; range: string } {
  if (!metric) {
    return { candidate: '—', typical: '—', range: '—' };
  }
  const cSign = metric.candidateValue < 0 ? '−' : '';
  const candidate = `${cSign}${Math.abs(metric.candidateValue).toFixed(1)} LUFS`;
  const tSign = metric.typicalValue < 0 ? '−' : '';
  const typical = `${tSign}${Math.abs(metric.typicalValue).toFixed(1)} LUFS`;
  const lSign = metric.usualLow < 0 ? '−' : '';
  const hSign = metric.usualHigh < 0 ? '−' : '';
  const range = `${lSign}${Math.abs(metric.usualLow).toFixed(1)} → ${hSign}${Math.abs(metric.usualHigh).toFixed(1)} LUFS`;
  return { candidate, typical, range };
}

export function formatShowCheckPeak(metric?: ShowCheckContinuousMetric | null): { candidate: string; typical: string; range: string } {
  if (!metric) {
    return { candidate: '—', typical: '—', range: '—' };
  }
  const cSign = metric.candidateValue < 0 ? '−' : '';
  const candidate = `${cSign}${Math.abs(metric.candidateValue).toFixed(1)} dBTP`;
  const tSign = metric.typicalValue < 0 ? '−' : '';
  const typical = `${tSign}${Math.abs(metric.typicalValue).toFixed(1)} dBTP`;
  const lSign = metric.usualLow < 0 ? '−' : '';
  const hSign = metric.usualHigh < 0 ? '−' : '';
  const range = `${lSign}${Math.abs(metric.usualLow).toFixed(1)} → ${hSign}${Math.abs(metric.usualHigh).toFixed(1)} dBTP`;
  return { candidate, typical, range };
}

export function formatShowCheckDuration(metric?: ShowCheckContinuousMetric | null): { candidate: string; typical: string; range: string } {
  if (!metric) {
    return { candidate: '—', typical: '—', range: '—' };
  }
  const candidate = formatAudioDuration(metric.candidateValue);
  const typical = formatAudioDuration(metric.typicalValue);
  const range = `${formatAudioDuration(metric.usualLow)} → ${formatAudioDuration(metric.usualHigh)}`;
  return { candidate, typical, range };
}

export function formatShowCheckSilence(metric?: ShowCheckContinuousMetric | null): { candidate: string; typical: string; range: string } {
  if (!metric) {
    return { candidate: '—', typical: '—', range: '—' };
  }
  const candidate = `${metric.candidateValue.toFixed(1)}s`;
  const typical = `${metric.typicalValue.toFixed(1)}s`;
  const range = `${metric.usualLow.toFixed(1)}s → ${metric.usualHigh.toFixed(1)}s`;
  return { candidate, typical, range };
}

export function formatShowCheckBitrate(metric?: ShowCheckContinuousMetric | null): { candidate: string; typical: string; range: string } {
  if (!metric) {
    return { candidate: '—', typical: '—', range: '—' };
  }
  const toKbps = (val: number): number => Math.round(val >= 1000 ? val / 1000 : val);
  const candidate = `${toKbps(metric.candidateValue)} kbps`;
  const typical = `${toKbps(metric.typicalValue)} kbps`;
  const range = `${toKbps(metric.usualLow)} → ${toKbps(metric.usualHigh)} kbps`;
  return { candidate, typical, range };
}

export function formatSampleRateDisplay(sr: number | string | undefined | null): string {
  if (sr === undefined || sr === null) return '—';
  let num: number;
  if (typeof sr === 'number') {
    num = sr;
  } else {
    const cleaned = sr.toString().replace(/[^0-9.]/g, '');
    num = parseFloat(cleaned);
  }
  if (isNaN(num) || num <= 0) return typeof sr === 'string' ? sr : '—';

  // If already in kHz format e.g. 44.1 or 48
  if (num < 1000) {
    return `${num} kHz`;
  }
  const inKhz = num / 1000;
  // Format with up to 2 decimal places, trimming unnecessary trailing zeros
  const formatted = inKhz % 1 === 0 ? inKhz.toFixed(0) : inKhz.toString();
  return `${formatted} kHz`;
}

export function formatChannelDisplay(ch: number | string | undefined | null): string {
  if (ch === undefined || ch === null) return '—';
  const str = ch.toString().trim().toLowerCase();
  if (str === '1' || str === 'mono') return 'Mono';
  if (str === '2' || str === 'stereo') return 'Stereo';
  const num = parseInt(str, 10);
  if (!isNaN(num)) {
    return num === 1 ? 'Mono' : num === 2 ? 'Stereo' : `${num} Channels`;
  }
  return ch.toString();
}

// Stage 5E: Batch / Show Publishing Domain

export type PublishingEpisodeStage =
  | 'PREPARING'
  | 'PROCESSING'
  | 'VERIFYING'
  | 'EXPORTING'
  | 'TRANSCRIBING'
  | 'PACKAGING';

export type PublishingEpisodeStatus =
  | 'WAITING'
  | 'PREPARING'
  | 'PROCESSING'
  | 'VERIFYING'
  | 'EXPORTING'
  | 'TRANSCRIBING'
  | 'PACKAGING'
  | 'COMPLETE'
  | 'FAILED'
  | 'CANCELLED'
  | 'SKIPPED';

export interface BatchPublishingEpisode {
  episodeId: string;
  sourcePath: string;
  filename: string;
  status: PublishingEpisodeStatus;
  stage?: PublishingEpisodeStage;
  elapsedSeconds?: number;
  sourceAvailability?: SourceAvailability;
  skipReason?: string;
  package?: PodReadyPackage;
  error?: string;
  reanalysed?: boolean;
}

export interface BatchPublishingSummary {
  total: number;
  complete: number;
  partial: number;
  failed: number;
  cancelled: number;
  skipped: number;
  elapsedSeconds: number;
}

export type BatchPublishingJobStatus = 'QUEUED' | 'RUNNING' | 'COMPLETE' | 'CANCELLED';

export interface BatchPublishingJob {
  id: string;
  showId?: string;
  showName?: string;
  status: BatchPublishingJobStatus;
  destinationDirectory: string;
  episodes: BatchPublishingEpisode[];
  summary: BatchPublishingSummary;
  createdAt: string;
  startedAt?: string;
  elapsedSeconds?: number;
}

export interface BatchPublishingProgressPayload {
  jobId: string;
  episodeId: string;
  status: PublishingEpisodeStatus;
  stage?: PublishingEpisodeStage;
  episode: BatchPublishingEpisode;
  summary: BatchPublishingSummary;
}

export interface StartBatchPublishingInput {
  episodeIds: string[];
  showId?: string;
  destinationDirectory: string;
  options?: Partial<ExportOptions>;
}

export function formatBatchPublishingDuration(seconds: number): string {
  if (isNaN(seconds) || seconds < 0) {
    return '0.0 seconds';
  }
  if (seconds < 60) {
    return `${seconds.toFixed(1)} seconds`;
  }
  const totalSecs = Math.round(seconds);
  const mins = Math.floor(totalSecs / 60);
  const remSecs = totalSecs % 60;
  if (mins < 60) {
    return `${mins}m ${remSecs}s`;
  }
  const hrs = Math.floor(mins / 60);
  const remMins = mins % 60;
  return `${hrs}h ${remMins}m ${remSecs}s`;
}

