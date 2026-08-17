import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface DropzoneProps {
  onFileDropped: (path: string) => void;
}

export function Dropzone({ onFileDropped }: DropzoneProps) {
  const [isHovered, setIsHovered] = useState(false);

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
              onFileDropped(paths[0]);
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
        onFileDropped(paths[0]);
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
  }, [onFileDropped]);

  return (
    <div
      className={`flex flex-col items-center justify-center w-full max-w-md p-12 border-2 border-dashed rounded-xl transition-colors ${
        isHovered
          ? "border-blue-500 bg-blue-50"
          : "border-gray-300 hover:border-gray-400 bg-gray-50"
      }`}
    >
      <h1 className="text-2xl font-bold tracking-tight text-gray-900 mb-2">PODREADY</h1>
      <p className="text-lg text-gray-700 mb-6 font-medium">Is your episode ready?</p>
      
      <p className="text-sm text-gray-500 mb-2 font-medium tracking-wide">Drop an episode here</p>
      <p className="text-xs text-gray-400 mb-8 tracking-widest uppercase">WAV · MP3 · M4A</p>
      
      <p className="text-xs text-gray-400">Everything stays on your Mac.</p>
    </div>
  );
}
