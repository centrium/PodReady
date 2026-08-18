import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  MediaSource,
  AudioMeasurements,
  Assessment,
  FixPlan,
  ProcessAudioResponse,
  PodReadyPackage,
  ExportOptions,
  BatchAnalysisJob,
  BatchEpisode,
  BatchProgressPayload,
  ShowSummary,
} from "@podready/domain";
import { Dropzone } from "./components/Dropzone";
import { Report } from "./components/Report";
import { BatchProgress } from "./components/BatchProgress";
import { BatchResults } from "./components/BatchResults";
import { ShowLibrary } from "./components/ShowLibrary";
import { ShowDetail } from "./components/ShowDetail";
import { AddToShowModal } from "./components/AddToShowModal";

type NavigationTab = "WORKSPACE" | "SHOWS";

function App() {
  // Navigation
  const [currentTab, setCurrentTab] = useState<NavigationTab>("WORKSPACE");
  const [selectedShowId, setSelectedShowId] = useState<string | null>(null);
  const [showsCount, setShowsCount] = useState<number>(0);

  // Single episode state
  const [media, setMedia] = useState<MediaSource | null>(null);
  const [loadingFile, setLoadingFile] = useState<string | null>(null);
  const [isAnalysing, setIsAnalysing] = useState<boolean>(false);
  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [processingResponse, setProcessingResponse] = useState<ProcessAudioResponse | null>(null);
  const [isExporting, setIsExporting] = useState<boolean>(false);
  const [exportResult, setExportResult] = useState<PodReadyPackage | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Batch state
  const [batchJob, setBatchJob] = useState<BatchAnalysisJob | null>(null);
  const [isBatchRunning, setIsBatchRunning] = useState<boolean>(false);
  const [isCancellingBatch, setIsCancellingBatch] = useState<boolean>(false);
  const [selectedBatchEpisode, setSelectedBatchEpisode] = useState<BatchEpisode | null>(null);

  // Add to Show modal state
  const [isAddToShowOpen, setIsAddToShowOpen] = useState<boolean>(false);
  const [addToShowTarget, setAddToShowTarget] = useState<"SINGLE" | "BATCH">("SINGLE");

  const batchJobRef = useRef<BatchAnalysisJob | null>(null);
  batchJobRef.current = batchJob;

  useEffect(() => {
    loadShowsCount();
  }, [currentTab, isAddToShowOpen]);

  const loadShowsCount = async () => {
    try {
      const shows = await invoke<ShowSummary[]>("get_shows_cmd");
      setShowsCount(shows.length);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;

    const setupBatchListeners = async () => {
      unlistenProgress = await listen<BatchProgressPayload>("batch-progress", (event) => {
        const payload = event.payload;
        setBatchJob((prev) => {
          if (!prev || prev.id !== payload.jobId) return prev;
          const updatedEpisodes = prev.episodes.map((ep) =>
            ep.id === payload.episodeId ? payload.episode : ep
          );
          return {
            ...prev,
            episodes: updatedEpisodes,
            summary: payload.summary,
          };
        });
      });

      unlistenComplete = await listen<BatchProgressPayload>("batch-complete", (event) => {
        const payload = event.payload;
        setBatchJob((prev) => {
          if (!prev || prev.id !== payload.jobId) return prev;
          const updatedEpisodes = prev.episodes.map((ep) =>
            ep.id === payload.episodeId ? payload.episode : ep
          );
          return {
            ...prev,
            status: "COMPLETE",
            episodes: updatedEpisodes,
            summary: payload.summary,
          };
        });
        setIsBatchRunning(false);
        setIsCancellingBatch(false);
      });
    };

    setupBatchListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
    };
  }, []);

  const handleSingleFileDropped = async (path: string) => {
    const filename = path.split(/[/\\]/).pop() || path;

    setLoadingFile(filename);
    setError(null);
    setMedia(null);
    setBatchJob(null);
    setIsBatchRunning(false);
    setSelectedBatchEpisode(null);
    setIsAnalysing(false);
    setIsProcessing(false);
    setProcessingResponse(null);
    setIsExporting(false);
    setExportResult(null);
    setCurrentTab("WORKSPACE");

    try {
      // Step 1: Inspect media properties via ffprobe
      const inspected = await invoke<MediaSource>("inspect_media_cmd", { path });

      // Run initial file inspection assessment
      const initialAssessment = await invoke<Assessment>("assess_media_cmd", {
        inspection: inspected.inspection,
        measurements: null,
        format: inspected.format,
        codec: inspected.codec,
      });

      const initialFixPlan = await invoke<FixPlan>("generate_fix_plan_cmd", {
        assessment: initialAssessment,
      });

      setMedia({ ...inspected, assessment: initialAssessment, fixPlan: initialFixPlan });
      setLoadingFile(null);

      // Step 2: Analyse audio measurements via ffmpeg (non-blocking)
      setIsAnalysing(true);
      const measurements = await invoke<AudioMeasurements>("analyse_audio_cmd", {
        path,
        durationSeconds: inspected.inspection.durationSeconds,
      });

      // Step 3: Run comprehensive audio assessment
      const assessment = await invoke<Assessment>("assess_media_cmd", {
        inspection: inspected.inspection,
        measurements,
        format: inspected.format,
        codec: inspected.codec,
      });

      // Step 4: Generate deterministic FixPlan
      const fixPlan = await invoke<FixPlan>("generate_fix_plan_cmd", {
        assessment,
      });

      setMedia((prev) => (prev ? { ...prev, measurements, assessment, fixPlan } : null));
    } catch (err: any) {
      console.error(err);
      setError(err.message || "An unexpected error occurred.");
    } finally {
      setLoadingFile(null);
      setIsAnalysing(false);
    }
  };

  const handleBatchFilesDropped = async (paths: string[]) => {
    setError(null);
    setMedia(null);
    setLoadingFile(null);
    setSelectedBatchEpisode(null);
    setIsCancellingBatch(false);
    setCurrentTab("WORKSPACE");

    try {
      const job = await invoke<BatchAnalysisJob>("start_batch_analysis_cmd", { paths });
      setBatchJob(job);
      setIsBatchRunning(true);
    } catch (err: any) {
      console.error("Failed to start batch analysis:", err);
      setError(err.message || "Failed to start batch analysis.");
    }
  };

  const handleFilesDropped = (paths: string[]) => {
    if (!paths || paths.length === 0) return;
    if (paths.length === 1) {
      handleSingleFileDropped(paths[0]);
    } else {
      handleBatchFilesDropped(paths);
    }
  };

  const handleCancelBatch = async () => {
    if (!batchJob) return;
    setIsCancellingBatch(true);
    try {
      await invoke("cancel_batch_analysis_cmd", { jobId: batchJob.id });
      // Fetch latest state
      const updated = await invoke<BatchAnalysisJob>("get_batch_job_cmd", {
        jobId: batchJob.id,
      });
      setBatchJob(updated);
    } catch (err: any) {
      console.error("Failed to cancel batch job:", err);
    } finally {
      setIsBatchRunning(false);
      setIsCancellingBatch(false);
    }
  };

  const handleReset = () => {
    setMedia(null);
    setBatchJob(null);
    setIsBatchRunning(false);
    setSelectedBatchEpisode(null);
    setError(null);
  };

  const handleProcessAudio = async () => {
    if (!media || !media.fixPlan) return;
    setIsProcessing(true);
    setError(null);

    try {
      const response = await invoke<ProcessAudioResponse>("process_audio_cmd", {
        sourcePath: media.path,
        plan: media.fixPlan,
        beforeMeasurements: media.measurements || null,
        beforeAssessment: media.assessment || null,
      });
      setProcessingResponse(response);
    } catch (err: any) {
      console.error(err);
      setError(err.message || "Audio processing failed.");
    } finally {
      setIsProcessing(false);
    }
  };

  const handleExportPackage = async (options: ExportOptions) => {
    if (!media) return;
    setIsExporting(true);
    setError(null);
    const startedAt = performance.now();

    try {
      const inputAudioPath = processingResponse?.candidatePath || media.path;
      const sourceOriginalPath = media.path;
      const beforeMeasurements =
        processingResponse?.beforeMeasurements || media.measurements || null;
      const beforeAssessment = processingResponse?.beforeAssessment || media.assessment || null;
      const appliedActions = (processingResponse?.result.actionsApplied || []).map((a) => ({
        actionType: a.actionType,
        title: a.title,
        description: a.description,
        success: a.success,
      }));

      const pkg = await invoke<PodReadyPackage>("export_package_cmd", {
        inputAudioPath,
        sourceOriginalPath,
        options,
        beforeMeasurements,
        beforeAssessment,
        appliedActions,
      });

      const elapsedSeconds = (performance.now() - startedAt) / 1000;
      const backendSeconds = pkg.generationDurationSeconds ?? 0;
      const diffSeconds = elapsedSeconds - backendSeconds;

      console.info(
        `[PodReady Export Timing] User elapsed: ${elapsedSeconds.toFixed(1)}s | ` +
          `Backend package: ${backendSeconds.toFixed(1)}s | ` +
          `Discrepancy (IPC/overhead): ${diffSeconds.toFixed(1)}s`
      );

      setExportResult({
        ...pkg,
        userElapsedSeconds: elapsedSeconds,
      });
    } catch (err: any) {
      console.error(err);
      setError(err.message || "Failed to export publishing package.");
    } finally {
      setIsExporting(false);
    }
  };

  const openAddToShowModal = (target: "SINGLE" | "BATCH") => {
    setAddToShowTarget(target);
    setIsAddToShowOpen(true);
  };

  // Convert BatchEpisode to MediaSource for Report display
  const batchEpisodeMedia: MediaSource | null = selectedBatchEpisode
    ? {
        path: selectedBatchEpisode.sourcePath,
        filename: selectedBatchEpisode.filename,
        format: selectedBatchEpisode.format || "UNKNOWN",
        codec: selectedBatchEpisode.codec || "",
        inspection: selectedBatchEpisode.inspection || {
          durationSeconds: selectedBatchEpisode.durationSeconds || 0,
          sampleRate: 0,
          channels: 0,
          fileSizeBytes: 0,
        },
        measurements: selectedBatchEpisode.measurements,
        assessment: selectedBatchEpisode.assessment,
      }
    : null;

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col font-sans">
      {/* App Top Navigation Bar */}
      <header className="w-full bg-white border-b border-gray-200 sticky top-0 z-40 px-6 py-3 shadow-2xs">
        <div className="max-w-6xl mx-auto flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-2">
              <span className="w-3 h-3 rounded-full bg-indigo-600 shadow-xs" />
              <h1 className="text-base font-bold tracking-tight text-gray-900">
                PodReady
              </h1>
            </div>
            <span className="text-xs font-semibold text-gray-300">/</span>
            <span className="text-xs font-medium text-gray-500">
              {currentTab === "WORKSPACE" ? "Audio Workspace" : "Show Catalogue"}
            </span>
          </div>

          <div className="flex items-center space-x-1.5 bg-gray-100 p-1 rounded-xl">
            <button
              onClick={() => {
                setCurrentTab("WORKSPACE");
              }}
              className={`px-3 py-1.5 text-xs font-semibold rounded-lg transition-all ${
                currentTab === "WORKSPACE"
                  ? "bg-white text-gray-900 shadow-xs"
                  : "text-gray-600 hover:text-gray-900"
              }`}
            >
              Workspace
            </button>
            <button
              onClick={() => {
                setCurrentTab("SHOWS");
              }}
              className={`px-3 py-1.5 text-xs font-semibold rounded-lg transition-all flex items-center space-x-1.5 ${
                currentTab === "SHOWS"
                  ? "bg-white text-gray-900 shadow-xs"
                  : "text-gray-600 hover:text-gray-900"
              }`}
            >
              <span>Shows</span>
              {showsCount > 0 && (
                <span className="px-1.5 py-0.2 text-[10px] font-bold rounded-full bg-indigo-100 text-indigo-800">
                  {showsCount}
                </span>
              )}
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 flex items-center justify-center p-6 md:p-8">
        {/* TAB 1: WORKSPACE */}
        {currentTab === "WORKSPACE" && (
          <div className="w-full flex justify-center">
            {/* 1. Idle state: Dropzone */}
            {!loadingFile && !media && !batchJob && (
              <Dropzone onFilesDropped={handleFilesDropped} />
            )}

            {/* 2. Loading single file initial inspection */}
            {loadingFile && (
              <div className="flex flex-col items-center justify-center space-y-4">
                <h2 className="text-xl font-medium text-gray-900">{loadingFile}</h2>
                <p className="text-gray-500 animate-pulse">Checking your episode…</p>
              </div>
            )}

            {/* 3. Single episode Report */}
            {media && !selectedBatchEpisode && (
              <div className="flex flex-col items-center space-y-4 w-full">
                <div className="w-full max-w-lg flex justify-start">
                  <button
                    onClick={handleReset}
                    className="text-xs font-semibold text-gray-500 hover:text-gray-800 flex items-center space-x-1"
                  >
                    <span>←</span>
                    <span>Analyse another file</span>
                  </button>
                </div>
                <Report
                  media={media}
                  isAnalysing={isAnalysing}
                  isProcessing={isProcessing}
                  processingResponse={processingResponse}
                  onProcessAudio={handleProcessAudio}
                  isExporting={isExporting}
                  exportResult={exportResult}
                  onExport={handleExportPackage}
                  onAddToShow={() => openAddToShowModal("SINGLE")}
                />
              </div>
            )}

            {/* 4. Batch in-progress */}
            {batchJob && isBatchRunning && !selectedBatchEpisode && (
              <BatchProgress
                job={batchJob}
                onCancel={handleCancelBatch}
                isCancelling={isCancellingBatch}
              />
            )}

            {/* 5. Batch results completed / cancelled */}
            {batchJob && !isBatchRunning && !selectedBatchEpisode && (
              <BatchResults
                job={batchJob}
                onSelectEpisode={(ep) => setSelectedBatchEpisode(ep)}
                onReset={handleReset}
                onAddToShow={() => openAddToShowModal("BATCH")}
              />
            )}

            {/* 6. Batch single-episode drill-down view */}
            {selectedBatchEpisode && batchEpisodeMedia && (
              <div className="flex flex-col w-full max-w-4xl space-y-4">
                <div className="flex items-center justify-between bg-white border border-gray-200 px-6 py-3 rounded-xl shadow-xs">
                  <button
                    onClick={() => setSelectedBatchEpisode(null)}
                    className="flex items-center text-sm font-medium text-gray-700 hover:text-gray-900 transition-colors"
                  >
                    <svg
                      className="w-4 h-4 mr-1.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M10 19l-7-7m0 0l7-7m-7 7h18"
                      />
                    </svg>
                    Back to Batch Results
                  </button>
                  <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
                    Batch Episode Assessment
                  </span>
                </div>

                <Report media={batchEpisodeMedia} isAnalysing={false} />
              </div>
            )}
          </div>
        )}

        {/* TAB 2: SHOWS LIBRARY */}
        {currentTab === "SHOWS" && (
          <div className="w-full flex justify-center">
            {selectedShowId ? (
              <ShowDetail
                showId={selectedShowId}
                onBack={() => setSelectedShowId(null)}
                onOpenInWorkspace={(path) => {
                  handleSingleFileDropped(path);
                }}
                onShowDeleted={() => {
                  setSelectedShowId(null);
                  loadShowsCount();
                }}
              />
            ) : (
              <ShowLibrary
                onSelectShow={(id) => {
                  setSelectedShowId(id);
                }}
              />
            )}
          </div>
        )}

        {/* Error Modal / Banner */}
        {error && (
          <div className="fixed bottom-8 right-8 z-50 p-5 max-w-md w-full bg-red-50 border border-red-200 rounded-2xl shadow-lg">
            <p className="text-red-800 text-xs font-medium mb-3">{error}</p>
            <div className="flex justify-end">
              <button
                onClick={() => setError(null)}
                className="px-3 py-1.5 bg-white text-gray-700 text-xs font-semibold border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors shadow-2xs"
              >
                Dismiss
              </button>
            </div>
          </div>
        )}

        {/* Add to Show Modal */}
        {isAddToShowOpen && (
          <AddToShowModal
            isOpen={true}
            onClose={() => setIsAddToShowOpen(false)}
            singleMedia={addToShowTarget === "SINGLE" ? media : null}
            batchJob={addToShowTarget === "BATCH" ? batchJob : null}
            onShowAdded={(showId) => {
              setCurrentTab("SHOWS");
              setSelectedShowId(showId);
              loadShowsCount();
            }}
          />
        )}
      </main>
    </div>
  );
}

export default App;
