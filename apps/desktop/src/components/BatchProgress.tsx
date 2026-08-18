import { useEffect, useState } from "react";
import type { BatchAnalysisJob } from "@podready/domain";

interface BatchProgressProps {
  job: BatchAnalysisJob;
  onCancel: () => void;
  isCancelling?: boolean;
}

export function BatchProgress({ job, onCancel, isCancelling = false }: BatchProgressProps) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    const started = Date.now();
    const interval = setInterval(() => {
      setElapsed((Date.now() - started) / 1000);
    }, 100);

    return () => clearInterval(interval);
  }, []);

  const total = job.summary.total || job.episodes.length;
  const complete = job.summary.complete;
  const failed = job.summary.failed;
  const processed = complete + failed + job.summary.cancelled;
  const progressPercent = total > 0 ? Math.min(100, Math.round((processed / total) * 100)) : 0;

  return (
    <div className="w-full max-w-2xl bg-white border border-gray-200 rounded-2xl shadow-sm p-8 flex flex-col space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-gray-100 pb-4">
        <div>
          <h2 className="text-xl font-bold tracking-tight text-gray-900">Analysing episodes</h2>
          <p className="text-sm font-medium text-gray-500 mt-0.5">
            {processed} of {total} complete
          </p>
        </div>
        <div className="text-right">
          <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider block">
            Elapsed
          </span>
          <span className="text-base font-mono font-medium text-gray-700">
            {elapsed.toFixed(1)}s
          </span>
        </div>
      </div>

      {/* Progress Bar */}
      <div className="w-full bg-gray-100 rounded-full h-2 overflow-hidden">
        <div
          className="bg-blue-600 h-2 rounded-full transition-all duration-300 ease-out"
          style={{ width: `${progressPercent}%` }}
        />
      </div>

      {/* Episode Queue List */}
      <div className="space-y-2 max-h-80 overflow-y-auto pr-1">
        {job.episodes.map((ep) => {
          let statusBadge = (
            <span className="flex items-center text-xs font-medium text-gray-400">
              <span className="inline-block w-2 h-2 rounded-full border border-gray-300 mr-2" />
              Waiting
            </span>
          );

          if (ep.status === "INSPECTING" || ep.status === "ANALYSING" || ep.status === "ASSESSING") {
            statusBadge = (
              <span className="flex items-center text-xs font-medium text-blue-600">
                <svg
                  className="animate-spin -ml-0.5 mr-2 h-3.5 w-3.5 text-blue-600"
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
                {ep.status === "INSPECTING"
                  ? "Inspecting…"
                  : ep.status === "ANALYSING"
                  ? "Analysing…"
                  : "Assessing…"}
              </span>
            );
          } else if (ep.status === "COMPLETE") {
            statusBadge = (
              <span className="flex items-center text-xs font-medium text-emerald-600">
                <svg
                  className="w-3.5 h-3.5 mr-1.5 text-emerald-500"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2.5}
                    d="M5 13l4 4L19 7"
                  />
                </svg>
                Complete
              </span>
            );
          } else if (ep.status === "FAILED") {
            statusBadge = (
              <span className="flex items-center text-xs font-medium text-rose-600">
                <svg
                  className="w-3.5 h-3.5 mr-1.5 text-rose-500"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                  />
                </svg>
                Failed
              </span>
            );
          } else if (ep.status === "CANCELLED") {
            statusBadge = (
              <span className="flex items-center text-xs font-medium text-gray-400">
                <span className="mr-1.5">⊘</span>
                Cancelled
              </span>
            );
          }

          return (
            <div
              key={ep.id}
              className="flex items-center justify-between p-3 bg-gray-50 hover:bg-gray-100/70 rounded-lg border border-gray-100 transition-colors"
            >
              <div className="flex items-center space-x-3 truncate">
                <span className="text-sm font-medium text-gray-900 truncate">
                  {ep.filename}
                </span>
                {ep.error && (
                  <span className="text-xs text-rose-500 truncate" title={ep.error}>
                    ({ep.error})
                  </span>
                )}
              </div>
              <div className="flex-shrink-0 ml-4">{statusBadge}</div>
            </div>
          );
        })}
      </div>

      {/* Cancel Action */}
      <div className="flex justify-center pt-2">
        <button
          onClick={onCancel}
          disabled={isCancelling}
          className="px-6 py-2 bg-white text-gray-700 text-sm font-medium border border-gray-300 rounded-lg hover:border-rose-300 hover:text-rose-600 hover:bg-rose-50/50 transition-colors shadow-sm disabled:opacity-50"
        >
          {isCancelling ? "Cancelling…" : "Cancel Analysis"}
        </button>
      </div>
    </div>
  );
}
