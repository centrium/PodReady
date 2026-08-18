import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CatalogueEpisode } from "@podready/domain";

interface BatchPublishingPreflightModalProps {
  isOpen: boolean;
  showName?: string;
  selectedEpisodes: CatalogueEpisode[];
  onClose: () => void;
  onStartPublishing: (destinationDirectory: string) => Promise<void>;
}

export function BatchPublishingPreflightModal({
  isOpen,
  showName,
  selectedEpisodes,
  onClose,
  onStartPublishing,
}: BatchPublishingPreflightModalProps) {
  // Infer a sensible default destination directory
  const defaultDir = (() => {
    if (selectedEpisodes.length > 0) {
      const firstPath = selectedEpisodes[0].sourcePath;
      const parts = firstPath.split(/[/\\]/);
      parts.pop();
      const parentDir = parts.join("/");
      const sanitizedShow = (showName || "PodReady").replace(/[^a-zA-Z0-9_-]/g, "_");
      return `${parentDir}/${sanitizedShow}_PodReady`;
    }
    return "~/Desktop/PodReady_Export";
  })();

  const [destinationDirectory, setDestinationDirectory] = useState<string>(defaultDir);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen || selectedEpisodes.length === 0) return null;

  const total = selectedEpisodes.length;
  const availableCount = selectedEpisodes.filter(
    (e) => e.sourceAvailability === "AVAILABLE"
  ).length;
  const changedCount = selectedEpisodes.filter(
    (e) => e.sourceAvailability === "CHANGED"
  ).length;
  const missingCount = selectedEpisodes.filter(
    (e) => e.sourceAvailability === "MISSING"
  ).length;

  const allMissing = missingCount === total;

  const handleBrowseDestination = async () => {
    try {
      const chosen = await invoke<string | null>("select_destination_directory_cmd");
      if (chosen) {
        setDestinationDirectory(chosen);
      }
    } catch (err: any) {
      console.error("Failed to select destination folder:", err);
    }
  };

  const handleConfirm = async () => {
    if (!destinationDirectory.trim()) {
      setError("Please specify an export destination directory.");
      return;
    }
    setIsStarting(true);
    setError(null);
    try {
      await onStartPublishing(destinationDirectory.trim());
    } catch (err: any) {
      console.error("Failed to start batch publishing:", err);
      setError(err.message || "Failed to start publishing.");
      setIsStarting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4">
      <div className="bg-white rounded-2xl border border-gray-200 shadow-xl max-w-lg w-full p-6 space-y-5">
        {/* Header */}
        <div className="flex items-center justify-between pb-3 border-b border-gray-100">
          <div>
            <h3 className="text-base font-bold text-gray-900 tracking-tight">
              {total === 1
                ? "Make 1 Episode PodReady"
                : `Make ${total} Episodes PodReady`}
            </h3>
            {showName && (
              <p className="text-xs font-medium text-gray-500 mt-0.5">
                From show: <strong className="text-gray-700">{showName}</strong>
              </p>
            )}
          </div>
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold bg-indigo-100 text-indigo-800">
            Publishing
          </span>
        </div>

        {/* Source State Breakdown */}
        <div className="space-y-2">
          <h4 className="text-[11px] font-bold uppercase tracking-wider text-gray-400">
            Preflight Summary
          </h4>
          <div className="p-3.5 bg-gray-50 rounded-xl border border-gray-100 space-y-2 text-xs">
            {availableCount > 0 && (
              <div className="flex items-center space-x-2 text-emerald-800">
                <span className="font-bold">✓</span>
                <span>
                  <strong>{availableCount}</strong> {availableCount === 1 ? "episode" : "episodes"} ready to publish
                </span>
              </div>
            )}
            {changedCount > 0 && (
              <div className="flex items-center space-x-2 text-amber-800">
                <span className="font-bold">⚠</span>
                <span>
                  <strong>{changedCount}</strong> changed {changedCount === 1 ? "source" : "sources"} — will be re-analysed before publishing
                </span>
              </div>
            )}
            {missingCount > 0 && (
              <div className="flex items-center space-x-2 text-rose-700">
                <span className="font-bold">○</span>
                <span>
                  <strong>{missingCount}</strong> missing {missingCount === 1 ? "source" : "sources"} — will be skipped
                </span>
              </div>
            )}
          </div>
        </div>

        {/* Destination Directory Selection */}
        <div className="space-y-2">
          <label className="block text-[11px] font-bold uppercase tracking-wider text-gray-400">
            Destination Directory
          </label>
          <div className="flex items-center space-x-2">
            <input
              type="text"
              value={destinationDirectory}
              onChange={(e) => setDestinationDirectory(e.target.value)}
              placeholder="/path/to/destination"
              className="flex-1 px-3 py-2 bg-gray-50 border border-gray-200 rounded-xl text-xs font-mono text-gray-800 focus:bg-white focus:ring-1 focus:ring-indigo-500 focus:outline-none"
            />
            <button
              type="button"
              onClick={handleBrowseDestination}
              className="px-3 py-2 text-xs font-semibold text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-xl transition-colors shrink-0"
            >
              Browse…
            </button>
          </div>
          <p className="text-[11px] text-gray-400">
            Independent episode packages will be created inside this folder.
          </p>
        </div>

        {/* Non-destructive invariant notice */}
        <div className="p-3 bg-indigo-50/50 border border-indigo-100 rounded-xl text-xs text-indigo-950 flex items-start space-x-2">
          <span className="text-sm">🛡️</span>
          <p className="text-[11px] leading-relaxed">
            <strong>Source media is preserved:</strong> Original files will never be modified, renamed, or moved.
          </p>
        </div>

        {error && (
          <div className="p-3 bg-rose-50 border border-rose-200 rounded-xl text-xs text-rose-700">
            {error}
          </div>
        )}

        {/* Action Buttons */}
        <div className="flex items-center justify-end space-x-3 pt-2 border-t border-gray-100">
          <button
            type="button"
            onClick={onClose}
            disabled={isStarting}
            className="px-4 py-2 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleConfirm}
            disabled={isStarting || allMissing || !destinationDirectory.trim()}
            className="px-4 py-2 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs disabled:opacity-50 flex items-center space-x-1.5"
          >
            {isStarting ? (
              <span>Starting…</span>
            ) : (
              <span>Make {total === 1 ? "PodReady" : `${total} Episodes PodReady`}</span>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
