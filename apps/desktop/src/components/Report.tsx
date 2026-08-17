import type { MediaSource } from "@podready/domain";

interface ReportProps {
  media: MediaSource;
  isAnalysing?: boolean;
}

export function Report({ media, isAnalysing }: ReportProps) {
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

  const formatLoudness = (lufs: number | null | undefined) => {
    if (lufs === null || lufs === undefined) return "—";
    const sign = lufs < 0 ? "−" : "";
    return `${sign}${Math.abs(lufs).toFixed(1)} LUFS`;
  };

  const formatTruePeak = (dbtp: number | null | undefined) => {
    if (dbtp === null || dbtp === undefined) return "—";
    const sign = dbtp < 0 ? "−" : "";
    return `${sign}${Math.abs(dbtp).toFixed(1)} dBTP`;
  };

  const formatSeconds = (sec: number | undefined) => {
    if (sec === undefined || isNaN(sec)) return "—";
    return `${sec.toFixed(1)} sec`;
  };

  return (
    <div className="flex flex-col w-full max-w-md p-8 bg-white border border-gray-200 rounded-xl shadow-sm">
      <div className="mb-6">
        <h2 className="text-xl font-bold tracking-tight text-gray-900 mb-1">
          {media.filename}
        </h2>
        <p className="text-3xl font-light text-gray-600">
          {formatTime(media.inspection.durationSeconds)}
        </p>
      </div>

      {/* AUDIO Measurements Section */}
      <div className="mb-6">
        <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase mb-3">
          AUDIO
        </h3>
        {isAnalysing && !media.measurements ? (
          <div className="py-4 text-sm text-gray-500 italic animate-pulse">
            Analysing audio…
          </div>
        ) : media.measurements ? (
          <dl className="divide-y divide-gray-100 text-sm">
            <div className="flex justify-between py-2">
              <dt className="text-gray-600">Integrated loudness</dt>
              <dd className="font-mono font-medium text-gray-900">
                {formatLoudness(media.measurements.integratedLoudnessLufs)}
              </dd>
            </div>
            <div className="flex justify-between py-2">
              <dt className="text-gray-600">True peak</dt>
              <dd className="font-mono font-medium text-gray-900">
                {formatTruePeak(media.measurements.truePeakDbtp)}
              </dd>
            </div>
            <div className="flex justify-between py-2">
              <dt className="text-gray-600">Leading silence</dt>
              <dd className="font-mono font-medium text-gray-900">
                {formatSeconds(media.measurements.leadingSilenceSeconds)}
              </dd>
            </div>
            <div className="flex justify-between py-2">
              <dt className="text-gray-600">Trailing silence</dt>
              <dd className="font-mono font-medium text-gray-900">
                {formatSeconds(media.measurements.trailingSilenceSeconds)}
              </dd>
            </div>
            <div className="flex justify-between py-2">
              <dt className="text-gray-600">Peak clipping</dt>
              <dd className="font-mono font-medium text-gray-900">
                {media.measurements.clipping.evidence === "POSSIBLE"
                  ? `Possible clipping detected${
                      media.measurements.clipping.samplesAtCeiling > 0
                        ? ` (${media.measurements.clipping.samplesAtCeiling.toLocaleString()} samples)`
                        : ""
                    }`
                  : media.measurements.clipping.evidence === "UNCERTAIN"
                  ? "Uncertain (lossy source)"
                  : "No obvious clipping detected"}
              </dd>
            </div>
          </dl>
        ) : null}
      </div>

      {/* FILE Inspection Section */}
      <div>
        <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase mb-3">
          FILE
        </h3>
        <ul className="space-y-2 text-sm font-medium text-gray-700">
          <li className="flex items-center">
            {media.format}
          </li>
          <li className="flex items-center">
            {formatHz(media.inspection.sampleRate)}
          </li>
          <li className="flex items-center">
            {formatChannels(media.inspection.channels)}
          </li>
          {media.inspection.bitrate && (
            <li className="flex items-center">
              {formatBitrate(media.inspection.bitrate)}
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}
