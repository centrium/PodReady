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


