import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  MediaSource,
  AudioMeasurements,
  Assessment,
  FixPlan,
  ProcessAudioResponse,
} from "@podready/domain";
import { Dropzone } from "./components/Dropzone";
import { Report } from "./components/Report";

function App() {
  const [media, setMedia] = useState<MediaSource | null>(null);
  const [loadingFile, setLoadingFile] = useState<string | null>(null);
  const [isAnalysing, setIsAnalysing] = useState<boolean>(false);
  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [processingResponse, setProcessingResponse] = useState<ProcessAudioResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleFileDropped = async (path: string) => {
    // Extract filename from path for loading state
    const filename = path.split(/[/\\]/).pop() || path;

    setLoadingFile(filename);
    setError(null);
    setMedia(null);
    setIsAnalysing(false);
    setIsProcessing(false);
    setProcessingResponse(null);

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

  return (
    <main className="min-h-screen bg-gray-50 flex items-center justify-center p-8 font-sans">
      {!loadingFile && !media && <Dropzone onFileDropped={handleFileDropped} />}

      {loadingFile && (
        <div className="flex flex-col items-center justify-center space-y-4">
          <h2 className="text-xl font-medium text-gray-900">{loadingFile}</h2>
          <p className="text-gray-500 animate-pulse">Checking your episode…</p>
        </div>
      )}

      {media && (
        <Report
          media={media}
          isAnalysing={isAnalysing}
          isProcessing={isProcessing}
          processingResponse={processingResponse}
          onProcessAudio={handleProcessAudio}
        />
      )}

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

