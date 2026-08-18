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
} from "@podready/domain";
import { Dropzone } from "./components/Dropzone";
import { Report } from "./components/Report";
import { BatchProgress } from "./components/BatchProgress";
import { BatchResults } from "./components/BatchResults";

function App() {
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

  const batchJobRef = useRef<BatchAnalysisJob | null>(null);
  batchJobRef.current = batchJob;

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
    <main className="min-h-screen bg-gray-50 flex items-center justify-center p-8 font-sans">
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
        <Report
          media={media}
          isAnalysing={isAnalysing}
          isProcessing={isProcessing}
          processingResponse={processingResponse}
          onProcessAudio={handleProcessAudio}
          isExporting={isExporting}
          exportResult={exportResult}
          onExport={handleExportPackage}
        />
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

      {/* Error Modal / Banner */}
      {error && (
        <div className="mt-8 p-6 max-w-md w-full bg-red-50 border border-red-200 rounded-xl">
          <p className="text-red-800 font-medium text-center mb-4">{error}</p>
          <div className="flex justify-center">
            <button
              onClick={() => setError(null)}
              className="px-4 py-2 bg-white text-gray-700 text-sm font-medium border border-gray-300 rounded hover:bg-gray-50 transition-colors"
            >
              TRY AGAIN
            </button>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
