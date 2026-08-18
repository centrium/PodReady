import { invoke } from "@tauri-apps/api/core";
import type { BatchPublishingJob } from "@podready/domain";
import { formatBatchPublishingDuration } from "@podready/domain";

interface BatchPublishingResultsProps {
  job: BatchPublishingJob;
  onClose: () => void;
  onOpenEpisode?: (episodeId: string) => void;
}

export function BatchPublishingResults({
  job,
  onClose,
  onOpenEpisode,
}: BatchPublishingResultsProps) {
  const isCancelled = job.status === "CANCELLED";
  const { complete, partial, failed, skipped, cancelled, elapsedSeconds } = job.summary;

  const handleOpenFolder = async () => {
    try {
      await invoke("open_path_in_file_manager_cmd", { path: job.destinationDirectory });
    } catch (err) {
      console.error("Failed to open destination directory:", err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
      <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-2xl w-full p-8 space-y-6">
        {/* Header Banner */}
        <div className="border-b border-gray-100 pb-4">
          <div className="flex items-center space-x-2">
            <span
              className={`w-3 h-3 rounded-full ${
                isCancelled
                  ? "bg-amber-500"
                  : failed > 0
                  ? "bg-amber-500"
                  : "bg-emerald-500"
              }`}
            />
            <h2 className="text-xl font-bold tracking-tight text-gray-900">
              {isCancelled ? "Publishing Stopped" : "Publishing Complete"}
            </h2>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-2 mt-1">
            <p className="text-xs font-medium text-gray-600">
              <strong className="text-gray-900">{complete}</strong> {complete === 1 ? "package created" : "packages created"}
              {partial > 0 && <span> · <strong className="text-amber-800">{partial}</strong> partial</span>}
              {failed > 0 && <span> · <strong className="text-rose-700">{failed}</strong> failed</span>}
              {skipped > 0 && <span> · <strong className="text-gray-500">{skipped}</strong> skipped</span>}
              {cancelled > 0 && <span> · <strong className="text-gray-400">{cancelled}</strong> cancelled</span>}
            </p>
            <span className="text-xs font-mono font-medium text-gray-500">
              Created in {formatBatchPublishingDuration(elapsedSeconds)}
            </span>
          </div>
        </div>

        {/* Destination Directory Card */}
        <div className="p-3.5 bg-gray-50 rounded-xl border border-gray-100 flex items-center justify-between">
          <div className="truncate pr-3">
            <span className="text-[10px] font-bold uppercase tracking-wider text-gray-400 block">
              Destination Folder
            </span>
            <span className="text-xs font-mono text-gray-800 truncate block mt-0.5" title={job.destinationDirectory}>
              {job.destinationDirectory}
            </span>
          </div>
          <button
            type="button"
            onClick={handleOpenFolder}
            className="px-3 py-1.5 text-xs font-semibold text-indigo-700 bg-indigo-50 hover:bg-indigo-100 rounded-lg transition-colors shrink-0 flex items-center space-x-1"
          >
            <span>Open Folder</span>
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
          </button>
        </div>

        {/* Episode Outcome List */}
        <div className="space-y-2 max-h-80 overflow-y-auto pr-1">
          {job.episodes.map((ep) => {
            const isPartial = ep.package?.transcriptError;
            const isComplete = ep.status === "COMPLETE";
            const isSkipped = ep.status === "SKIPPED";
            const isFailed = ep.status === "FAILED";
            const isCancelled = ep.status === "CANCELLED";

            return (
              <div
                key={ep.episodeId}
                className="p-3.5 bg-gray-50/80 hover:bg-gray-100/70 rounded-xl border border-gray-100 space-y-1 transition-colors"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2 truncate">
                    {isComplete && !isPartial && (
                      <span className="text-emerald-600 font-bold text-xs">✓</span>
                    )}
                    {isComplete && isPartial && (
                      <span className="text-amber-600 font-bold text-xs">⚠</span>
                    )}
                    {isFailed && (
                      <span className="text-rose-600 font-bold text-xs">✕</span>
                    )}
                    {isSkipped && (
                      <span className="text-gray-400 font-bold text-xs">○</span>
                    )}
                    {isCancelled && (
                      <span className="text-gray-400 font-bold text-xs">⊘</span>
                    )}
                    <span className="text-sm font-medium text-gray-900 truncate">
                      {ep.filename}
                    </span>
                  </div>

                  <div className="flex items-center space-x-2 shrink-0">
                    {typeof ep.elapsedSeconds === "number" && (
                      <span className="text-xs font-mono text-gray-400">
                        {ep.elapsedSeconds.toFixed(1)}s
                      </span>
                    )}
                    {onOpenEpisode && (
                      <button
                        type="button"
                        onClick={() => onOpenEpisode(ep.episodeId)}
                        className="text-[11px] font-medium text-indigo-600 hover:text-indigo-800"
                      >
                        View
                      </button>
                    )}
                  </div>
                </div>

                {/* Subtitle / Package details */}
                {ep.package && (
                  <p className="text-xs text-gray-500 font-mono pl-4 truncate">
                    📁 {ep.package.packageName}
                    {ep.package.audioFile && (
                      <span className="text-emerald-700 ml-2">· Audio verified</span>
                    )}
                    {ep.package.transcriptFile && (
                      <span className="text-indigo-700 ml-2">· Transcript</span>
                    )}
                    {isPartial && (
                      <span className="text-amber-800 ml-2 font-sans font-medium">
                        (Transcript: {ep.package.transcriptError})
                      </span>
                    )}
                  </p>
                )}

                {isSkipped && (
                  <p className="text-xs text-gray-500 pl-4">
                    Skipped · {ep.skipReason || "Source media unavailable on disk"}
                  </p>
                )}

                {isFailed && (
                  <p className="text-xs text-rose-700 pl-4">
                    Failed · {ep.error || "Publishing could not complete"}
                  </p>
                )}

                {isCancelled && (
                  <p className="text-xs text-gray-400 pl-4">
                    Cancelled
                  </p>
                )}
              </div>
            );
          })}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end space-x-3 pt-4 border-t border-gray-100">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
