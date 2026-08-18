import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface DropzoneProps {
  onFilesDropped: (paths: string[]) => void;
}

export function Dropzone({ onFilesDropped }: DropzoneProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isSelecting, setIsSelecting] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const appWindow = getCurrentWebviewWindow();
        unlisten = await appWindow.onDragDropEvent((event) => {
          if (event.payload.type === "over") {
            setIsHovered(true);
          } else if (event.payload.type === "drop") {
            setIsHovered(false);
            const paths = event.payload.paths;
            if (paths && paths.length > 0) {
              onFilesDropped(paths);
            }
          } else {
            setIsHovered(false);
          }
        });
      } catch (e) {
        console.warn("Failed to attach window drag-drop event, falling back to event listeners:", e);
      }
    };

    setupListener();

    // Fallback direct event listeners
    const unlistenDrop = listen<{ paths?: string[]; position?: any } | string[]>("tauri://drag-drop", (event) => {
      setIsHovered(false);
      const payload = event.payload;
      const paths = Array.isArray(payload) ? payload : payload?.paths;
      if (paths && paths.length > 0) {
        onFilesDropped(paths);
      }
    });

    const unlistenHover = listen("tauri://drag-over", () => setIsHovered(true));
    const unlistenEnter = listen("tauri://drag-enter", () => setIsHovered(true));
    const unlistenLeave = listen("tauri://drag-leave", () => setIsHovered(false));

    return () => {
      if (unlisten) unlisten();
      unlistenDrop.then((fn) => fn());
      unlistenHover.then((fn) => fn());
      unlistenEnter.then((fn) => fn());
      unlistenLeave.then((fn) => fn());
    };
  }, [onFilesDropped]);

  const handleSelectFiles = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isSelecting) return;
    setIsSelecting(true);
    try {
      const selected = await invoke<string[]>("select_files_cmd");
      if (selected && selected.length > 0) {
        onFilesDropped(selected);
      }
    } catch (err) {
      console.error("Failed to select files:", err);
    } finally {
      setIsSelecting(false);
    }
  };

  return (
    <div
      onClick={handleSelectFiles}
      className={`flex flex-col items-center justify-center w-full max-w-md p-12 border-2 border-dashed rounded-2xl cursor-pointer transition-all ${
        isHovered
          ? "border-blue-500 bg-blue-50/80 scale-[1.01]"
          : "border-gray-300 hover:border-gray-400 bg-gray-50/50 hover:bg-gray-50"
      }`}
    >
      <h1 className="text-2xl font-bold tracking-tight text-gray-900 mb-2">PODREADY</h1>
      <p className="text-lg text-gray-700 mb-6 font-medium">Is your episode ready?</p>

      <p className="text-sm text-gray-600 mb-1 font-medium tracking-wide">
        Drop episode(s) here or click to browse
      </p>
      <p className="text-xs text-gray-400 mb-6 tracking-widest uppercase">WAV · MP3 · M4A</p>

      <button
        type="button"
        onClick={handleSelectFiles}
        disabled={isSelecting}
        className="px-4 py-2 bg-white text-gray-700 text-xs font-semibold uppercase tracking-wider border border-gray-300 rounded-lg hover:bg-gray-50 shadow-xs transition-colors mb-6 disabled:opacity-50"
      >
        {isSelecting ? "Opening…" : "Select Audio Files"}
      </button>

      <p className="text-xs text-gray-400">Everything stays on your Mac.</p>
    </div>
  );
}
