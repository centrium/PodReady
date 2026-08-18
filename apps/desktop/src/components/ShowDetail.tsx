import { useState, useMemo, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ShowWithEpisodes,
  CatalogueEpisode,
} from "@podready/domain";
import { formatAudioDuration } from "@podready/domain";
import { CatalogueEpisodeModal } from "./CatalogueEpisodeModal";

interface ShowDetailProps {
  showId: string;
  onBack: () => void;
  onOpenInWorkspace: (sourcePath: string) => void;
  onShowDeleted: () => void;
}

type SortField = "ANALYSED_AT" | "FILENAME" | "DURATION" | "LOUDNESS" | "TRUE_PEAK" | "STATUS";
type FilterStatus = "ALL" | "READY" | "ATTENTION" | "NEEDS_ATTENTION" | "CHANGED" | "MISSING";

export function ShowDetail({
  showId,
  onBack,
  onOpenInWorkspace,
  onShowDeleted,
}: ShowDetailProps) {
  const [data, setData] = useState<ShowWithEpisodes | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Sorting & Filtering
  const [sortField, setSortField] = useState<SortField>("ANALYSED_AT");
  const [sortAsc, setSortAsc] = useState<boolean>(false); // default newest first
  const [filter, setFilter] = useState<FilterStatus>("ALL");

  // Editing Show Details
  const [isEditing, setIsEditing] = useState<boolean>(false);
  const [editName, setEditName] = useState<string>("");
  const [editDescription, setEditDescription] = useState<string>("");
  const [isSavingShow, setIsSavingShow] = useState<boolean>(false);

  // Deletion Confirmation Modal
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<boolean>(false);
  const [isDeletingShow, setIsDeletingShow] = useState<boolean>(false);

  // Episode Inspection Modal
  const [selectedEpisode, setSelectedEpisode] = useState<CatalogueEpisode | null>(null);

  const loadShowData = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const res = await invoke<ShowWithEpisodes>("get_show_cmd", { id: showId });
      setData(res);
      setEditName(res.show.name);
      setEditDescription(res.show.description || "");
    } catch (err: any) {
      console.error("Failed to load show:", err);
      setError(err.message || "Failed to load show details.");
    } finally {
      setIsLoading(false);
    }
  }, [showId]);

  useEffect(() => {
    loadShowData();
  }, [loadShowData]);

  const handleSaveShow = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editName.trim()) return;
    setIsSavingShow(true);
    try {
      const updated = await invoke<{ id: string; name: string; description?: string }>(
        "update_show_cmd",
        {
          input: {
            id: showId,
            name: editName.trim(),
            description: editDescription.trim() || undefined,
          },
        }
      );
      setData((prev) =>
        prev
          ? {
              ...prev,
              show: {
                ...prev.show,
                name: updated.name,
                description: updated.description,
              },
            }
          : null
      );
      setIsEditing(false);
    } catch (err: any) {
      console.error("Failed to update show:", err);
      setError(err.message || "Failed to save show changes.");
    } finally {
      setIsSavingShow(false);
    }
  };

  const handleDeleteShow = async () => {
    setIsDeletingShow(true);
    try {
      await invoke("delete_show_cmd", { id: showId });
      setShowDeleteConfirm(false);
      onShowDeleted();
    } catch (err: any) {
      console.error("Failed to delete show:", err);
      setError(err.message || "Failed to delete show.");
      setIsDeletingShow(false);
    }
  };

  const handleDeleteEpisode = async (episodeId: string) => {
    try {
      await invoke("delete_catalogue_episode_cmd", { id: episodeId });
      setSelectedEpisode(null);
      loadShowData();
    } catch (err: any) {
      console.error("Failed to delete episode:", err);
      setError(err.message || "Failed to delete episode from show.");
    }
  };

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

  const formatAnalysedDate = (iso: string) => {
    try {
      const d = new Date(iso);
      return d.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
      });
    } catch {
      return iso;
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case "READY":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-emerald-100 text-emerald-800">
            Ready
          </span>
        );
      case "ATTENTION":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-amber-100 text-amber-800">
            Attention
          </span>
        );
      case "NEEDS_ATTENTION":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-rose-100 text-rose-800">
            Needs Attention
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium bg-gray-100 text-gray-600">
            Unknown
          </span>
        );
    }
  };

  const filteredEpisodes = useMemo(() => {
    if (!data) return [];
    return data.episodes.filter((ep) => {
      if (filter === "ALL") return true;
      if (filter === "MISSING") return ep.sourceAvailability === "MISSING";
      if (filter === "CHANGED") return ep.sourceAvailability === "CHANGED";
      return ep.overallAssessmentStatus === filter;
    });
  }, [data, filter]);

  const sortedEpisodes = useMemo(() => {
    const list = [...filteredEpisodes];
    list.sort((a, b) => {
      let comp = 0;
      switch (sortField) {
        case "ANALYSED_AT":
          comp = new Date(a.analysedAt).getTime() - new Date(b.analysedAt).getTime();
          break;
        case "FILENAME":
          comp = a.filename.localeCompare(b.filename);
          break;
        case "DURATION":
          comp = a.durationSeconds - b.durationSeconds;
          break;
        case "LOUDNESS":
          comp = (a.integratedLoudnessLufs ?? -999) - (b.integratedLoudnessLufs ?? -999);
          break;
        case "TRUE_PEAK":
          comp = (a.truePeakDbtp ?? -999) - (b.truePeakDbtp ?? -999);
          break;
        case "STATUS": {
          const rank = (s: string) =>
            s === "NEEDS_ATTENTION" ? 3 : s === "ATTENTION" ? 2 : s === "READY" ? 1 : 0;
          comp = rank(a.overallAssessmentStatus) - rank(b.overallAssessmentStatus);
          break;
        }
      }
      return sortAsc ? comp : -comp;
    });
    return list;
  }, [filteredEpisodes, sortField, sortAsc]);

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
      setSortAsc(field === "ANALYSED_AT" ? false : true);
    }
  };

  if (isLoading) {
    return (
      <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl p-12 text-center shadow-xs">
        <p className="text-sm font-medium text-gray-500 animate-pulse">Loading show catalogue…</p>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl p-8 space-y-4 shadow-xs">
        <div className="p-4 bg-rose-50 border border-rose-200 rounded-xl text-rose-800 text-sm font-medium">
          {error || "Show not found."}
        </div>
        <button
          onClick={onBack}
          className="px-4 py-2 text-xs font-semibold text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
        >
          ← Back to Shows
        </button>
      </div>
    );
  }

  const { show, episodes } = data;
  const readyCount = episodes.filter((e) => e.overallAssessmentStatus === "READY").length;
  const attentionCount = episodes.filter((e) => e.overallAssessmentStatus === "ATTENTION").length;
  const needsAttentionCount = episodes.filter((e) => e.overallAssessmentStatus === "NEEDS_ATTENTION").length;
  const changedCount = episodes.filter((e) => e.sourceAvailability === "CHANGED").length;
  const missingCount = episodes.filter((e) => e.sourceAvailability === "MISSING").length;

  return (
    <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl shadow-sm p-8 flex flex-col space-y-6">
      {/* Top Navigation & Actions */}
      <div className="flex items-center justify-between border-b border-gray-100 pb-4">
        <button
          onClick={onBack}
          className="flex items-center text-xs font-semibold text-gray-600 hover:text-gray-900 transition-colors"
        >
          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 19l-7-7m0 0l7-7m-7 7h18" />
          </svg>
          Back to Shows
        </button>

        <div className="flex items-center space-x-2">
          {!isEditing && (
            <button
              onClick={() => setIsEditing(true)}
              className="px-3 py-1.5 text-xs font-semibold text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
            >
              Edit Details
            </button>
          )}
          <button
            onClick={() => setShowDeleteConfirm(true)}
            className="px-3 py-1.5 text-xs font-semibold text-rose-700 bg-rose-50 hover:bg-rose-100 rounded-lg transition-colors"
          >
            Delete Show
          </button>
        </div>
      </div>

      {/* Show Title & Info */}
      {isEditing ? (
        <form onSubmit={handleSaveShow} className="bg-gray-50 border border-gray-200 rounded-xl p-4 space-y-3">
          <div className="space-y-1">
            <label className="block text-xs font-bold text-gray-700">Show Name</label>
            <input
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              placeholder="Show name"
              className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2 focus:ring-1 focus:ring-indigo-500"
              required
            />
          </div>
          <div className="space-y-1">
            <label className="block text-xs font-bold text-gray-700">Description</label>
            <input
              type="text"
              value={editDescription}
              onChange={(e) => setEditDescription(e.target.value)}
              placeholder="Show description (optional)"
              className="w-full bg-white border border-gray-300 text-gray-900 text-sm rounded-lg p-2 focus:ring-1 focus:ring-indigo-500"
            />
          </div>
          <div className="flex items-center justify-end space-x-2 pt-1">
            <button
              type="button"
              onClick={() => setIsEditing(false)}
              className="px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-200 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSavingShow}
              className="px-3 py-1.5 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs"
            >
              {isSavingShow ? "Saving…" : "Save"}
            </button>
          </div>
        </form>
      ) : (
        <div className="space-y-1">
          <h2 className="text-2xl font-bold text-gray-900 tracking-tight">{show.name}</h2>
          {show.description && (
            <p className="text-sm text-gray-600 max-w-2xl">{show.description}</p>
          )}
          <p className="text-xs font-medium text-gray-400 pt-1">
            {episodes.length} {episodes.length === 1 ? "catalogued episode" : "catalogued episodes"}
          </p>
        </div>
      )}

      {/* KPI / Summary Filter Buttons */}
      <div className="flex flex-wrap items-center justify-between gap-3 pt-2">
        <div className="flex items-center space-x-1.5">
          <button
            onClick={() => setFilter("ALL")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "ALL"
                ? "bg-gray-900 text-white"
                : "bg-gray-100 text-gray-600 hover:bg-gray-200"
            }`}
          >
            All ({episodes.length})
          </button>
          <button
            onClick={() => setFilter("READY")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "READY"
                ? "bg-emerald-600 text-white"
                : "bg-emerald-50 text-emerald-700 hover:bg-emerald-100"
            }`}
          >
            Ready ({readyCount})
          </button>
          <button
            onClick={() => setFilter("ATTENTION")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "ATTENTION"
                ? "bg-amber-600 text-white"
                : "bg-amber-50 text-amber-700 hover:bg-amber-100"
            }`}
          >
            Attention ({attentionCount})
          </button>
          <button
            onClick={() => setFilter("NEEDS_ATTENTION")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "NEEDS_ATTENTION"
                ? "bg-rose-600 text-white"
                : "bg-rose-50 text-rose-700 hover:bg-rose-100"
            }`}
          >
            Needs Attention ({needsAttentionCount})
          </button>
          {changedCount > 0 && (
            <button
              onClick={() => setFilter("CHANGED")}
              className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
                filter === "CHANGED"
                  ? "bg-amber-700 text-white"
                  : "bg-amber-100 text-amber-900 hover:bg-amber-200"
              }`}
            >
              Changed Source ({changedCount})
            </button>
          )}
          {missingCount > 0 && (
            <button
              onClick={() => setFilter("MISSING")}
              className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
                filter === "MISSING"
                  ? "bg-rose-700 text-white"
                  : "bg-rose-50 text-rose-800 hover:bg-rose-100"
              }`}
            >
              Missing Source ({missingCount})
            </button>
          )}
        </div>

        {/* Sort selector */}
        <div className="flex items-center space-x-2 text-xs text-gray-500">
          <span>Sort:</span>
          <select
            value={sortField}
            onChange={(e) => handleSort(e.target.value as SortField)}
            className="bg-gray-50 border border-gray-200 text-gray-700 text-xs font-medium rounded-md px-2 py-1 focus:outline-none focus:ring-1 focus:ring-indigo-500"
          >
            <option value="ANALYSED_AT">Date Analysed</option>
            <option value="FILENAME">Filename</option>
            <option value="DURATION">Duration</option>
            <option value="LOUDNESS">Loudness (LUFS)</option>
            <option value="TRUE_PEAK">True Peak (dBTP)</option>
            <option value="STATUS">Status</option>
          </select>
        </div>
      </div>

      {/* Episode Table */}
      <div className="border border-gray-200 rounded-xl overflow-hidden shadow-xs">
        <div className="grid grid-cols-12 bg-gray-50 px-4 py-2.5 text-xs font-semibold text-gray-500 uppercase tracking-wider border-b border-gray-200">
          <div className="col-span-4">Episode</div>
          <div className="col-span-2 text-center">Duration</div>
          <div className="col-span-2 text-center">Loudness</div>
          <div className="col-span-2 text-center">True Peak</div>
          <div className="col-span-2 text-right">Status</div>
        </div>

        <div className="divide-y divide-gray-100 max-h-[480px] overflow-y-auto">
          {sortedEpisodes.map((ep) => {
            const isMissing = ep.sourceAvailability === "MISSING";
            const isChanged = ep.sourceAvailability === "CHANGED";
            return (
              <div
                key={ep.id}
                onClick={() => setSelectedEpisode(ep)}
                className="grid grid-cols-12 items-center px-4 py-3.5 hover:bg-indigo-50/30 transition-colors cursor-pointer"
              >
                {/* Filename & Analysed Date */}
                <div className="col-span-4 pr-2 truncate">
                  <div className="flex items-center space-x-1.5">
                    <span className="text-sm font-medium text-gray-900 truncate" title={ep.filename}>
                      {ep.filename}
                    </span>
                    {isChanged && (
                      <span
                        className="text-[10px] font-semibold text-amber-800 bg-amber-100 px-1.5 py-0.2 rounded shrink-0"
                        title="Source file was modified on disk since analysis"
                      >
                        Changed
                      </span>
                    )}
                    {isMissing && (
                      <span
                        className="text-[10px] font-semibold text-rose-700 bg-rose-100 px-1.5 py-0.2 rounded shrink-0"
                        title="Source file is not at original path"
                      >
                        Missing
                      </span>
                    )}

                  </div>
                  <span className="text-xs text-gray-400 block mt-0.5">
                    Analysed {formatAnalysedDate(ep.analysedAt)} · {ep.format}
                  </span>
                </div>

                {/* Duration */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-600">
                  {formatAudioDuration(ep.durationSeconds)}
                </div>

                {/* Loudness */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-700">
                  {formatLoudness(ep.integratedLoudnessLufs)}
                </div>

                {/* True Peak */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-700">
                  {formatPeak(ep.truePeakDbtp)}
                </div>

                {/* Status Badge */}
                <div className="col-span-2 text-right">
                  {getStatusBadge(ep.overallAssessmentStatus)}
                </div>
              </div>
            );
          })}

          {sortedEpisodes.length === 0 && (
            <div className="p-8 text-center text-sm text-gray-500">
              {episodes.length === 0
                ? "No episodes catalogued yet in this show. Analyse episodes and click 'Add to Show' to catalogue them."
                : "No episodes match the selected filter."}
            </div>
          )}
        </div>
      </div>

      {/* Catalogue Episode Inspection Modal */}
      {selectedEpisode && (
        <CatalogueEpisodeModal
          episode={selectedEpisode}
          isOpen={true}
          onClose={() => setSelectedEpisode(null)}
          onOpenInWorkspace={onOpenInWorkspace}
          onDeleteEpisode={handleDeleteEpisode}
        />
      )}

      {/* Delete Show Confirmation Dialog */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-4">
            <h3 className="text-base font-bold text-gray-900">Delete Show Catalogue?</h3>
            <p className="text-xs text-gray-600 leading-relaxed">
              This will remove <strong className="text-gray-900">{show.name}</strong> and its {episodes.length} catalogued records from PodReady.
            </p>
            <div className="p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-900 flex items-center space-x-2">
              <span>🛡️</span>
              <span>
                <strong>Your media files are safe:</strong> Original audio files will NEVER be modified or deleted.
              </span>
            </div>
            <div className="flex items-center justify-end space-x-3 pt-2">
              <button
                type="button"
                onClick={() => setShowDeleteConfirm(false)}
                disabled={isDeletingShow}
                className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleDeleteShow}
                disabled={isDeletingShow}
                className="px-4 py-2 text-xs font-bold text-white bg-rose-600 hover:bg-rose-700 rounded-lg transition-colors shadow-xs disabled:opacity-50"
              >
                {isDeletingShow ? "Deleting…" : "Delete Catalogue Record"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
