import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ShowSummary, Show } from "@podready/domain";

interface ShowLibraryProps {
  onSelectShow: (showId: string) => void;
}

export function ShowLibrary({ onSelectShow }: ShowLibraryProps) {
  const [shows, setShows] = useState<ShowSummary[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // New Show Form Modal / Inline
  const [isCreating, setIsCreating] = useState<boolean>(false);
  const [newShowName, setNewShowName] = useState<string>("");
  const [newShowDesc, setNewShowDesc] = useState<string>("");
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);

  useEffect(() => {
    loadShows();
  }, []);

  const loadShows = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const list = await invoke<ShowSummary[]>("get_shows_cmd");
      setShows(list);
    } catch (err: any) {
      console.error("Failed to load shows:", err);
      setError(err.message || "Failed to load shows.");
    } finally {
      setIsLoading(false);
    }
  };

  const handleCreateShow = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newShowName.trim()) return;

    setIsSubmitting(true);
    setError(null);
    try {
      const created = await invoke<Show>("create_show_cmd", {
        input: {
          name: newShowName.trim(),
          description: newShowDesc.trim() || undefined,
        },
      });
      setNewShowName("");
      setNewShowDesc("");
      setIsCreating(false);
      await loadShows();
      onSelectShow(created.id);
    } catch (err: any) {
      console.error("Failed to create show:", err);
      setError(err.message || "Failed to create show.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const formatLastAnalysed = (isoStr?: string) => {
    if (!isoStr) return "Never";
    try {
      const d = new Date(isoStr);
      return d.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
      });
    } catch {
      return isoStr;
    }
  };

  return (
    <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl shadow-sm p-8 flex flex-col space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between border-b border-gray-100 pb-6 gap-4">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-gray-900">Show Library</h2>
          <p className="text-sm font-medium text-gray-500 mt-1">
            Organise and track your podcast catalogue over time.
          </p>
        </div>

        <button
          onClick={() => setIsCreating(true)}
          className="self-start sm:self-auto px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold rounded-xl transition-colors shadow-sm flex items-center space-x-1.5"
        >
          <span>+ New Show</span>
        </button>
      </div>

      {error && (
        <div className="p-4 bg-rose-50 border border-rose-200 rounded-xl text-xs text-rose-800 font-medium">
          {error}
        </div>
      )}

      {/* Loading state */}
      {isLoading && (
        <div className="py-12 text-center text-sm text-gray-400 animate-pulse">
          Loading your shows…
        </div>
      )}

      {/* Shows Grid / List */}
      {!isLoading && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {shows.map((show) => (
            <div
              key={show.id}
              onClick={() => onSelectShow(show.id)}
              className="p-5 rounded-2xl border border-gray-200 hover:border-indigo-300 hover:shadow-md transition-all cursor-pointer bg-white group flex flex-col justify-between space-y-4"
            >
              <div className="space-y-1.5">
                <div className="flex items-start justify-between">
                  <h3 className="text-base font-bold text-gray-900 group-hover:text-indigo-600 transition-colors truncate max-w-[80%]">
                    {show.name}
                  </h3>
                  <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-gray-100 text-gray-700">
                    {show.episodeCount} {show.episodeCount === 1 ? "ep" : "eps"}
                  </span>
                </div>
                {show.description ? (
                  <p className="text-xs text-gray-500 line-clamp-2 leading-relaxed">
                    {show.description}
                  </p>
                ) : (
                  <p className="text-xs text-gray-400 italic">No description</p>
                )}
              </div>

              <div className="pt-3 border-t border-gray-100 flex items-center justify-between text-[11px] text-gray-400">
                <span>Last analysed: {formatLastAnalysed(show.lastAnalysedAt)}</span>
                <span className="font-semibold text-indigo-600 group-hover:translate-x-0.5 transition-transform">
                  Open →
                </span>
              </div>
            </div>
          ))}

          {shows.length === 0 && (
            <div className="col-span-full py-16 text-center rounded-2xl border-2 border-dashed border-gray-200 p-8 space-y-3">
              <span className="text-3xl block">🎙️</span>
              <h4 className="text-base font-bold text-gray-800">No Shows Created Yet</h4>
              <p className="text-xs text-gray-500 max-w-md mx-auto leading-relaxed">
                Create your first podcast show to start keeping a persistent catalogue of your analysed episodes.
              </p>
              <button
                onClick={() => setIsCreating(true)}
                className="mt-2 px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold rounded-xl transition-colors shadow-xs"
              >
                Create a Show
              </button>
            </div>
          )}
        </div>
      )}

      {/* Create Show Dialog */}
      {isCreating && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-4">
            <div className="flex items-center justify-between border-b border-gray-100 pb-3">
              <h3 className="text-base font-bold text-gray-900">Create New Show</h3>
              <button
                onClick={() => setIsCreating(false)}
                className="text-gray-400 hover:text-gray-600 transition-colors p-1"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <form onSubmit={handleCreateShow} className="space-y-4">
              <div className="space-y-1">
                <label className="block text-xs font-bold uppercase tracking-wider text-gray-500">
                  Show Name <span className="text-rose-500">*</span>
                </label>
                <input
                  type="text"
                  required
                  value={newShowName}
                  onChange={(e) => setNewShowName(e.target.value)}
                  placeholder="e.g. The Product Podcast"
                  className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2.5 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
                />
              </div>

              <div className="space-y-1">
                <label className="block text-xs font-bold uppercase tracking-wider text-gray-500">
                  Description (optional)
                </label>
                <textarea
                  rows={3}
                  value={newShowDesc}
                  onChange={(e) => setNewShowDesc(e.target.value)}
                  placeholder="Brief description of your podcast topic or format"
                  className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2.5 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500 resize-none"
                />
              </div>

              <div className="flex items-center justify-end space-x-3 pt-2">
                <button
                  type="button"
                  onClick={() => setIsCreating(false)}
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
                  {isSubmitting ? "Creating…" : "Create Show"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
