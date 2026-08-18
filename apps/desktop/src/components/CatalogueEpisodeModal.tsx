import type {
  CatalogueEpisode,
  AssessmentStatus,
  OverallStatus,
} from "@podready/domain";
import { formatAudioDuration } from "@podready/domain";
import { BulletSparkline } from "sexy-sparklines";

interface CatalogueEpisodeModalProps {
  episode: CatalogueEpisode | null;
  isOpen: boolean;
  onClose: () => void;
  onOpenInWorkspace?: (sourcePath: string) => void;
  onDeleteEpisode?: (episodeId: string) => void;
}

export function CatalogueEpisodeModal({
  episode,
  isOpen,
  onClose,
  onOpenInWorkspace,
  onDeleteEpisode,
}: CatalogueEpisodeModalProps) {
  if (!isOpen || !episode) return null;

  const formatLoudness = (val?: number | null) => {
    if (val === null || val === undefined || isNaN(val)) return "—";
    const sign = val < 0 ? "−" : "";
    return `${sign}${Math.abs(val).toFixed(1)} LUFS`;
  };

  const formatPeak = (val?: number | null) => {
    if (val === null || val === undefined || isNaN(val)) return "—";
    const sign = val < 0 ? "−" : "";
    return `${sign}${Math.abs(val).toFixed(1)} dBTP`;
  };

  const formatAnalysedDate = (isoStr: string) => {
    try {
      const d = new Date(isoStr);
      return d.toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return isoStr;
    }
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
      default:
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-600">
            Unknown
          </span>
        );
    }
  };

  const getOverallStatusBanner = (overallStatus: OverallStatus, summary?: string) => {
    switch (overallStatus) {
      case "READY":
        return (
          <div className="p-3.5 bg-emerald-50 border border-emerald-200 rounded-xl flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500" />
              <span className="text-xs font-bold text-emerald-900 uppercase tracking-wider">
                Ready
              </span>
            </div>
            <span className="text-xs font-medium text-emerald-700">
              {summary || "Ready for publication"}
            </span>
          </div>
        );
      case "ATTENTION":
        return (
          <div className="p-3.5 bg-amber-50 border border-amber-200 rounded-xl flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <span className="w-2.5 h-2.5 rounded-full bg-amber-500" />
              <span className="text-xs font-bold text-amber-900 uppercase tracking-wider">
                Attention
              </span>
            </div>
            <span className="text-xs font-semibold text-amber-800">
              {summary || "Review recommended"}
            </span>
          </div>
        );
      case "NEEDS_ATTENTION":
        return (
          <div className="p-3.5 bg-rose-50 border border-rose-200 rounded-xl flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <span className="w-2.5 h-2.5 rounded-full bg-rose-500" />
              <span className="text-xs font-bold text-rose-900 uppercase tracking-wider">
                Needs Attention
              </span>
            </div>
            <span className="text-xs font-semibold text-rose-800">
              {summary || "Significant issues detected"}
            </span>
          </div>
        );
    }
  };

  const assessment = episode.assessment;
  const isMissing = episode.sourceAvailability === "MISSING";
  const isChanged = episode.sourceAvailability === "CHANGED";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 overflow-y-auto">
      <div className="bg-white rounded-2xl border border-gray-200 shadow-2xl max-w-2xl w-full p-6 space-y-6 my-8 max-h-[90vh] overflow-y-auto">
        {/* Top Header */}
        <div className="flex items-start justify-between border-b border-gray-100 pb-4">
          <div className="space-y-1 max-w-[80%]">
            <div className="flex items-center space-x-2">
              <span className="text-[11px] font-bold uppercase tracking-wider text-indigo-600 bg-indigo-50 px-2 py-0.5 rounded">
                Stored Analysis
              </span>
              {isChanged && (
                <span className="text-[11px] font-bold uppercase tracking-wider text-amber-800 bg-amber-100 px-2 py-0.5 rounded">
                  Source Changed
                </span>
              )}
              {isMissing && (
                <span className="text-[11px] font-bold uppercase tracking-wider text-rose-800 bg-rose-100 px-2 py-0.5 rounded">
                  Source Missing
                </span>
              )}
              <span className="text-xs text-gray-400">
                Analysed {formatAnalysedDate(episode.analysedAt)}
              </span>
            </div>
            <h2 className="text-xl font-bold text-gray-900 truncate" title={episode.filename}>
              {episode.filename}
            </h2>
            <p className="text-xs text-gray-500 font-mono truncate" title={episode.sourcePath}>
              {episode.sourcePath}
            </p>
          </div>

          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors p-1.5 rounded-lg hover:bg-gray-100"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Source Availability Notices */}
        {isMissing && (
          <div className="p-3.5 bg-amber-50 border border-amber-200 rounded-xl flex items-center space-x-3 text-xs text-amber-900">
            <span className="text-base">⚠️</span>
            <div>
              <strong className="font-semibold block">Source File Missing</strong>
              <span>
                The original file was not found at its path. The saved analysis is preserved, but you cannot open it in the workspace until the file is restored.
              </span>
            </div>
          </div>
        )}

        {isChanged && (
          <div className="p-3.5 bg-amber-50 border border-amber-300 rounded-xl flex items-center space-x-3 text-xs text-amber-900">
            <span className="text-base">⚠️</span>
            <div>
              <strong className="font-semibold block">Source Changed</strong>
              <span>
                Source changed — stored analysis may be out of date. You can open the file in the workspace to run a fresh analysis.
              </span>
            </div>
          </div>
        )}


        {/* Overall Status Banner */}
        {getOverallStatusBanner(
          episode.overallAssessmentStatus as OverallStatus,
          assessment?.summary
        )}

        {/* Metrics Grid */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <div className="p-3 bg-gray-50 rounded-xl border border-gray-100">
            <span className="text-[10px] font-bold uppercase tracking-wider text-gray-400 block">
              Duration
            </span>
            <span className="text-base font-semibold font-mono text-gray-900 mt-0.5 block">
              {formatAudioDuration(episode.durationSeconds)}
            </span>
          </div>

          <div className="p-3 bg-gray-50 rounded-xl border border-gray-100">
            <span className="text-[10px] font-bold uppercase tracking-wider text-gray-400 block">
              Loudness
            </span>
            <span className="text-base font-semibold font-mono text-gray-900 mt-0.5 block">
              {formatLoudness(episode.integratedLoudnessLufs)}
            </span>
          </div>

          <div className="p-3 bg-gray-50 rounded-xl border border-gray-100">
            <span className="text-[10px] font-bold uppercase tracking-wider text-gray-400 block">
              True Peak
            </span>
            <span className="text-base font-semibold font-mono text-gray-900 mt-0.5 block">
              {formatPeak(episode.truePeakDbtp)}
            </span>
          </div>

          <div className="p-3 bg-gray-50 rounded-xl border border-gray-100">
            <span className="text-[10px] font-bold uppercase tracking-wider text-gray-400 block">
              Format
            </span>
            <span className="text-base font-semibold text-gray-900 mt-0.5 block">
              {episode.format} {episode.codec ? `· ${episode.codec}` : ""}
            </span>
          </div>
        </div>

        {/* Audio Checks with Sparklines (from saved assessment) */}
        {assessment?.audioChecks && assessment.audioChecks.length > 0 && (
          <div className="space-y-3">
            <h4 className="text-xs font-bold uppercase tracking-wider text-gray-400">
              Audio Checks ({assessment.profileName})
            </h4>
            <div className="space-y-2.5">
              {assessment.audioChecks.map((check) => (
                <div
                  key={check.id}
                  className="p-3 bg-gray-50 rounded-xl border border-gray-100 space-y-1.5"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-semibold text-gray-800">
                      {check.label}
                    </span>
                    <div className="flex items-center space-x-2">
                      <span className="font-mono text-xs font-medium text-gray-900">
                        {check.displayValue}
                      </span>
                      {getStatusBadge(check.status)}
                    </div>
                  </div>
                  <p className="text-xs text-gray-600">{check.message}</p>
                  {check.sparkline && (
                    <div className="pt-1">
                      <BulletSparkline
                        value={check.sparkline.value}
                        min={check.sparkline.min}
                        max={check.sparkline.max}
                        target={check.sparkline.target}
                        ranges={check.sparkline.ranges}
                        height={12}
                        theme="minimal"
                        aria-label={`${check.label} chart`}
                      />
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Action Footer */}
        <div className="flex items-center justify-between pt-4 border-t border-gray-100">
          {onDeleteEpisode && (
            <button
              onClick={() => onDeleteEpisode(episode.id)}
              className="text-xs font-semibold text-rose-600 hover:text-rose-700 transition-colors px-3 py-2 rounded-lg hover:bg-rose-50"
            >
              Remove from Show
            </button>
          )}

          <div className="flex items-center space-x-3 ml-auto">
            <button
              onClick={onClose}
              className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
            >
              Close
            </button>
            {!isMissing && onOpenInWorkspace && (
              <button
                onClick={() => onOpenInWorkspace(episode.sourcePath)}
                className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs flex items-center space-x-1.5"
              >
                <span>{isChanged ? "Re-analyse in Workspace" : "Open in Workspace"}</span>
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
                </svg>
              </button>
            )}

          </div>
        </div>
      </div>
    </div>
  );
}
