import { useState, useMemo } from "react";
import type { BatchAnalysisJob, BatchEpisode, OverallStatus } from "@podready/domain";
import { formatBatchDuration, formatAudioDuration } from "@podready/domain";

interface BatchResultsProps {
  job: BatchAnalysisJob;
  onSelectEpisode: (episode: BatchEpisode) => void;
  onReset: () => void;
  onAddToShow?: () => void;
}

type SortField = "IMPORT_ORDER" | "STATUS" | "FILENAME" | "LOUDNESS" | "TRUE_PEAK";
type FilterStatus = "ALL" | "READY" | "ATTENTION" | "NEEDS_ATTENTION" | "FAILED";

export function BatchResults({ job, onSelectEpisode, onReset, onAddToShow }: BatchResultsProps) {
  const [filter, setFilter] = useState<FilterStatus>("ALL");
  const [sortField, setSortField] = useState<SortField>("IMPORT_ORDER");
  const [sortAsc, setSortAsc] = useState<boolean>(true);


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

  const getStatusBadge = (status?: OverallStatus, isFailed?: boolean) => {
    if (isFailed) {
      return (
        <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-rose-100 text-rose-800">
          Failed
        </span>
      );
    }
    switch (status) {
      case "READY":
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-emerald-100 text-emerald-800">
            Ready
          </span>
        );
      case "ATTENTION":
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-amber-100 text-amber-800">
            Attention
          </span>
        );
      case "NEEDS_ATTENTION":
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-rose-100 text-rose-800">
            Needs Attention
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-600">
            Unknown
          </span>
        );
    }
  };

  const statusPriority = (ep: BatchEpisode): number => {
    if (ep.status === "FAILED") return 4;
    const overall = ep.assessment?.overallStatus;
    if (overall === "NEEDS_ATTENTION") return 3;
    if (overall === "ATTENTION") return 2;
    if (overall === "READY") return 1;
    return 0;
  };

  const filteredEpisodes = useMemo(() => {
    return job.episodes.filter((ep) => {
      if (filter === "ALL") return true;
      if (filter === "FAILED") return ep.status === "FAILED";
      return ep.assessment?.overallStatus === filter;
    });
  }, [job.episodes, filter]);

  const sortedEpisodes = useMemo(() => {
    const list = [...filteredEpisodes];
    if (sortField === "IMPORT_ORDER") {
      return sortAsc ? list : list.reverse();
    }

    list.sort((a, b) => {
      let comparison = 0;
      switch (sortField) {
        case "FILENAME":
          comparison = a.filename.localeCompare(b.filename);
          break;
        case "STATUS":
          comparison = statusPriority(b) - statusPriority(a); // Default worst first
          break;
        case "LOUDNESS":
          const aLoud = a.measurements?.integratedLoudnessLufs ?? -999;
          const bLoud = b.measurements?.integratedLoudnessLufs ?? -999;
          comparison = aLoud - bLoud;
          break;
        case "TRUE_PEAK":
          const aPeak = a.measurements?.truePeakDbtp ?? -999;
          const bPeak = b.measurements?.truePeakDbtp ?? -999;
          comparison = aPeak - bPeak;
          break;
      }
      return sortAsc ? comparison : -comparison;
    });

    return list;
  }, [filteredEpisodes, sortField, sortAsc]);

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
      setSortAsc(field === "STATUS" ? false : true);
    }
  };

  const total = job.summary.total;
  const complete = job.summary.complete;
  const failed = job.summary.failed;
  const ready = job.summary.ready;
  const attention = job.summary.attention;
  const needsAttention = job.summary.needsAttention;
  const elapsed = formatBatchDuration(job.summary.elapsedSeconds);

  return (
    <div className="w-full max-w-4xl bg-white border border-gray-200 rounded-2xl shadow-sm p-8 flex flex-col space-y-6">
      {/* Header & Metrics Summary */}
      <div className="flex flex-col md:flex-row md:items-center justify-between border-b border-gray-100 pb-6 gap-4">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-gray-900">
            {complete} {complete === 1 ? "episode" : "episodes"} analysed
          </h2>
          <p className="text-sm font-medium text-gray-500 mt-1">
            Total analysis time: <span className="font-semibold text-gray-700">{elapsed}</span>
          </p>
        </div>

        <div className="flex items-center space-x-3 self-start md:self-auto">
          {onAddToShow && complete > 0 && (
            <button
              onClick={onAddToShow}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-semibold rounded-lg transition-colors shadow-sm cursor-pointer"
            >
              + Add to Show
            </button>
          )}
          <button
            onClick={onReset}
            className="px-4 py-2 bg-gray-900 text-white text-sm font-medium rounded-lg hover:bg-gray-800 transition-colors shadow-sm"
          >
            Analyse More Episodes
          </button>
        </div>
      </div>


      {/* KPI Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div
          onClick={() => setFilter(filter === "READY" ? "ALL" : "READY")}
          className={`p-4 rounded-xl border transition-all cursor-pointer ${
            filter === "READY"
              ? "border-emerald-500 bg-emerald-50/80 ring-2 ring-emerald-400"
              : "border-emerald-100 bg-emerald-50/40 hover:bg-emerald-50"
          }`}
        >
          <div className="flex items-center space-x-2">
            <span className="w-2 h-2 rounded-full bg-emerald-500" />
            <span className="text-xs font-semibold text-emerald-800 uppercase tracking-wider">
              Ready
            </span>
          </div>
          <p className="text-2xl font-bold text-emerald-950 mt-1">{ready}</p>
        </div>

        <div
          onClick={() => setFilter(filter === "ATTENTION" ? "ALL" : "ATTENTION")}
          className={`p-4 rounded-xl border transition-all cursor-pointer ${
            filter === "ATTENTION"
              ? "border-amber-500 bg-amber-50/80 ring-2 ring-amber-400"
              : "border-amber-100 bg-amber-50/40 hover:bg-amber-50"
          }`}
        >
          <div className="flex items-center space-x-2">
            <span className="w-2 h-2 rounded-full bg-amber-500" />
            <span className="text-xs font-semibold text-amber-800 uppercase tracking-wider">
              Attention
            </span>
          </div>
          <p className="text-2xl font-bold text-amber-950 mt-1">{attention}</p>
        </div>

        <div
          onClick={() => setFilter(filter === "NEEDS_ATTENTION" ? "ALL" : "NEEDS_ATTENTION")}
          className={`p-4 rounded-xl border transition-all cursor-pointer ${
            filter === "NEEDS_ATTENTION"
              ? "border-rose-500 bg-rose-50/80 ring-2 ring-rose-400"
              : "border-rose-100 bg-rose-50/40 hover:bg-rose-50"
          }`}
        >
          <div className="flex items-center space-x-2">
            <span className="w-2 h-2 rounded-full bg-rose-500" />
            <span className="text-xs font-semibold text-rose-800 uppercase tracking-wider">
              Needs Attention
            </span>
          </div>
          <p className="text-2xl font-bold text-rose-950 mt-1">{needsAttention}</p>
        </div>

        {failed > 0 ? (
          <div
            onClick={() => setFilter(filter === "FAILED" ? "ALL" : "FAILED")}
            className={`p-4 rounded-xl border transition-all cursor-pointer ${
              filter === "FAILED"
                ? "border-rose-600 bg-rose-100 ring-2 ring-rose-500"
                : "border-rose-200 bg-rose-50/60 hover:bg-rose-100"
            }`}
          >
            <div className="flex items-center space-x-2">
              <span className="w-2 h-2 rounded-full bg-rose-600" />
              <span className="text-xs font-semibold text-rose-900 uppercase tracking-wider">
                Failed
              </span>
            </div>
            <p className="text-2xl font-bold text-rose-950 mt-1">{failed}</p>
          </div>
        ) : (
          <div className="p-4 rounded-xl border border-gray-100 bg-gray-50/50">
            <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
              Total In Batch
            </span>
            <p className="text-2xl font-bold text-gray-700 mt-1">{total}</p>
          </div>
        )}
      </div>

      {/* Filter & Sort Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 pt-2">
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setFilter("ALL")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "ALL"
                ? "bg-gray-900 text-white"
                : "bg-gray-100 text-gray-600 hover:bg-gray-200"
            }`}
          >
            All ({total})
          </button>
          <button
            onClick={() => setFilter("NEEDS_ATTENTION")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "NEEDS_ATTENTION"
                ? "bg-rose-600 text-white"
                : "bg-rose-50 text-rose-700 hover:bg-rose-100"
            }`}
          >
            Needs Attention ({needsAttention})
          </button>
          <button
            onClick={() => setFilter("ATTENTION")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "ATTENTION"
                ? "bg-amber-600 text-white"
                : "bg-amber-50 text-amber-700 hover:bg-amber-100"
            }`}
          >
            Attention ({attention})
          </button>
          <button
            onClick={() => setFilter("READY")}
            className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
              filter === "READY"
                ? "bg-emerald-600 text-white"
                : "bg-emerald-50 text-emerald-700 hover:bg-emerald-100"
            }`}
          >
            Ready ({ready})
          </button>
          {failed > 0 && (
            <button
              onClick={() => setFilter("FAILED")}
              className={`px-3 py-1 text-xs font-medium rounded-lg transition-colors ${
                filter === "FAILED"
                  ? "bg-rose-700 text-white"
                  : "bg-rose-50 text-rose-800 hover:bg-rose-100"
              }`}
            >
              Failed ({failed})
            </button>
          )}
        </div>

        <div className="flex items-center space-x-2 text-xs text-gray-500">
          <span>Sort:</span>
          <select
            value={sortField}
            onChange={(e) => handleSort(e.target.value as SortField)}
            className="bg-gray-50 border border-gray-200 text-gray-700 text-xs font-medium rounded-md px-2 py-1 focus:outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="IMPORT_ORDER">Import Order</option>
            <option value="STATUS">Assessment Status</option>
            <option value="FILENAME">Filename</option>
            <option value="LOUDNESS">Loudness (LUFS)</option>
            <option value="TRUE_PEAK">True Peak (dBTP)</option>
          </select>
        </div>
      </div>

      {/* Episode Table / Rows */}
      <div className="border border-gray-200 rounded-xl overflow-hidden shadow-xs">
        <div className="grid grid-cols-12 bg-gray-50 px-4 py-2.5 text-xs font-semibold text-gray-500 uppercase tracking-wider border-b border-gray-200">
          <div className="col-span-5">Episode</div>
          <div className="col-span-2 text-center">Duration</div>
          <div className="col-span-2 text-center">Loudness</div>
          <div className="col-span-2 text-center">True Peak</div>
          <div className="col-span-1 text-right">Status</div>
        </div>

        <div className="divide-y divide-gray-100 max-h-96 overflow-y-auto">
          {sortedEpisodes.map((ep) => {
            const isFailed = ep.status === "FAILED";
            return (
              <div
                key={ep.id}
                onClick={() => {
                  if (ep.assessment) {
                    onSelectEpisode(ep);
                  }
                }}
                className={`grid grid-cols-12 items-center px-4 py-3.5 transition-colors ${
                  ep.assessment
                    ? "cursor-pointer hover:bg-blue-50/40"
                    : "cursor-default opacity-85"
                }`}
              >
                {/* Filename & sub info */}
                <div className="col-span-5 pr-2 truncate">
                  <span className="text-sm font-medium text-gray-900 block truncate">
                    {ep.filename}
                  </span>
                  {isFailed && ep.error ? (
                    <span className="text-xs text-rose-600 block truncate" title={ep.error}>
                      {ep.error}
                    </span>
                  ) : (
                    <span className="text-xs text-gray-400 block">
                      {ep.format || "AUDIO"} {ep.codec ? `· ${ep.codec}` : ""}
                    </span>
                  )}
                </div>

                {/* Duration */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-600">
                  {formatAudioDuration(ep.durationSeconds || ep.inspection?.durationSeconds)}
                </div>

                {/* Loudness */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-700">
                  {formatLoudness(ep.measurements?.integratedLoudnessLufs)}
                </div>

                {/* True Peak */}
                <div className="col-span-2 text-center text-sm font-mono text-gray-700">
                  {formatPeak(ep.measurements?.truePeakDbtp)}
                </div>

                {/* Status Badge */}
                <div className="col-span-1 text-right">
                  {getStatusBadge(ep.assessment?.overallStatus, isFailed)}
                </div>
              </div>
            );
          })}

          {sortedEpisodes.length === 0 && (
            <div className="p-8 text-center text-sm text-gray-500">
              No episodes match the selected filter.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
