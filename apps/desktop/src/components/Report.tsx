import React from "react";
import type { MediaSource } from "@podready/domain";

interface ReportProps {
  media: MediaSource;
}

export function Report({ media }: ReportProps) {
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  const formatHz = (hz: number) => {
    return `${(hz / 1000).toFixed(1).replace(".0", "")} kHz`;
  };

  const formatChannels = (channels: number) => {
    if (channels === 1) return "Mono";
    if (channels === 2) return "Stereo";
    return `${channels} Channels`;
  };

  const formatBitrate = (bitrate?: number) => {
    if (!bitrate) return null;
    return `${Math.round(bitrate / 1000)} kbps`;
  };

  return (
    <div className="flex flex-col w-full max-w-md p-8 bg-white border border-gray-200 rounded-xl shadow-sm">
      <div className="mb-8">
        <h2 className="text-xl font-bold tracking-tight text-gray-900 mb-1">
          {media.filename}
        </h2>
        <p className="text-3xl font-light text-gray-600">
          {formatTime(media.measurements.durationSeconds ?? (media.measurements as any).duration_seconds ?? 0)}
        </p>
      </div>

      <div className="mb-4">
        <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase mb-4">
          FILE
        </h3>
        <ul className="space-y-3">
          <li className="flex items-center text-sm font-medium text-gray-700">
            <span className="text-green-500 mr-3">✓</span> {media.format}
          </li>
          <li className="flex items-center text-sm font-medium text-gray-700">
            <span className="text-green-500 mr-3">✓</span>{" "}
            {formatHz(media.measurements.sampleRate ?? (media.measurements as any).sample_rate ?? 0)}
          </li>
          <li className="flex items-center text-sm font-medium text-gray-700">
            <span className="text-green-500 mr-3">✓</span>{" "}
            {formatChannels(media.measurements.channels)}
          </li>
          {media.measurements.bitrate && (
            <li className="flex items-center text-sm font-medium text-gray-700">
              <span className="text-green-500 mr-3">✓</span>{" "}
              {formatBitrate(media.measurements.bitrate)}
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}
