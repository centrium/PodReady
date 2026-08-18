import { useEffect, useState } from "react";
import type { BatchPublishingJob, PublishingEpisodeStage, PublishingEpisodeStatus } from "@podready/domain";
import { formatBatchPublishingDuration } from "@podready/domain";

interface BatchPublishingProgressProps {
  job: BatchPublishingJob;
  onCancel: () => void;
  isCancelling?: boolean;
}

export function BatchPublishingProgress({
  job,
  onCancel,
  isCancelling = false,
}: BatchPublishingProgressProps) {
  const [elapsed, setElapsed] = useState<number>(0);

  useEffect(() => {
    const started = Date.now();
    const interval = setInterval(() => {
      setElapsed((Date.now() - started) / 1000);
    }, 100);

    return () => clearInterval(interval);
  }, []);

  const total = job.summary.total || job.episodes.length;
  const processed =
    job.summary.complete +
    job.summary.partial +
    job.summary.failed +
    job.summary.cancelled +
    job.summary.skipped;

  const progressPercent = total > 0 ? Math.min(100, Math.round((processed / total) * 100)) : 0;

  const getStageLabel = (stage?: PublishingEpisodeStage, status?: PublishingEpisodeStatus) => {
    if (status === "PREPARING" || stage === "PREPARING") return "Preparing…";
    if (status === "PROCESSING" || stage === "PROCESSING") return "Processing audio…";
    if (status === "VERIFYING" || stage === "VERIFYING") return "Verifying output…";
    if (status === "EXPORTING" || stage === "EXPORTING") return "Exporting MP3…";
    if (status === "TRANSCRIBING" || stage === "TRANSCRIBING") return "Transcribing speech…";
    if (status === "PACKAGING" || stage === "PACKAGING") return "Packaging…";
    return "In progress…";
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
      <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-2xl w-full p-8 space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-100 pb-4">
          <div>
            <h2 className="text-xl font-bold tracking-tight text-gray-900">
              {total === 1 ? "Publishing episode" : `Publishing ${total} episodes`}
            </h2>
            <p className="text-sm font-medium text-gray-500 mt-0.5">
              {processed} of {total} processed
            </p>
          </div>
          <div className="text-right">
            <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider block">
              Elapsed
            </span>
            <span className="text-base font-mono font-medium text-gray-700">
              {formatBatchPublishingDuration(elapsed)}
            </span>
          </div>
        </div>

        {/* Progress Bar */}
        <div className="w-full bg-gray-100 rounded-full h-2 overflow-hidden">
          <div
            className="bg-indigo-600 h-2 rounded-full transition-all duration-300 ease-out"
            style={{ width: `${progressPercent}%` }}
          />
        </div>

        {/* Episode Queue List */}
        <div className="space-y-2 max-h-80 overflow-y-auto pr-1">
          {job.episodes.map((ep) => {
            const isActive =
              ep.status === "PREPARING" ||
              ep.status === "PROCESSING" ||
              ep.status === "VERIFYING" ||
              ep.status === "EXPORTING" ||
              ep.status === "TRANSCRIBING" ||
              ep.status === "PACKAGING";

            let statusBadge = (
              <span className="flex items-center text-xs font-medium text-gray-400">
                <span className="inline-block w-2 h-2 rounded-full border border-gray-300 mr-2" />
                Waiting
              </span>
            );

            if (isActive) {
              statusBadge = (
                <span className="flex items-center text-xs font-medium text-indigo-600">
                  <svg
                    className="animate-spin -ml-0.5 mr-2 h-3.5 w-3.5 text-indigo-600"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    />
                  </svg>
                  {getStageLabel(ep.stage, ep.status)}
                </span>
              );
            } else if (ep.status === "COMPLETE") {
              const isPartial = ep.package?.transcriptError;
              statusBadge = isPartial ? (
                <span className="flex items-center text-xs font-medium text-amber-600" title={`Audio ready, transcript failed: ${ep.package?.transcriptError}`}>
                  <svg className="w-3.5 h-3.5 mr-1 text-amber-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                  </svg>
                  Audio ready (partial)
                </span>
              ) : (
                <span className="flex items-center text-xs font-medium text-emerald-600">
                  <svg className="w-3.5 h-3.5 mr-1 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                  </svg>
                  Complete
                </span>
              );
            } else if (ep.status === "SKIPPED") {
              statusBadge = (
                <span className="flex items-center text-xs font-medium text-rose-600" title={ep.skipReason || "Source unavailable"}>
                  <span className="inline-block w-2 h-2 rounded-full border border-rose-400 mr-2" />
                  Skipped (Source unavailable)
                </span>
              );
            } else if (ep.status === "FAILED") {
              statusBadge = (
                <span className="flex items-center text-xs font-medium text-rose-600" title={ep.error || "Failed"}>
                  <svg className="w-3.5 h-3.5 mr-1 text-rose-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                  Failed
                </span>
              );
            } else if (ep.status === "CANCELLED") {
              statusBadge = (
                <span className="flex items-center text-xs font-medium text-gray-400">
                  <span className="mr-1">⊘</span>
                  Cancelled
                </span>
              );
            }

            return (
              <div
                key={ep.episodeId}
                className="flex items-center justify-between p-3 bg-gray-50 hover:bg-gray-100/70 rounded-xl border border-gray-100 transition-colors"
              >
                <div className="flex items-center space-x-3 truncate">
                  <span className="text-sm font-medium text-gray-900 truncate">
                    {ep.filename}
                  </span>
                  {ep.reanalysed && (
                    <span className="text-[10px] font-semibold text-amber-700 bg-amber-100 px-1.5 py-0.2 rounded shrink-0">
                      Re-analysed
                    </span>
                  )}
                </div>
                <div className="flex items-center space-x-3 shrink-0">
                  {statusBadge}
                  {typeof ep.elapsedSeconds === "number" && (
                    <span className="text-xs font-mono text-gray-400">
                      {ep.elapsedSeconds.toFixed(1)}s
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {/* Footer / Cancel Button */}
        <div className="flex items-center justify-between pt-4 border-t border-gray-100">
          <p className="text-xs text-gray-400">
            Publishing sequentially (concurrency 1) for safe audio processing and transcription.
          </p>
          <button
            type="button"
            onClick={onCancel}
            disabled={isCancelling}
            className="px-4 py-2 text-xs font-semibold text-rose-700 bg-rose-50 hover:bg-rose-100 rounded-lg transition-colors disabled:opacity-50"
          >
            {isCancelling ? "Cancelling…" : "Cancel Publishing"}
          </button>
        </div>
      </div>
    </div>
  );
}
