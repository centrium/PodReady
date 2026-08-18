import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ShowSummary,
  Show,
  MediaSource,
  BatchAnalysisJob,
  AddEpisodeOutcome,
  AddBatchEpisodesResult,
} from "@podready/domain";

interface AddToShowModalProps {
  isOpen: boolean;
  onClose: () => void;
  singleMedia?: MediaSource | null;
  batchJob?: BatchAnalysisJob | null;
  onShowAdded?: (showId: string) => void;
}

export function AddToShowModal({
  isOpen,
  onClose,
  singleMedia,
  batchJob,
  onShowAdded,
}: AddToShowModalProps) {
  const [shows, setShows] = useState<ShowSummary[]>([]);
  const [isLoadingShows, setIsLoadingShows] = useState<boolean>(true);
  const [selectedShowId, setSelectedShowId] = useState<string>("NEW");
  const [newShowName, setNewShowName] = useState<string>("");
  const [newShowDescription, setNewShowDescription] = useState<string>("");
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [resultMessage, setResultMessage] = useState<string | null>(null);
  const [addedShowId, setAddedShowId] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      loadShows();
      setResultMessage(null);
      setError(null);
      setAddedShowId(null);
    }
  }, [isOpen]);

  const loadShows = async () => {
    setIsLoadingShows(true);
    setError(null);
    try {
      const fetched = await invoke<ShowSummary[]>("get_shows_cmd");
      setShows(fetched);
      if (fetched.length > 0) {
        setSelectedShowId(fetched[0].id);
      } else {
        setSelectedShowId("NEW");
      }
    } catch (err: any) {
      console.error("Failed to load shows:", err);
      setError("Could not load your shows.");
    } finally {
      setIsLoadingShows(false);
    }
  };

  if (!isOpen) return null;

  const validBatchCount =
    batchJob?.episodes.filter(
      (ep) => ep.status === "COMPLETE" && ep.assessment !== undefined
    ).length ?? 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsSubmitting(true);

    try {
      let targetShowId = selectedShowId;

      // Create new show if requested
      if (selectedShowId === "NEW") {
        if (!newShowName.trim()) {
          setError("Please enter a name for the new show.");
          setIsSubmitting(false);
          return;
        }

        const created = await invoke<Show>("create_show_cmd", {
          input: {
            name: newShowName.trim(),
            description: newShowDescription.trim() || undefined,
          },
        });
        targetShowId = created.id;
      }

      // Add single media episode
      if (singleMedia) {
        const outcome = await invoke<AddEpisodeOutcome>("add_episode_to_show_cmd", {
          showId: targetShowId,
          media: singleMedia,
        });

        if (outcome.status === "ADDED") {
          setResultMessage(`Added "${outcome.filename}" to show.`);
        } else if (outcome.status === "UPDATED") {
          setResultMessage(`Updated catalogue entry for "${outcome.filename}".`);
        } else {
          setResultMessage(`"${outcome.filename}" is already in this show.`);
        }
      } else if (batchJob) {
        // Add batch episodes
        const batchRes = await invoke<AddBatchEpisodesResult>(
          "add_batch_episodes_to_show_cmd",
          {
            showId: targetShowId,
            jobId: batchJob.id,
          }
        );

        const parts = [];
        if (batchRes.added > 0) parts.push(`${batchRes.added} added`);
        if (batchRes.updated > 0) parts.push(`${batchRes.updated} updated`);
        if (batchRes.alreadyExists > 0)
          parts.push(`${batchRes.alreadyExists} already existed`);
        if (batchRes.skippedFailed > 0)
          parts.push(`${batchRes.skippedFailed} failed/incomplete skipped`);

        setResultMessage(
          `Successfully processed batch for "${batchRes.showName}": ${parts.join(", ")}.`
        );
      }

      setAddedShowId(targetShowId);
    } catch (err: any) {
      console.error("Failed adding to show:", err);
      setError(err.message || "Failed to add episode(s) to show.");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
      <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-5 animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between border-b border-gray-100 pb-3">
          <h3 className="text-base font-bold text-gray-900">
            {singleMedia ? "Add Episode to Show" : "Add Batch to Show"}
          </h3>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors p-1 rounded-md"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {resultMessage ? (
          <div className="space-y-4 py-2">
            <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl text-emerald-900 text-sm space-y-1">
              <div className="flex items-center space-x-2 font-bold text-emerald-800">
                <span>✓</span>
                <span>Catalogue Updated</span>
              </div>
              <p className="text-xs text-emerald-700 leading-relaxed">{resultMessage}</p>
            </div>

            <div className="flex items-center justify-end space-x-3 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
              >
                Close
              </button>
              {addedShowId && onShowAdded && (
                <button
                  type="button"
                  onClick={() => {
                    onShowAdded(addedShowId);
                    onClose();
                  }}
                  className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs"
                >
                  View Show Library →
                </button>
              )}
            </div>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            {singleMedia ? (
              <div className="p-3 bg-gray-50 rounded-xl border border-gray-100 text-xs">
                <span className="font-semibold text-gray-700 block truncate">
                  {singleMedia.filename}
                </span>
                <span className="text-gray-500">
                  {singleMedia.format} · {singleMedia.codec}
                </span>
              </div>
            ) : (
              <div className="p-3 bg-gray-50 rounded-xl border border-gray-100 text-xs text-gray-600">
                Adding <strong className="text-gray-900">{validBatchCount}</strong> completed episodes to catalogue.
              </div>
            )}

            {error && (
              <div className="p-3 bg-rose-50 border border-rose-200 rounded-lg text-xs text-rose-800 font-medium">
                {error}
              </div>
            )}

            <div className="space-y-1.5">
              <label className="block text-xs font-bold uppercase tracking-wider text-gray-500">
                Destination Show
              </label>
              {isLoadingShows ? (
                <div className="text-xs text-gray-400 py-2 animate-pulse">Loading shows…</div>
              ) : (
                <select
                  value={selectedShowId}
                  onChange={(e) => setSelectedShowId(e.target.value)}
                  className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2.5 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
                >
                  {shows.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.episodeCount} {s.episodeCount === 1 ? "episode" : "episodes"})
                    </option>
                  ))}
                  <option value="NEW">+ Create New Show…</option>
                </select>
              )}
            </div>

            {selectedShowId === "NEW" && (
              <div className="space-y-3 pt-2 border-t border-gray-100">
                <div className="space-y-1">
                  <label className="block text-xs font-semibold text-gray-700">
                    New Show Name <span className="text-rose-500">*</span>
                  </label>
                  <input
                    type="text"
                    required
                    value={newShowName}
                    onChange={(e) => setNewShowName(e.target.value)}
                    placeholder="e.g. The Product Podcast"
                    className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                </div>

                <div className="space-y-1">
                  <label className="block text-xs font-semibold text-gray-700">
                    Description (optional)
                  </label>
                  <input
                    type="text"
                    value={newShowDescription}
                    onChange={(e) => setNewShowDescription(e.target.value)}
                    placeholder="e.g. Weekly conversations on product engineering"
                    className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                </div>
              </div>
            )}

            <div className="flex items-center justify-end space-x-3 pt-3 border-t border-gray-100">
              <button
                type="button"
                onClick={onClose}
                disabled={isSubmitting}
                className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting}
                className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs disabled:opacity-50"
              >
                {isSubmitting ? "Adding…" : "Add to Show"}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
