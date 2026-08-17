export type MediaFormat = 'WAV' | 'MP3' | 'M4A' | 'MOV' | 'MP4' | 'UNKNOWN';

export interface AudioMeasurements {
  durationSeconds: number;
  sampleRate: number;
  channels: number;
  bitrate?: number;
  fileSizeBytes: number;
}

export interface MediaSource {
  path: string;
  filename: string;
  format: MediaFormat;
  codec: string;
  measurements: AudioMeasurements;
}

export interface AppError {
  message: string;
  code?: string;
}
