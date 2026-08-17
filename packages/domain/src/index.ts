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

export interface MediaSource {
  path: string;
  filename: string;
  format: MediaFormat;
  codec: string;
  inspection: MediaInspection;
  measurements?: AudioMeasurements;
}

export interface AppError {
  message: string;
  code?: string;
}
