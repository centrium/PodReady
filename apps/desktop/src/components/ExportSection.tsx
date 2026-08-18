import { useState } from "react";
import type {
  MediaSource,
  ProcessAudioResponse,
  PodReadyPackage,
  ExportOptions,
  EpisodeMetadata,
} from "@podready/domain";

interface ExportSectionProps {
  media: MediaSource;
  processingResponse?: ProcessAudioResponse | null;
  isExporting: boolean;
  exportResult: PodReadyPackage | null;
  onExport: (options: ExportOptions) => Promise<void>;
}

export function ExportSection({
  media,
  processingResponse,
  isExporting,
  exportResult,
  onExport,
}: ExportSectionProps) {
  // Use verified candidate measurements if available, else source measurements
  const activeMeasurements =
    processingResponse?.afterMeasurements || media.measurements;
  const activeAssessment =
    processingResponse?.afterAssessment || media.assessment;

  // Extract source stem for default metadata
  const defaultTitle = media.filename.replace(/\.[^/.]+$/, "");
  // Infer parent directory from media.path
  const pathParts = media.path.split(/[/\\]/);
  pathParts.pop();
  const defaultDest = pathParts.join("/") || "/Users/matt/Desktop";

  const [destinationDirectory, setDestinationDirectory] = useState(defaultDest);
  const [includeAudio, setIncludeAudio] = useState(true);
  const [includeTranscript, setIncludeTranscript] = useState(true);
  const [includeReport, setIncludeReport] = useState(true);

  // Metadata inputs
  const [title, setTitle] = useState(defaultTitle);
  const [artist, setArtist] = useState("");
  const [episodeNumber, setEpisodeNumber] = useState("");
  const [year, setYear] = useState(new Date().getFullYear().toString());
  const [artworkPath, setArtworkPath] = useState("");
  const [transcriptText, setTranscriptText] = useState("");
  const [showMetadataForm, setShowMetadataForm] = useState(false);

  const formatLoudness = (val: number | null | undefined) => {
    if (val === null || val === undefined || isNaN(val)) return "—";
    const sign = val < 0 ? "−" : "";
    return `${sign}${Math.abs(val).toFixed(1)} LUFS`;
  };

  const formatPeak = (val: number | null | undefined) => {
    if (val === null || val === undefined || isNaN(val)) return "—";
    const sign = val < 0 ? "−" : "";
    return `${sign}${Math.abs(val).toFixed(1)} dBTP`;
  };

  const handleExportClick = () => {
    const metadata: EpisodeMetadata = {
      title: title.trim() || undefined,
      artist: artist.trim() || undefined,
      episodeNumber: episodeNumber.trim() || undefined,
      year: year.trim() || undefined,
      artworkPath: artworkPath.trim() || undefined,
    };

    onExport({
      destinationDirectory,
      includeAudio,
      includeTranscript,
      includeReport,
      metadata,
      transcriptText: transcriptText.trim()
        ? transcriptText
        : `Spoken transcript for ${media.filename}\nExtracted by PodReady.`,
    });
  };

  return (
    <div className="p-5 bg-white border border-gray-200 rounded-2xl shadow-sm space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-gray-100">
        <div className="flex items-center space-x-2">
          <span className="w-2.5 h-2.5 rounded-full bg-indigo-600" />
          <h4 className="text-xs font-bold text-gray-900 tracking-wider uppercase">
            Ready to Export
          </h4>
        </div>
        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold bg-indigo-100 text-indigo-800">
          PodReady Pack
        </span>
      </div>

      {/* Audio Verification Summary */}
      <div className="p-3.5 bg-emerald-50/60 border border-emerald-100 rounded-xl space-y-2">
        <div className="flex items-center space-x-1.5 text-xs font-bold text-emerald-900">
          <span>✓</span>
          <span>Audio Verified for Publication</span>
        </div>
        <div className="grid grid-cols-2 gap-2 text-xs font-mono text-emerald-950 pt-1">
          <div>
            <span className="text-emerald-700 text-[11px] block">Loudness:</span>
            {formatLoudness(activeMeasurements?.integratedLoudnessLufs)}
          </div>
          <div>
            <span className="text-emerald-700 text-[11px] block">Peak:</span>
            {formatPeak(activeMeasurements?.truePeakDbtp)}
          </div>
        </div>
      </div>

      {/* Package Contents Checklist */}
      <div className="space-y-2">
        <h5 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
          Package Contents
        </h5>
        <div className="space-y-1.5 text-xs text-gray-800">
          <label className="flex items-center space-x-2 p-2 bg-gray-50 rounded-lg hover:bg-gray-100/70 transition-colors cursor-pointer">
            <input
              type="checkbox"
              checked={includeAudio}
              onChange={(e) => setIncludeAudio(e.target.checked)}
              className="rounded text-indigo-600 focus:ring-indigo-500"
            />
            <span className="font-semibold text-gray-900">Publishing MP3 Audio</span>
            <span className="text-gray-500 text-[11px]">
              ({activeAssessment?.profileName?.includes("Mono") ? "128 kbps" : "192 kbps"} stereo/mono)
            </span>
          </label>

          <label className="flex items-center space-x-2 p-2 bg-gray-50 rounded-lg hover:bg-gray-100/70 transition-colors cursor-pointer">
            <input
              type="checkbox"
              checked={includeTranscript}
              onChange={(e) => setIncludeTranscript(e.target.checked)}
              className="rounded text-indigo-600 focus:ring-indigo-500"
            />
            <span className="font-semibold text-gray-900">Transcript companion (.txt)</span>
          </label>

          <label className="flex items-center space-x-2 p-2 bg-gray-50 rounded-lg hover:bg-gray-100/70 transition-colors cursor-pointer">
            <input
              type="checkbox"
              checked={includeReport}
              onChange={(e) => setIncludeReport(e.target.checked)}
              className="rounded text-indigo-600 focus:ring-indigo-500"
            />
            <span className="font-semibold text-gray-900">Verification Report (.json)</span>
          </label>
        </div>
      </div>

      {/* Optional Metadata Toggle & Summary */}
      <div className="pt-2 border-t border-gray-100 space-y-2">
        <div className="flex items-center justify-between">
          <h5 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
            Episode Details & Artwork
          </h5>
          <button
            type="button"
            onClick={() => setShowMetadataForm(!showMetadataForm)}
            className="text-xs text-indigo-600 hover:text-indigo-800 font-semibold"
          >
            {showMetadataForm ? "Close Details" : "Edit Details (Optional)"}
          </button>
        </div>

        {!showMetadataForm ? (
          <div className="p-3 bg-gray-50 rounded-xl text-xs space-y-1 text-gray-600">
            <div className="flex justify-between">
              <span>Title:</span>
              <span className="font-medium text-gray-900 truncate max-w-[200px]">
                {title || defaultTitle}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Podcast Name:</span>
              <span className="font-medium text-gray-900">
                {artist || "Not specified"}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Episode Number:</span>
              <span className="font-medium text-gray-900">
                {episodeNumber || "Not specified"}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Cover Artwork:</span>
              <span className="font-medium text-gray-900">
                {artworkPath ? "Provided" : "Not provided"}
              </span>
            </div>
          </div>
        ) : (
          <div className="p-3.5 bg-gray-50 rounded-xl space-y-2.5 text-xs text-gray-700">
            <div>
              <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                Episode Title
              </label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={defaultTitle}
                className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none"
              />
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                  Podcast / Show Name
                </label>
                <input
                  type="text"
                  value={artist}
                  onChange={(e) => setArtist(e.target.value)}
                  placeholder="e.g. The Tech Wave"
                  className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                  Episode Number
                </label>
                <input
                  type="text"
                  value={episodeNumber}
                  onChange={(e) => setEpisodeNumber(e.target.value)}
                  placeholder="e.g. 37"
                  className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                  Year
                </label>
                <input
                  type="text"
                  value={year}
                  onChange={(e) => setYear(e.target.value)}
                  placeholder="2026"
                  className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                  Artwork Image Path
                </label>
                <input
                  type="text"
                  value={artworkPath}
                  onChange={(e) => setArtworkPath(e.target.value)}
                  placeholder="/path/to/cover.jpg"
                  className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none"
                />
              </div>
            </div>

            {includeTranscript && (
              <div>
                <label className="block text-[11px] font-semibold text-gray-600 mb-1">
                  Transcript text (optional custom text)
                </label>
                <textarea
                  value={transcriptText}
                  onChange={(e) => setTranscriptText(e.target.value)}
                  placeholder="Spoken words..."
                  rows={2}
                  className="w-full px-2.5 py-1.5 bg-white border border-gray-200 rounded-lg text-xs focus:ring-1 focus:ring-indigo-500 focus:outline-none font-mono"
                />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Destination Directory */}
      <div className="space-y-1.5 pt-1">
        <label className="block text-[11px] font-bold text-gray-400 uppercase tracking-widest">
          Export Destination
        </label>
        <input
          type="text"
          value={destinationDirectory}
          onChange={(e) => setDestinationDirectory(e.target.value)}
          placeholder="/path/to/export/folder"
          className="w-full px-3 py-1.5 bg-gray-50 border border-gray-200 rounded-xl text-xs font-mono text-gray-800 focus:bg-white focus:ring-1 focus:ring-indigo-500 focus:outline-none"
        />
      </div>

      {/* Export Action Button */}
      <div className="pt-2">
        <button
          type="button"
          onClick={handleExportClick}
          disabled={isExporting || (!includeAudio && !includeTranscript && !includeReport)}
          className={`w-full py-3 px-4 text-xs font-bold tracking-wider rounded-xl uppercase transition-colors ${
            isExporting
              ? "bg-indigo-400 text-white cursor-wait"
              : "bg-indigo-600 hover:bg-indigo-700 text-white shadow-sm cursor-pointer"
          }`}
        >
          {isExporting ? "Creating PodReady Package…" : "Export PodReady Package"}
        </button>
      </div>

      {/* Successful Export Result View */}
      {exportResult && (
        <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl space-y-3 pt-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-1.5 text-xs font-bold text-emerald-900">
              <span className="w-2 h-2 rounded-full bg-emerald-500" />
              <span>Package Exported Successfully ✓</span>
            </div>
            <span className="text-[11px] font-semibold text-emerald-700">
              Verified Ready
            </span>
          </div>

          <div className="text-xs text-gray-800 space-y-1 bg-white/80 p-3 rounded-lg border border-emerald-100 font-mono">
            <div className="font-bold text-gray-900 truncate">
              📁 {exportResult.packageName}
            </div>
            {exportResult.audioFile && (
              <div className="text-emerald-900 truncate pl-3">
                🎵 {exportResult.audioFile.filename} ({(exportResult.audioFile.fileSizeBytes / 1024 / 1024).toFixed(2)} MB)
              </div>
            )}
            {exportResult.transcriptFile && (
              <div className="text-indigo-900 truncate pl-3">
                📝 {exportResult.transcriptFile.filename}
              </div>
            )}
            {exportResult.reportFile && (
              <div className="text-slate-900 truncate pl-3">
                📊 {exportResult.reportFile.filename}
              </div>
            )}
          </div>

          <p className="text-[11px] text-gray-500 font-mono break-all">
            Saved to: {exportResult.packageDirectory}
          </p>
        </div>
      )}
    </div>
  );
}
