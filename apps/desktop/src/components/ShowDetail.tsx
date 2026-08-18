import { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ShowWithEpisodes,
  CatalogueEpisode,
  ShowBaseline,
  BatchPublishingJob,
  BatchPublishingProgressPayload,
  ShowSummary,
  Show,
  AddBatchEpisodesResult,
  MoveEpisodesResult,
  BatchAnalysisJob,
  BatchProgressPayload,
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
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  // Sorting & Filtering
  const [sortField, setSortField] = useState<SortField>("ANALYSED_AT");
  const [sortAsc, setSortAsc] = useState<boolean>(false); // default newest first
  const [filter, setFilter] = useState<FilterStatus>("ALL");

  // Selection state
  const [selectedEpisodeIds, setSelectedEpisodeIds] = useState<Set<string>>(new Set());

  // Publishing State
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

  // Deletion Confirmation Modal (Show)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<boolean>(false);
  const [isDeletingShow, setIsDeletingShow] = useState<boolean>(false);

  // Episode Inspection Modal
  const [selectedEpisode, setSelectedEpisode] = useState<CatalogueEpisode | null>(null);

  // Active open row menu (by episode id)
  const [openRowMenuId, setOpenRowMenuId] = useState<string | null>(null);

  // Adding Episodes Direct Flow State
  const [isAddingEpisodes, setIsAddingEpisodes] = useState<boolean>(false);
  const [addingStatusText, setAddingStatusText] = useState<string | null>(null);

  // In-Place Re-analysing State
  const [reanalysingEpId, setReanalysingEpId] = useState<string | null>(null);

  // Move Episodes Modal State
  const [isMoveModalOpen, setIsMoveModalOpen] = useState<boolean>(false);
  const [episodesToMove, setEpisodesToMove] = useState<CatalogueEpisode[]>([]);
  const [availableShows, setAvailableShows] = useState<ShowSummary[]>([]);
  const [moveTargetShowId, setMoveTargetShowId] = useState<string>("");
  const [moveNewShowName, setMoveNewShowName] = useState<string>("");
  const [moveNewShowDesc, setMoveNewShowDesc] = useState<string>("");
  const [isMoving, setIsMoving] = useState<boolean>(false);

  // Remove Episodes Confirmation Modal State
  const [isRemoveConfirmOpen, setIsRemoveConfirmOpen] = useState<boolean>(false);
  const [episodesToRemove, setEpisodesToRemove] = useState<CatalogueEpisode[]>([]);
  const [isRemoving, setIsRemoving] = useState<boolean>(false);

  // Ref for closing row menu on outside click
  const menuContainerRef = useRef<HTMLDivElement | null>(null);

  const showToast = (msg: string) => {
    setToastMessage(msg);
    setTimeout(() => {
      setToastMessage((current) => (current === msg ? null : current));
    }, 5000);
  };

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
    setSelectedEpisodeIds(new Set());
  }, [loadShowData]);

  // Click outside to close row menu
  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (menuContainerRef.current && !menuContainerRef.current.contains(e.target as Node)) {
        setOpenRowMenuId(null);
      }
    };
    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, []);

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

  // Add Episodes from disk directly into this Show
  const handleAddEpisodesFromDisk = async () => {
    try {
      const paths = await invoke<string[]>("select_files_cmd");
      if (!paths || paths.length === 0) return;

      setIsAddingEpisodes(true);
      setAddingStatusText(`Analysing ${paths.length} ${paths.length === 1 ? "file" : "files"}…`);

      // Start batch analysis
      const job = await invoke<BatchAnalysisJob>("start_batch_analysis_cmd", { paths });

      // Listen for batch completion
      const unlisten = await listen<BatchProgressPayload>("batch-complete", async (event) => {
        if (event.payload.jobId === job.id) {
          unlisten();
          setAddingStatusText("Adding analysed episodes to catalogue…");
          try {
            const addRes = await invoke<AddBatchEpisodesResult>("add_batch_episodes_to_show_cmd", {
              showId,
              jobId: job.id,
            });

            const parts: string[] = [];
            if (addRes.added > 0) parts.push(`${addRes.added} added`);
            if (addRes.alreadyExists > 0) parts.push(`${addRes.alreadyExists} already in show`);
            if (addRes.updated > 0) parts.push(`${addRes.updated} updated`);
            if (addRes.skippedFailed > 0) parts.push(`${addRes.skippedFailed} failed`);

            showToast(`Catalogue updated: ${parts.join(", ")}.`);
            await loadShowData();
          } catch (err: any) {
            console.error("Failed to add batch to catalogue:", err);
            setError(err.message || "Failed to add episodes to show catalogue.");
          } finally {
            setIsAddingEpisodes(false);
            setAddingStatusText(null);
          }
        }
      });
    } catch (err: any) {
      console.error("Failed to add episodes:", err);
      setError(err.message || "Failed to add episodes.");
      setIsAddingEpisodes(false);
      setAddingStatusText(null);
    }
  };

  // Direct In-Place Re-analysis
  const handleDirectReanalyse = async (episodeId: string) => {
    setReanalysingEpId(episodeId);
    setOpenRowMenuId(null);
    try {
      const updated = await invoke<CatalogueEpisode>("reanalyse_catalogue_episode_cmd", {
        id: episodeId,
      });
      showToast(`"${updated.filename}" re-analysed and updated.`);
      await loadShowData();
      if (selectedEpisode && selectedEpisode.id === episodeId) {
        setSelectedEpisode(updated);
      }
    } catch (err: any) {
      console.error("Re-analysis failed:", err);
      setError(err.message || "Re-analysis failed.");
    } finally {
      setReanalysingEpId(null);
    }
  };

  // Locate Missing File
  const handleLocateMissingFile = async (ep: CatalogueEpisode) => {
    setOpenRowMenuId(null);
    try {
      const paths = await invoke<string[]>("select_files_cmd");
      if (!paths || paths.length === 0) return;

      const newPath = paths[0];
      setReanalysingEpId(ep.id);
      const updated = await invoke<CatalogueEpisode>("relink_catalogue_episode_cmd", {
        episodeId: ep.id,
        newSourcePath: newPath,
      });
      showToast(`Source relinked and verified for "${updated.filename}".`);
      await loadShowData();
      if (selectedEpisode && selectedEpisode.id === ep.id) {
        setSelectedEpisode(updated);
      }
    } catch (err: any) {
      console.error("Relink failed:", err);
      setError(err.message || "Failed to relink missing source file.");
    } finally {
      setReanalysingEpId(null);
    }
  };

  // Open Move Modal
  const handleOpenMoveModal = async (episodes: CatalogueEpisode[]) => {
    setOpenRowMenuId(null);
    setEpisodesToMove(episodes);
    setIsMoving(false);
    setMoveNewShowName("");
    setMoveNewShowDesc("");
    try {
      const allShows = await invoke<ShowSummary[]>("get_shows_cmd");
      const otherShows = allShows.filter((s) => s.id !== showId);
      setAvailableShows(otherShows);
      if (otherShows.length > 0) {
        setMoveTargetShowId(otherShows[0].id);
      } else {
        setMoveTargetShowId("NEW");
      }
      setIsMoveModalOpen(true);
    } catch (err: any) {
      console.error("Failed to load shows for move:", err);
      setError(err.message || "Failed to load shows list.");
    }
  };

  // Execute Move Episodes
  const handleExecuteMove = async (e: React.FormEvent) => {
    e.preventDefault();
    if (episodesToMove.length === 0) return;

    setIsMoving(true);
    try {
      let targetId = moveTargetShowId;

      // If creating new show
      if (targetId === "NEW") {
        if (!moveNewShowName.trim()) {
          setError("Please enter a name for the new show.");
          setIsMoving(false);
          return;
        }
        const created = await invoke<Show>("create_show_cmd", {
          input: {
            name: moveNewShowName.trim(),
            description: moveNewShowDesc.trim() || undefined,
          },
        });
        targetId = created.id;
      }

      const episodeIds = episodesToMove.map((ep) => ep.id);
      const res = await invoke<MoveEpisodesResult>("move_catalogue_episodes_cmd", {
        episodeIds,
        targetShowId: targetId,
      });

      // Remove moved episodes from selection
      setSelectedEpisodeIds((prev) => {
        const next = new Set(prev);
        for (const id of episodeIds) next.delete(id);
        return next;
      });

      setIsMoveModalOpen(false);
      setSelectedEpisode(null);
      showToast(
        `${res.moved} ${res.moved === 1 ? "episode" : "episodes"} moved to "${res.targetShowName}".`
      );
      await loadShowData();
    } catch (err: any) {
      console.error("Move failed:", err);
      setError(err.message || "Failed to move episode(s).");
    } finally {
      setIsMoving(false);
    }
  };

  // Open Remove Confirmation
  const handleOpenRemoveConfirm = (episodes: CatalogueEpisode[]) => {
    setOpenRowMenuId(null);
    setEpisodesToRemove(episodes);
    setIsRemoveConfirmOpen(true);
  };

  // Execute Remove Episodes
  const handleExecuteRemove = async () => {
    if (episodesToRemove.length === 0) return;
    setIsRemoving(true);
    try {
      const episodeIds = episodesToRemove.map((ep) => ep.id);
      await invoke("delete_catalogue_episodes_cmd", { episodeIds });

      setSelectedEpisodeIds((prev) => {
        const next = new Set(prev);
        for (const id of episodeIds) next.delete(id);
        return next;
      });

      setIsRemoveConfirmOpen(false);
      setSelectedEpisode(null);
      showToast(
        `Removed ${episodesToRemove.length} ${
          episodesToRemove.length === 1 ? "episode" : "episodes"
        } from show catalogue.`
      );
      await loadShowData();
    } catch (err: any) {
      console.error("Failed to remove episode(s):", err);
      setError(err.message || "Failed to remove episode(s) from show.");
    } finally {
      setIsRemoving(false);
    }
  };

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
      showToast("Show details updated.");
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
      if (filter === "UNKNOWN")
        return !["READY", "ATTENTION", "NEEDS_ATTENTION"].includes(ep.overallAssessmentStatus);
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
    setOpenRowMenuId(null);
    setSelectedEpisodeIds(new Set([ep.id]));
    setIsPreflightOpen(true);
    setSelectedEpisode(null);
  };

  const handleMakeWholeShowPodReady = () => {
    if (!data) return;
    const eligible = data.episodes.filter((e) => e.sourceAvailability !== "MISSING");
    if (eligible.length === 0) {
      setError("No available episodes to publish in this show.");
      return;
    }
    setSelectedEpisodeIds(new Set(eligible.map((e) => e.id)));
    setIsPreflightOpen(true);
  };

  if (isLoading) {
    return (
      <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl p-12 text-center shadow-xs">
        <p className="text-sm font-medium text-gray-500 animate-pulse">Loading show catalogue…</p>
      </div>
    );
  }

  if (error && !data) {
    return (
      <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl p-8 space-y-4 shadow-xs">
        <div className="p-4 bg-rose-50 border border-rose-200 rounded-xl text-rose-800 text-sm font-medium">
          {error}
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

  if (!data) return null;

  const { show, episodes } = data;
  const readyCount = episodes.filter((e) => e.overallAssessmentStatus === "READY").length;
  const attentionCount = episodes.filter((e) => e.overallAssessmentStatus === "ATTENTION").length;
  const needsAttentionCount = episodes.filter(
    (e) => e.overallAssessmentStatus === "NEEDS_ATTENTION"
  ).length;
  const unknownCount = episodes.filter(
    (e) => !["READY", "ATTENTION", "NEEDS_ATTENTION"].includes(e.overallAssessmentStatus)
  ).length;
  const changedCount = episodes.filter((e) => e.sourceAvailability === "CHANGED").length;
  const missingCount = episodes.filter((e) => e.sourceAvailability === "MISSING").length;

  const allVisibleSelected =
    sortedEpisodes.length > 0 && sortedEpisodes.every((e) => selectedEpisodeIds.has(e.id));

  return (
    <div
      ref={menuContainerRef}
      className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl shadow-sm p-8 flex flex-col space-y-6 relative"
    >
      {/* Toast Feedback Notification */}
      {toastMessage && (
        <div className="fixed top-16 right-8 z-50 p-4 max-w-sm bg-gray-900 text-white text-xs font-medium rounded-xl shadow-xl flex items-center justify-between space-x-3 transition-all animate-in fade-in slide-in-from-top-2">
          <span>{toastMessage}</span>
          <button
            onClick={() => setToastMessage(null)}
            className="text-gray-400 hover:text-white p-1"
          >
            ✕
          </button>
        </div>
      )}

      {/* Adding Episodes Progress Banner */}
      {isAddingEpisodes && (
        <div className="p-4 bg-indigo-50 border border-indigo-200 rounded-xl flex items-center justify-between text-xs text-indigo-950 shadow-xs animate-pulse">
          <div className="flex items-center space-x-2.5">
            <svg
              className="animate-spin h-4 w-4 text-indigo-600"
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
            <span className="font-semibold">{addingStatusText || "Adding episodes to show…"}</span>
          </div>
          <span className="text-[11px] text-indigo-700">Analysing using bundled engine</span>
        </div>
      )}

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
          <button
            onClick={handleAddEpisodesFromDisk}
            disabled={isAddingEpisodes}
            className="px-3.5 py-1.5 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs flex items-center space-x-1.5 disabled:opacity-50"
          >
            <span>+ Add Episodes</span>
          </button>
          {episodes.length > 0 && (
            <button
              onClick={handleMakeWholeShowPodReady}
              className="px-3 py-1.5 text-xs font-semibold text-indigo-700 bg-indigo-50 hover:bg-indigo-100 rounded-lg transition-colors"
              title="Publish all eligible episodes in this show"
            >
              Make Show PodReady
            </button>
          )}
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

      {/* Selection Action Bar (Multi-Management) */}
      {selectedEpisodeIds.size > 0 && (
        <div className="flex items-center justify-between p-3.5 bg-indigo-50/70 border border-indigo-100 rounded-xl transition-all">
          <div className="flex items-center space-x-3 text-xs">
            <span className="font-bold text-indigo-950">
              {selectedEpisodeIds.size}{" "}
              {selectedEpisodeIds.size === 1 ? "episode selected" : "episodes selected"}
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

          <div className="flex items-center space-x-2">
            <button
              onClick={() => handleOpenMoveModal(selectedEpisodesList)}
              className="px-3 py-1.5 text-xs font-semibold text-gray-700 bg-white hover:bg-gray-100 border border-gray-200 rounded-lg transition-colors shadow-2xs"
            >
              Move to Show…
            </button>
            <button
              onClick={() => handleOpenRemoveConfirm(selectedEpisodesList)}
              className="px-3 py-1.5 text-xs font-semibold text-rose-700 bg-white hover:bg-rose-50 border border-rose-200 rounded-lg transition-colors shadow-2xs"
            >
              Remove from Show
            </button>
            <button
              onClick={() => setIsPreflightOpen(true)}
              className="px-4 py-1.5 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs flex items-center space-x-1.5 cursor-pointer"
            >
              <span>Make PodReady</span>
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
              </svg>
            </button>
          </div>
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
          <div className="col-span-1 text-center">Status</div>
          <div className="col-span-1 text-right">Actions</div>
        </div>

        <div className="divide-y divide-gray-100 max-h-[480px] overflow-y-auto">
          {sortedEpisodes.map((ep) => {
            const isMissing = ep.sourceAvailability === "MISSING";
            const isChanged = ep.sourceAvailability === "CHANGED";
            const isSelected = selectedEpisodeIds.has(ep.id);
            const isReanalysing = reanalysingEpId === ep.id;
            const isMenuOpen = openRowMenuId === ep.id;

            return (
              <div
                key={ep.id}
                onClick={() => setSelectedEpisode(ep)}
                className={`grid grid-cols-12 items-center px-4 py-3.5 transition-colors cursor-pointer relative ${
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
                <div className="col-span-1 text-center">
                  {getStatusBadge(ep.overallAssessmentStatus)}
                </div>

                {/* Row Actions Menu */}
                <div className="col-span-1 text-right relative">
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setOpenRowMenuId(isMenuOpen ? null : ep.id);
                    }}
                    className="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100 transition-colors"
                    aria-label={`Actions for ${ep.filename}`}
                  >
                    {isReanalysing ? (
                      <svg
                        className="animate-spin h-4 w-4 text-indigo-600"
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
                    ) : (
                      <span className="font-bold text-sm tracking-widest leading-none">⋯</span>
                    )}
                  </button>

                  {/* Dropdown Menu */}
                  {isMenuOpen && (
                    <div
                      onClick={(e) => e.stopPropagation()}
                      className="absolute right-0 top-8 z-30 w-48 bg-white border border-gray-200 rounded-xl shadow-xl py-1 text-left text-xs text-gray-700 animate-in fade-in zoom-in-95 duration-100"
                    >
                      <button
                        onClick={() => {
                          setOpenRowMenuId(null);
                          setSelectedEpisode(ep);
                        }}
                        className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-medium"
                      >
                        <span>🔍</span>
                        <span>View details</span>
                      </button>

                      {!isMissing && (
                        <button
                          onClick={() => handlePublishSingleEpisode(ep)}
                          className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-semibold text-indigo-600"
                        >
                          <span>🚀</span>
                          <span>Make PodReady</span>
                        </button>
                      )}

                      {!isMissing && (
                        <button
                          onClick={() => handleDirectReanalyse(ep.id)}
                          className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-medium text-gray-700"
                        >
                          <span>🔄</span>
                          <span>Re-analyse in place</span>
                        </button>
                      )}

                      {!isMissing && (
                        <button
                          onClick={() => {
                            setOpenRowMenuId(null);
                            onOpenInWorkspace(ep.sourcePath);
                          }}
                          className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-medium"
                        >
                          <span>🎛️</span>
                          <span>Open in Workspace</span>
                        </button>
                      )}

                      {isMissing && (
                        <button
                          onClick={() => handleLocateMissingFile(ep)}
                          className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-medium text-amber-700"
                        >
                          <span>📁</span>
                          <span>Locate Missing File…</span>
                        </button>
                      )}

                      <button
                        onClick={() => handleOpenMoveModal([ep])}
                        className="w-full px-3.5 py-2 text-left hover:bg-gray-50 flex items-center space-x-2 font-medium"
                      >
                        <span>📦</span>
                        <span>Move to Show…</span>
                      </button>

                      <div className="border-t border-gray-100 my-1" />

                      <button
                        onClick={() => handleOpenRemoveConfirm([ep])}
                        className="w-full px-3.5 py-2 text-left hover:bg-rose-50 flex items-center space-x-2 font-semibold text-rose-600"
                      >
                        <span>🗑️</span>
                        <span>Remove from Show</span>
                      </button>
                    </div>
                  )}
                </div>
              </div>
            );
          })}

          {sortedEpisodes.length === 0 && (
            <div className="p-12 text-center text-sm text-gray-500 space-y-3">
              {episodes.length === 0 ? (
                <div className="space-y-3">
                  <p className="text-gray-800 font-semibold text-base">No episodes yet</p>
                  <p className="text-xs text-gray-500 max-w-sm mx-auto leading-relaxed">
                    Add a few previous episodes and PodReady can establish what this Show normally
                    sounds like.
                  </p>
                  <button
                    onClick={handleAddEpisodesFromDisk}
                    disabled={isAddingEpisodes}
                    className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold rounded-xl transition-colors shadow-xs"
                  >
                    + Add Episodes
                  </button>
                </div>
              ) : (
                <p>No episodes match the selected filter.</p>
              )}
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
          onDeleteEpisode={() => handleOpenRemoveConfirm([selectedEpisode])}
          onMakePodReady={handlePublishSingleEpisode}
          onMoveEpisode={() => handleOpenMoveModal([selectedEpisode])}
          onReanalyseEpisode={handleDirectReanalyse}
          onLocateFile={() => handleLocateMissingFile(selectedEpisode)}
        />
      )}

      {/* Batch Publishing Preflight Modal */}
      <BatchPublishingPreflightModal
        isOpen={isPreflightOpen}
        showName={show.name}
        selectedEpisodes={selectedEpisodesList}
        onClose={() => setIsPreflightOpen(false)}
        onStartPublishing={handleStartPublishing}
      />

      {/* Batch Publishing Progress Modal */}
      {isPublishing && publishingJob && (
        <BatchPublishingProgress
          job={publishingJob}
          onCancel={handleCancelPublishing}
          isCancelling={isCancellingPublishing}
        />
      )}

      {/* Batch Publishing Results Modal */}
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

      {/* Remove Episode(s) Confirmation Dialog */}
      {isRemoveConfirmOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-4 animate-in fade-in zoom-in-95 duration-150">
            <h3 className="text-base font-bold text-gray-900">
              {episodesToRemove.length === 1
                ? `Remove "${episodesToRemove[0].filename}" from ${show.name}?`
                : `Remove ${episodesToRemove.length} episodes from ${show.name}?`}
            </h3>
            <p className="text-xs text-gray-600 leading-relaxed">
              This removes the {episodesToRemove.length === 1 ? "episode" : "episodes"} from
              PodReady's catalogue record.
            </p>
            <div className="p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-900 flex items-center space-x-2">
              <span>🛡️</span>
              <span>
                <strong>Your media files are safe:</strong> Original audio files will NEVER be
                modified or deleted.
              </span>
            </div>
            <div className="flex items-center justify-end space-x-3 pt-2">
              <button
                type="button"
                onClick={() => setIsRemoveConfirmOpen(false)}
                disabled={isRemoving}
                className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleExecuteRemove}
                disabled={isRemoving}
                className="px-4 py-2 text-xs font-bold text-white bg-rose-600 hover:bg-rose-700 rounded-lg transition-colors shadow-xs disabled:opacity-50"
              >
                {isRemoving ? "Removing…" : "Remove"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Move Episode(s) Modal */}
      {isMoveModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-4 animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-gray-100 pb-3">
              <h3 className="text-base font-bold text-gray-900">
                Move {episodesToMove.length === 1 ? "Episode" : `${episodesToMove.length} Episodes`} to Show
              </h3>
              <button
                onClick={() => setIsMoveModalOpen(false)}
                className="text-gray-400 hover:text-gray-600 p-1"
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleExecuteMove} className="space-y-4">
              <div className="space-y-1.5">
                <label className="block text-xs font-bold text-gray-700">Destination Show</label>
                <select
                  value={moveTargetShowId}
                  onChange={(e) => setMoveTargetShowId(e.target.value)}
                  className="w-full bg-white border border-gray-300 text-gray-900 text-xs font-medium rounded-lg p-2.5 focus:ring-1 focus:ring-indigo-500"
                >
                  {availableShows.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.episodeCount} eps)
                    </option>
                  ))}
                  <option value="NEW">+ Create New Show…</option>
                </select>
              </div>

              {moveTargetShowId === "NEW" && (
                <div className="space-y-3 p-3 bg-gray-50 border border-gray-200 rounded-xl">
                  <div className="space-y-1">
                    <label className="block text-[11px] font-bold text-gray-700">
                      New Show Name <span className="text-rose-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={moveNewShowName}
                      onChange={(e) => setMoveNewShowName(e.target.value)}
                      placeholder="e.g. My Next Podcast"
                      className="w-full bg-white border border-gray-300 text-gray-900 text-xs rounded-lg p-2 focus:ring-1 focus:ring-indigo-500"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="block text-[11px] font-bold text-gray-700">
                      Description (optional)
                    </label>
                    <input
                      type="text"
                      value={moveNewShowDesc}
                      onChange={(e) => setMoveNewShowDesc(e.target.value)}
                      placeholder="Show topic"
                      className="w-full bg-white border border-gray-300 text-gray-900 text-xs rounded-lg p-2 focus:ring-1 focus:ring-indigo-500"
                    />
                  </div>
                </div>
              )}

              <div className="p-3 bg-blue-50 border border-blue-200 rounded-xl text-xs text-blue-900 flex items-center space-x-2">
                <span>ℹ️</span>
                <span>Moving alters catalogue ownership only. Source audio files on disk remain untouched.</span>
              </div>

              <div className="flex items-center justify-end space-x-3 pt-2">
                <button
                  type="button"
                  onClick={() => setIsMoveModalOpen(false)}
                  disabled={isMoving}
                  className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isMoving}
                  className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs disabled:opacity-50"
                >
                  {isMoving ? "Moving…" : "Move"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Delete Show Confirmation Dialog */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-md w-full p-6 space-y-4">
            <h3 className="text-base font-bold text-gray-900">Delete Show Catalogue?</h3>
            <p className="text-xs text-gray-600 leading-relaxed">
              This will remove <strong className="text-gray-900">{show.name}</strong> and its{" "}
              {episodes.length} catalogued records from PodReady.
            </p>
            <div className="p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-900 flex items-center space-x-2">
              <span>🛡️</span>
              <span>
                <strong>Your media files are safe:</strong> Original audio files will NEVER be
                modified or deleted.
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
