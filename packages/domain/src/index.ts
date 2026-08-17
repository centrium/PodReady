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
}

export interface AppError {
  message: string;
  code?: string;
}
