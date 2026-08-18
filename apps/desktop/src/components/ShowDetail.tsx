import { useState, useMemo, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ShowWithEpisodes,
  CatalogueEpisode,
  ShowBaseline,
  BatchPublishingJob,
  BatchPublishingProgressPayload,
} from "@podready/domain";
import { formatAudioDuration } from "@podready/domain";
import { CatalogueEpisodeModal } from "./CatalogueEpisodeModal";
import { ShowBaselineSection } from "./ShowBaselineSection";
import { BatchPublishingPreflightModal } from "./BatchPublishingPreflightModal";
import { BatchPublishingProgress } from "./BatchPublishingProgress";
import { BatchPublishingResults } from "./BatchPublishingResults";

interface ShowDetailProps {
  showId: string;
  onBack: () => void;
  onOpenInWorkspace: (sourcePath: string) => void;
  onShowDeleted: () => void;
}

type SortField = "ANALYSED_AT" | "FILENAME" | "DURATION" | "LOUDNESS" | "TRUE_PEAK" | "STATUS";
type FilterStatus = "ALL" | "READY" | "ATTENTION" | "NEEDS_ATTENTION" | "UNKNOWN" | "CHANGED" | "MISSING";

export function ShowDetail({
  showId,
  onBack,
  onOpenInWorkspace,
  onShowDeleted,
}: ShowDetailProps) {
  const [data, setData] = useState<ShowWithEpisodes | null>(null);
  const [baseline, setBaseline] = useState<ShowBaseline | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Sorting & Filtering
  const [sortField, setSortField] = useState<SortField>("ANALYSED_AT");
  const [sortAsc, setSortAsc] = useState<boolean>(false); // default newest first
  const [filter, setFilter] = useState<FilterStatus>("ALL");

  // Selection state for Stage 5E Batch Publishing
  const [selectedEpisodeIds, setSelectedEpisodeIds] = useState<Set<string>>(new Set());
  const [isPreflightOpen, setIsPreflightOpen] = useState<boolean>(false);
  const [publishingJob, setPublishingJob] = useState<BatchPublishingJob | null>(null);
  const [isPublishing, setIsPublishing] = useState<boolean>(false);
  const [isCancellingPublishing, setIsCancellingPublishing] = useState<boolean>(false);
  const [showPublishingResults, setShowPublishingResults] = useState<boolean>(false);

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
      const [res, baselineRes] = await Promise.all([
        invoke<ShowWithEpisodes>("get_show_cmd", { id: showId }),
        invoke<ShowBaseline>("get_show_baseline_cmd", { id: showId }),
      ]);
      setData(res);
      setBaseline(baselineRes);
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

  // Listen for batch publishing lifecycle events
  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;

    const setupPublishingListeners = async () => {
      unlistenProgress = await listen<BatchPublishingProgressPayload>(
        "batch-publishing-progress",
        (event) => {
          const payload = event.payload;
          setPublishingJob((prev) => {
            if (!prev || prev.id !== payload.jobId) return prev;
            const updatedEpisodes = prev.episodes.map((ep) =>
              ep.episodeId === payload.episodeId ? payload.episode : ep
            );
            return {
              ...prev,
              episodes: updatedEpisodes,
              summary: payload.summary,
            };
          });
        }
      );

      unlistenComplete = await listen<BatchPublishingProgressPayload>(
        "batch-publishing-complete",
        (event) => {
          const payload = event.payload;
          setPublishingJob((prev) => {
            if (!prev || prev.id !== payload.jobId) return prev;
            const updatedEpisodes = prev.episodes.map((ep) =>
              ep.episodeId === payload.episodeId ? payload.episode : ep
            );
            return {
              ...prev,
              status: "COMPLETE",
              episodes: updatedEpisodes,
              summary: payload.summary,
            };
          });
          setIsPublishing(false);
          setIsCancellingPublishing(false);
          setShowPublishingResults(true);
          setSelectedEpisodeIds(new Set());
          loadShowData();
        }
      );
    };

    setupPublishingListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
    };
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
      setSelectedEpisodeIds((prev) => {
        const next = new Set(prev);
        next.delete(episodeId);
        return next;
      });
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
      if (filter === "UNKNOWN") return !["READY", "ATTENTION", "NEEDS_ATTENTION"].includes(ep.overallAssessmentStatus);
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

  // Selection handlers
  const handleToggleSelect = (episodeId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setSelectedEpisodeIds((prev) => {
      const next = new Set(prev);
      if (next.has(episodeId)) {
        next.delete(episodeId);
      } else {
        next.add(episodeId);
      }
      return next;
    });
  };

  const handleSelectAllVisible = () => {
    const allVisibleIds = new Set(sortedEpisodes.map((e) => e.id));
    setSelectedEpisodeIds(allVisibleIds);
  };

  const handleClearSelection = () => {
    setSelectedEpisodeIds(new Set());
  };

  const selectedEpisodesList = useMemo(() => {
    if (!data) return [];
    return data.episodes.filter((e) => selectedEpisodeIds.has(e.id));
  }, [data, selectedEpisodeIds]);

  const handleStartPublishing = async (destinationDirectory: string) => {
    if (!data) return;
    const episodeIds = Array.from(selectedEpisodeIds);
    if (episodeIds.length === 0) return;

    try {
      const job = await invoke<BatchPublishingJob>("start_batch_publishing_cmd", {
        showId: data.show.id,
        showName: data.show.name,
        episodeIds,
        destinationDirectory,
        options: null,
      });
      setPublishingJob(job);
      setIsPublishing(true);
      setIsPreflightOpen(false);
    } catch (err: any) {
      console.error("Failed to start batch publishing:", err);
      setError(err.message || "Failed to start batch publishing.");
      throw err;
    }
  };

  const handleCancelPublishing = async () => {
    if (!publishingJob) return;
    setIsCancellingPublishing(true);
    try {
      await invoke("cancel_batch_publishing_cmd", { jobId: publishingJob.id });
      const updated = await invoke<BatchPublishingJob>("get_batch_publishing_job_cmd", {
        jobId: publishingJob.id,
      });
      setPublishingJob(updated);
    } catch (err) {
      console.error("Failed to cancel publishing job:", err);
    } finally {
      setIsPublishing(false);
      setIsCancellingPublishing(false);
      setShowPublishingResults(true);
    }
  };

  const handlePublishSingleEpisode = (ep: CatalogueEpisode) => {
    setSelectedEpisodeIds(new Set([ep.id]));
    setIsPreflightOpen(true);
    setSelectedEpisode(null);
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
  const unknownCount = episodes.filter(
    (e) => !["READY", "ATTENTION", "NEEDS_ATTENTION"].includes(e.overallAssessmentStatus)
  ).length;
  const changedCount = episodes.filter((e) => e.sourceAvailability === "CHANGED").length;
  const missingCount = episodes.filter((e) => e.sourceAvailability === "MISSING").length;

  const allVisibleSelected =
    sortedEpisodes.length > 0 && sortedEpisodes.every((e) => selectedEpisodeIds.has(e.id));

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

      {/* Show Baseline Historical Characteristics */}
      <ShowBaselineSection baseline={baseline} />

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
          {unknownCount > 0 && (
            <button
              onClick={() => setFilter("UNKNOWN")}
              className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
                filter === "UNKNOWN"
                  ? "bg-gray-700 text-white"
                  : "bg-gray-100 text-gray-700 hover:bg-gray-200"
              }`}
            >
              Unknown ({unknownCount})
            </button>
          )}
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

      {/* Stage 5E: Selection Action Bar */}
      {selectedEpisodeIds.size > 0 && (
        <div className="flex items-center justify-between p-3.5 bg-indigo-50/70 border border-indigo-100 rounded-xl transition-all">
          <div className="flex items-center space-x-3 text-xs">
            <span className="font-bold text-indigo-950">
              {selectedEpisodeIds.size} {selectedEpisodeIds.size === 1 ? "episode selected" : "episodes selected"}
            </span>
            <button
              onClick={handleSelectAllVisible}
              className="font-semibold text-indigo-700 hover:text-indigo-900 underline"
            >
              Select All Visible ({sortedEpisodes.length})
            </button>
            <button
              onClick={handleClearSelection}
              className="font-medium text-gray-500 hover:text-gray-700"
            >
              Clear
            </button>
          </div>

          <button
            onClick={() => setIsPreflightOpen(true)}
            className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs flex items-center space-x-1.5 cursor-pointer"
          >
            <span>Make PodReady</span>
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
            </svg>
          </button>
        </div>
      )}

      {/* Episode Table */}
      <div className="border border-gray-200 rounded-xl overflow-hidden shadow-xs">
        <div className="grid grid-cols-12 bg-gray-50 px-4 py-2.5 text-xs font-semibold text-gray-500 uppercase tracking-wider border-b border-gray-200 items-center">
          <div className="col-span-4 flex items-center space-x-2.5">
            <input
              type="checkbox"
              checked={allVisibleSelected}
              onChange={(e) => {
                if (e.target.checked) {
                  handleSelectAllVisible();
                } else {
                  handleClearSelection();
                }
              }}
              className="rounded text-indigo-600 focus:ring-indigo-500 cursor-pointer"
              title="Select all visible episodes"
            />
            <span>Episode</span>
          </div>
          <div className="col-span-2 text-center">Duration</div>
          <div className="col-span-2 text-center">Loudness</div>
          <div className="col-span-2 text-center">True Peak</div>
          <div className="col-span-2 text-right">Status</div>
        </div>

        <div className="divide-y divide-gray-100 max-h-[480px] overflow-y-auto">
          {sortedEpisodes.map((ep) => {
            const isMissing = ep.sourceAvailability === "MISSING";
            const isChanged = ep.sourceAvailability === "CHANGED";
            const isSelected = selectedEpisodeIds.has(ep.id);

            return (
              <div
                key={ep.id}
                onClick={() => setSelectedEpisode(ep)}
                className={`grid grid-cols-12 items-center px-4 py-3.5 transition-colors cursor-pointer ${
                  isSelected
                    ? "bg-indigo-50/50 hover:bg-indigo-50/70"
                    : "hover:bg-indigo-50/30"
                }`}
              >
                {/* Checkbox, Filename & Analysed Date */}
                <div className="col-span-4 pr-2 truncate flex items-center space-x-2.5">
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onClick={(e) => handleToggleSelect(ep.id, e)}
                    onChange={() => {}}
                    className="rounded text-indigo-600 focus:ring-indigo-500 cursor-pointer shrink-0"
                  />
                  <div className="truncate">
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
          onMakePodReady={handlePublishSingleEpisode}
        />
      )}

      {/* Stage 5E: Batch Publishing Preflight Modal */}
      <BatchPublishingPreflightModal
        isOpen={isPreflightOpen}
        showName={show.name}
        selectedEpisodes={selectedEpisodesList}
        onClose={() => setIsPreflightOpen(false)}
        onStartPublishing={handleStartPublishing}
      />

      {/* Stage 5E: Batch Publishing Progress Modal */}
      {isPublishing && publishingJob && (
        <BatchPublishingProgress
          job={publishingJob}
          onCancel={handleCancelPublishing}
          isCancelling={isCancellingPublishing}
        />
      )}

      {/* Stage 5E: Batch Publishing Results Modal */}
      {showPublishingResults && publishingJob && (
        <BatchPublishingResults
          job={publishingJob}
          onClose={() => {
            setShowPublishingResults(false);
            setPublishingJob(null);
          }}
          onOpenEpisode={(epId) => {
            const ep = episodes.find((e) => e.id === epId);
            if (ep) {
              setSelectedEpisode(ep);
              setShowPublishingResults(false);
            }
          }}
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
