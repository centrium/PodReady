import React from "react";
import type { ShowBaseline } from "@podready/domain";
import {
  formatBaselineLoudness,
  formatBaselinePeak,
  formatBaselineDuration,
} from "@podready/domain";

interface ShowBaselineSectionProps {
  baseline: ShowBaseline | null;
}

export const ShowBaselineSection: React.FC<ShowBaselineSectionProps> = ({ baseline }) => {
  if (!baseline || baseline.maturity === "NO_DATA" || baseline.eligibleEpisodes === 0) {
    return (
      <div className="bg-gray-50 border border-gray-200/80 rounded-xl p-5 text-center">
        <div className="flex flex-col items-center justify-center space-y-1">
          <p className="text-xs font-semibold text-gray-700 uppercase tracking-wider">
            Show Baseline
          </p>
          <p className="text-xs text-gray-500 max-w-md">
            No baseline yet. Add analysed episodes to begin building a historical picture of this Show.
          </p>
        </div>
      </div>
    );
  }

  const {
    maturity,
    totalEpisodes,
    eligibleEpisodes,
    excludedEpisodes,
    exclusionSummary,
    loudness,
    truePeak,
    duration,
    leadingSilence,
    trailingSilence,
    format,
    sampleRate,
    channels,
    clipping,
  } = baseline;

  const loudnessFormatted = formatBaselineLoudness(loudness);
  const peakFormatted = formatBaselinePeak(truePeak);
  const durationFormatted = formatBaselineDuration(duration);

  const getMaturityBadge = () => {
    switch (maturity) {
      case "ESTABLISHED":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-emerald-100 text-emerald-800">
            Established Baseline
          </span>
        );
      case "DEVELOPING":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-sky-100 text-sky-800">
            Baseline Developing
          </span>
        );
      case "EARLY":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-amber-100 text-amber-800">
            Early Baseline
          </span>
        );
      default:
        return null;
    }
  };

  const formatHz = (hzStr?: string) => {
    if (!hzStr) return "—";
    if (hzStr.includes("44100")) return "44.1 kHz";
    if (hzStr.includes("48000")) return "48 kHz";
    if (hzStr.includes("32000")) return "32 kHz";
    if (hzStr.includes("22050")) return "22.05 kHz";
    return hzStr;
  };

  return (
    <div className="bg-slate-50/70 border border-slate-200/90 rounded-xl p-5 space-y-4 shadow-2xs">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-slate-200/60 pb-3">
        <div className="space-y-0.5">
          <div className="flex items-center space-x-2">
            <h3 className="text-xs font-bold text-slate-800 uppercase tracking-wider">
              Show Baseline
            </h3>
            {getMaturityBadge()}
          </div>
          <p className="text-xs text-slate-500">
            Based on {eligibleEpisodes} {eligibleEpisodes === 1 ? "episode" : "episodes"}
            {excludedEpisodes > 0 && ` (${eligibleEpisodes} of ${totalEpisodes} used)`}
            {exclusionSummary.changedSourceCount > 0 && (
              <span className="text-amber-700 ml-1">
                · {exclusionSummary.changedSourceCount}{" "}
                {exclusionSummary.changedSourceCount === 1 ? "changed source is" : "changed sources are"} excluded until re-analysed
              </span>
            )}
          </p>
        </div>
      </div>

      {/* Main Grid: Acoustics & Duration */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
        {/* Typical Loudness */}
        <div className="bg-white border border-slate-200 rounded-lg p-3.5 flex flex-col justify-between space-y-1">
          <div>
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wide">
              Typical Loudness
            </span>
            <div className="text-lg font-bold font-mono text-slate-900 mt-0.5">
              {loudnessFormatted.typical}
            </div>
          </div>
          <div className="text-[11px] text-slate-500">
            Usual range: <span className="font-mono text-slate-700 font-medium">{loudnessFormatted.range}</span>
          </div>
        </div>

        {/* Typical True Peak */}
        <div className="bg-white border border-slate-200 rounded-lg p-3.5 flex flex-col justify-between space-y-1">
          <div>
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wide">
              Typical True Peak
            </span>
            <div className="text-lg font-bold font-mono text-slate-900 mt-0.5">
              {peakFormatted.typical}
            </div>
          </div>
          <div className="text-[11px] text-slate-500">
            Usual range: <span className="font-mono text-slate-700 font-medium">{peakFormatted.range}</span>
          </div>
        </div>

        {/* Typical Duration */}
        <div className="bg-white border border-slate-200 rounded-lg p-3.5 flex flex-col justify-between space-y-1">
          <div>
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wide">
              Typical Duration
            </span>
            <div className="text-lg font-bold font-mono text-slate-900 mt-0.5">
              {durationFormatted.typical}
            </div>
          </div>
          <div className="text-[11px] text-slate-500">
            Usual range: <span className="font-mono text-slate-700 font-medium">{durationFormatted.range}</span>
          </div>
        </div>
      </div>

      {/* Secondary Metrics: Delivery & Technical Characteristics */}
      <div className="pt-1">
        <div className="text-[11px] font-bold text-slate-600 uppercase tracking-wider mb-2">
          Delivery Characteristics
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
          {/* Format */}
          <div className="bg-white/80 border border-slate-200/80 rounded-lg p-2.5 space-y-0.5">
            <span className="text-[10px] font-medium text-slate-400 uppercase">Format</span>
            <div className="text-xs font-bold text-slate-800">
              {format?.dominantValue || "—"}
            </div>
            <div className="text-[10px] text-slate-500">
              {format
                ? `${format.dominantCount} of ${format.sampleCount}${format.sampleCount < eligibleEpisodes ? " measured" : ""} episodes`
                : "—"}
            </div>
          </div>

          {/* Sample Rate */}
          <div className="bg-white/80 border border-slate-200/80 rounded-lg p-2.5 space-y-0.5">
            <span className="text-[10px] font-medium text-slate-400 uppercase">Sample Rate</span>
            <div className="text-xs font-bold text-slate-800">
              {formatHz(sampleRate?.dominantValue)}
            </div>
            <div className="text-[10px] text-slate-500">
              {sampleRate
                ? `${sampleRate.dominantCount} of ${sampleRate.sampleCount}${sampleRate.sampleCount < eligibleEpisodes ? " measured" : ""} episodes`
                : "—"}
            </div>
          </div>

          {/* Channels */}
          <div className="bg-white/80 border border-slate-200/80 rounded-lg p-2.5 space-y-0.5">
            <span className="text-[10px] font-medium text-slate-400 uppercase">Channels</span>
            <div className="text-xs font-bold text-slate-800">
              {channels?.dominantValue || "—"}
            </div>
            <div className="text-[10px] text-slate-500">
              {channels
                ? `${channels.dominantCount} of ${channels.sampleCount}${channels.sampleCount < eligibleEpisodes ? " measured" : ""} episodes`
                : "—"}
            </div>
          </div>

          {/* Clipping Evidence Frequency */}
          <div className="bg-white/80 border border-slate-200/80 rounded-lg p-2.5 space-y-0.5">
            <span className="text-[10px] font-medium text-slate-400 uppercase">Clipping</span>
            <div className="text-xs font-bold text-slate-800">
              {clipping.possibleCount > 0 ? "Possible clipping" : "None detected"}
            </div>
            <div className="text-[10px] text-slate-500">
              {clipping.possibleCount > 0
                ? `${clipping.possibleCount} of ${clipping.totalChecked} episodes`
                : `${clipping.noneCount} of ${clipping.totalChecked} episodes`}
            </div>
          </div>
        </div>
      </div>

      {/* Silence boundary line if available */}
      {(leadingSilence || trailingSilence) && (
        <div className="text-[11px] text-slate-500 flex flex-wrap items-center gap-x-5 gap-y-1.5 pt-2 border-t border-slate-200/60">
          {leadingSilence && (
            <div className="flex items-center space-x-1">
              <span className="font-medium text-slate-600">Opening silence:</span>
              <span className="text-slate-800 font-mono font-semibold">{leadingSilence.median.toFixed(1)}s</span>
              <span className="text-slate-400 font-mono text-[10px]">(usual {leadingSilence.q1.toFixed(1)}s → {leadingSilence.q3.toFixed(1)}s)</span>
            </div>
          )}
          {leadingSilence && trailingSilence && (
            <span className="text-slate-300 hidden sm:inline">•</span>
          )}
          {trailingSilence && (
            <div className="flex items-center space-x-1">
              <span className="font-medium text-slate-600">Closing silence:</span>
              <span className="text-slate-800 font-mono font-semibold">{trailingSilence.median.toFixed(1)}s</span>
              <span className="text-slate-400 font-mono text-[10px]">(usual {trailingSilence.q1.toFixed(1)}s → {trailingSilence.q3.toFixed(1)}s)</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
