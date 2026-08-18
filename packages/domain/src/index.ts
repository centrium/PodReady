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
  transcriptText?: string;
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

export interface PodReadyPackage {
  packageDirectory: string;
  packageName: string;
  audioFile?: ExportedFile;
  transcriptFile?: ExportedFile;
  reportFile?: ExportedFile;
  metadata?: EpisodeMetadata;
  artworkEmbedded: boolean;
  verificationResult: ExportVerificationResult;
  createdAt: string;
}



