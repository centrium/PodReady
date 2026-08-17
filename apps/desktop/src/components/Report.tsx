import type { MediaSource, AssessmentStatus, OverallStatus } from "@podready/domain";
import { BulletSparkline } from "sexy-sparklines";

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

  const getStatusBadge = (status: AssessmentStatus) => {
    switch (status) {
      case "GOOD":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-emerald-100 text-emerald-800">
            Good
          </span>
        );
      case "ATTENTION":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-amber-100 text-amber-800">
            Attention
          </span>
        );
      case "ISSUE":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-rose-100 text-rose-800">
            Issue
          </span>
        );
      case "INFO":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-slate-100 text-slate-700">
            Info
          </span>
        );
      case "UNKNOWN":
      default:
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-600">
            Unknown
          </span>
        );
    }
  };

  const getOverallStatusBanner = (overallStatus: OverallStatus, summary: string) => {
    switch (overallStatus) {
      case "READY":
        return (
          <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-emerald-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-emerald-900 uppercase">
                  Ready
                </h3>
              </div>
              <span className="text-xs font-medium text-emerald-700">
                {summary}
              </span>
            </div>
          </div>
        );
      case "ATTENTION":
        return (
          <div className="p-4 bg-amber-50 border border-amber-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-amber-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-amber-900 uppercase">
                  Attention
                </h3>
              </div>
              <span className="text-xs font-semibold text-amber-800">
                {summary}
              </span>
            </div>
          </div>
        );
      case "NEEDS_ATTENTION":
        return (
          <div className="p-4 bg-rose-50 border border-rose-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-rose-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-rose-900 uppercase">
                  Needs Attention
                </h3>
              </div>
              <span className="text-xs font-semibold text-rose-800">
                {summary}
              </span>
            </div>
          </div>
        );
    }
  };

  const assessment = media.assessment;

  return (
    <div className="flex flex-col w-full max-w-lg p-8 bg-white border border-gray-200 rounded-2xl shadow-sm space-y-6">
      {/* EPISODE Header */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs font-bold tracking-widest text-gray-400 uppercase">
            Episode
          </span>
          {assessment && (
            <span className="text-xs font-medium text-gray-400">
              {assessment.profileName}
            </span>
          )}
        </div>
        <h2 className="text-xl font-bold tracking-tight text-gray-900 truncate">
          {media.filename}
        </h2>
        <p className="text-3xl font-light text-gray-600 mt-1">
          {formatTime(media.inspection.durationSeconds)}
        </p>
      </div>

      {/* OVERALL READINESS STATUS */}
      {assessment && (
        <div>
          {getOverallStatusBanner(assessment.overallStatus, assessment.summary)}
        </div>
      )}

      {/* AUDIO CHECKS SECTION */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase">
            Audio
          </h3>
          {isAnalysing && (
            <span className="text-xs text-indigo-600 font-medium animate-pulse">
              Analysing audio…
            </span>
          )}
        </div>

        {isAnalysing && !assessment?.audioChecks.length ? (
          <div className="py-8 text-center text-sm text-gray-500 italic bg-gray-50 rounded-xl border border-gray-100 animate-pulse">
            Measuring loudness, true peak, boundary silence and clipping…
          </div>
        ) : assessment?.audioChecks.length ? (
          <div className="space-y-4">
            {assessment.audioChecks.map((check) => (
              <div
                key={check.id}
                className="p-3.5 bg-gray-50 rounded-xl border border-gray-100 flex flex-col space-y-1.5"
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold text-gray-800">
                    {check.label}
                  </span>
                  <div className="flex items-center space-x-2">
                    <span className="font-mono text-sm font-medium text-gray-900">
                      {check.displayValue}
                    </span>
                    {getStatusBadge(check.status)}
                  </div>
                </div>

                <p className="text-xs text-gray-600 leading-relaxed">
                  {check.message}
                </p>

                {/* Sparkline from sexy-sparklines */}
                {check.sparkline && (
                  <div className="pt-1.5">
                    <BulletSparkline
                      value={check.sparkline.value}
                      min={check.sparkline.min}
                      max={check.sparkline.max}
                      target={check.sparkline.target}
                      ranges={check.sparkline.ranges}
                      height={14}
                      theme="minimal"
                      aria-label={`${check.label} chart`}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : null}
      </div>

      {/* FILE DETAILS SECTION */}
      <div>
        <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase mb-3">
          File
        </h3>

        {assessment?.fileChecks.length ? (
          <div className="grid grid-cols-2 gap-2.5">
            {assessment.fileChecks.map((check) => (
              <div
                key={check.id}
                className="p-3 bg-gray-50 rounded-xl border border-gray-100 flex flex-col justify-between"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs text-gray-500">{check.label}</span>
                  {check.status !== "GOOD" && getStatusBadge(check.status)}
                </div>
                <div className="font-mono text-sm font-semibold text-gray-900">
                  {check.displayValue}
                </div>
                <div className="text-[11px] text-gray-500 mt-1 line-clamp-1">
                  {check.message}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-gray-500">
            {media.format} · {(media.inspection.sampleRate / 1000).toFixed(1)} kHz · {media.inspection.channels === 1 ? "Mono" : "Stereo"}
          </div>
        )}
      </div>
    </div>
  );
}
