import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { MediaSource } from "@podready/domain";
import { Dropzone } from "./components/Dropzone";
import { Report } from "./components/Report";

function App() {
  const [media, setMedia] = useState<MediaSource | null>(null);
  const [loadingFile, setLoadingFile] = useState<string | null>(null);
  const [isAnalysing, setIsAnalysing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const handleFileDropped = async (path: string) => {
    // Extract filename from path for loading state
    const filename = path.split(/[/\\]/).pop() || path;
    
    setLoadingFile(filename);
    setError(null);
    setMedia(null);
    setIsAnalysing(false);

    try {
      // Step 1: Inspect media properties via ffprobe
      const inspected = await invoke<MediaSource>("inspect_media_cmd", { path });
      setMedia(inspected);
      setLoadingFile(null);

      // Step 2: Analyse audio measurements via ffmpeg (non-blocking)
      setIsAnalysing(true);
      const measurements = await invoke<any>("analyse_audio_cmd", {
        path,
        durationSeconds: inspected.inspection.durationSeconds,
      });

      setMedia((prev) => (prev ? { ...prev, measurements } : null));
    } catch (err: any) {
      console.error(err);
      setError(err.message || "An unexpected error occurred.");
    } finally {
      setLoadingFile(null);
      setIsAnalysing(false);
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

      {media && <Report media={media} isAnalysing={isAnalysing} />}

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
