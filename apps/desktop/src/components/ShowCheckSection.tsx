import React, { useState } from "react";
import type {
  ShowCheck,
  ShowCheckContinuousMetric,
} from "@podready/domain";
import {
  formatShowCheckLoudness,
  formatShowCheckPeak,
  formatShowCheckDuration,
  formatShowCheckSilence,
  formatShowCheckBitrate,
  formatSampleRateDisplay,
  formatChannelDisplay,
} from "@podready/domain";
import { BulletSparkline } from "sexy-sparklines";

interface ShowCheckSectionProps {
  showCheck: ShowCheck | null;
  isLoading?: boolean;
}

export const ShowCheckSection: React.FC<ShowCheckSectionProps> = ({
  showCheck,
  isLoading = false,
}) => {
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);

  if (isLoading) {
    return (
      <div className="bg-slate-50/80 border border-slate-200/90 rounded-xl p-5 animate-pulse text-center">
        <div className="flex items-center justify-center space-x-2 text-slate-500 text-xs font-medium">
          <span>Comparing episode with Show baseline…</span>
        </div>
      </div>
    );
  }

  if (!showCheck) return null;

  if (
    showCheck.status === "INSUFFICIENT_DATA" ||
    showCheck.baselineEpisodeCount === 0
  ) {
    return (
      <div className="bg-slate-50/70 border border-slate-200/90 rounded-xl p-5 space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <span className="text-xs font-bold text-slate-700 uppercase tracking-wider">
              Show Check · {showCheck.showName}
            </span>
            <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-gray-100 text-gray-600">
              Not Enough History
            </span>
          </div>
        </div>
        <p className="text-xs text-slate-500">
          This show has no baseline history yet. Add analysed episodes to build a comparison baseline.
        </p>
      </div>
    );
  }

  const {
    showName,
    baselineMaturity,
    baselineEpisodeCount,
    status,
    summary,
    isStale,
    metrics,
    categoricalMetrics,
  } = showCheck;

  const getStatusBadge = () => {
    switch (status) {
      case "TYPICAL":
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-sky-100/90 text-sky-800 border border-sky-200">
            Typical
          </span>
        );
      case "DIFFERENT":
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-violet-100/90 text-violet-800 border border-violet-200">
            Different
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-gray-100 text-gray-700 border border-gray-200">
            Not Enough History
          </span>
        );
    }
  };

  const getMaturityBadge = () => {
    switch (baselineMaturity) {
      case "ESTABLISHED":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium bg-emerald-50 text-emerald-700 border border-emerald-200/60">
            Established Baseline ({baselineEpisodeCount} eps)
          </span>
        );
      case "DEVELOPING":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium bg-sky-50 text-sky-700 border border-sky-200/60">
            Baseline Developing ({baselineEpisodeCount} eps)
          </span>
        );
      case "EARLY":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium bg-amber-50 text-amber-700 border border-amber-200/60">
            Early Baseline ({baselineEpisodeCount} eps)
          </span>
        );
      default:
        return null;
    }
  };

  // Primary Continuous Metrics (Loudness, True Peak, Duration)
  const isPrimaryContinuous = (id: string) =>
    id === "loudness" || id === "truePeak" || id === "true_peak" || id === "duration";

  const primaryMetrics = metrics.filter((m) => isPrimaryContinuous(m.id));
  const secondaryMetrics = metrics.filter((m) => !isPrimaryContinuous(m.id));

  // Categorical Metrics
  const channelMetric = categoricalMetrics.find((c) => c.id === "channels");
  const formatMetric = categoricalMetrics.find((c) => c.id === "format");
  const sampleRateMetric = categoricalMetrics.find(
    (c) => c.id === "sampleRate" || c.id === "sample_rate"
  );
  const codecMetric = categoricalMetrics.find((c) => c.id === "codec");

  // Determine if codec is distinct from container format
  const isCodecDistinct =
    codecMetric &&
    formatMetric &&
    codecMetric.candidateValue.toUpperCase() !== formatMetric.candidateValue.toUpperCase() &&
    !codecMetric.candidateValue.toUpperCase().includes(formatMetric.candidateValue.toUpperCase());

  // Count secondary differences for disclosure badge
  const secondaryDifferencesCount =
    secondaryMetrics.filter((m) => m.status === "DIFFERENT").length +
    (isCodecDistinct && codecMetric?.status === "DIFFERENT" ? 1 : 0);

  const renderPrimaryMetric = (metric: ShowCheckContinuousMetric) => {
    let formattedCandidate = `${metric.candidateValue.toFixed(1)} ${metric.unit}`;
    let formattedRange = `${metric.usualLow.toFixed(1)} → ${metric.usualHigh.toFixed(1)} ${metric.unit}`;

    if (metric.id === "loudness") {
      const f = formatShowCheckLoudness(metric);
      formattedCandidate = f.candidate;
      formattedRange = f.range;
    } else if (metric.id === "truePeak" || metric.id === "true_peak") {
      const f = formatShowCheckPeak(metric);
      formattedCandidate = f.candidate;
      formattedRange = f.range;
    } else if (metric.id === "duration") {
      const f = formatShowCheckDuration(metric);
      formattedCandidate = f.candidate;
      formattedRange = f.range;
    }

    const isMetricDifferent = metric.status === "DIFFERENT";
    const isMetricSlightlyDifferent = metric.status === "SLIGHTLY_DIFFERENT";

    return (
      <div
        key={metric.id}
        className="bg-white/90 border border-slate-200/80 rounded-lg p-3.5 space-y-2 shadow-2xs"
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <span className="text-xs font-bold text-slate-800">{metric.label}</span>
            {isMetricDifferent ? (
              <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-violet-50 text-violet-700 border border-violet-200">
                Different
              </span>
            ) : isMetricSlightlyDifferent ? (
              <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-50 text-amber-700 border border-amber-200">
                Slightly Different
              </span>
            ) : (
              <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-slate-100 text-slate-600">
                Typical
              </span>
            )}
          </div>
          <span className="text-xs font-bold text-slate-900">{formattedCandidate}</span>
        </div>

        <div className="flex items-center justify-between text-[11px] text-slate-500">
          <span>
            Usually: <strong className="text-slate-700">{formattedRange}</strong>
          </span>
        </div>

        {metric.sparkline && (
          <div className="pt-0.5">
            <BulletSparkline
              value={metric.sparkline.value}
              min={metric.sparkline.min}
              max={metric.sparkline.max}
              target={metric.sparkline.target}
              ranges={metric.sparkline.ranges}
              height={12}
              theme="minimal"
              aria-label={`${metric.label} comparison chart`}
            />
          </div>
        )}

        <p className="text-[11px] text-slate-600 italic">{metric.message}</p>
      </div>
    );
  };

  const renderSecondaryMetric = (metric: ShowCheckContinuousMetric) => {
    let formattedCandidate = `${metric.candidateValue.toFixed(1)} ${metric.unit}`;
    let formattedTypical = `${metric.typicalValue.toFixed(1)} ${metric.unit}`;
    let formattedRange = `${metric.usualLow.toFixed(1)} → ${metric.usualHigh.toFixed(1)} ${metric.unit}`;

    if (metric.id.includes("silence") || metric.id.includes("Silence")) {
      const f = formatShowCheckSilence(metric);
      formattedCandidate = f.candidate;
      formattedTypical = f.typical;
      formattedRange = f.range;
    } else if (metric.id === "bitrate") {
      const f = formatShowCheckBitrate(metric);
      formattedCandidate = f.candidate;
      formattedTypical = f.typical;
      formattedRange = f.range;
    }

    const isDifferent = metric.status === "DIFFERENT";
    const isSlightlyDifferent = metric.status === "SLIGHTLY_DIFFERENT";

    return (
      <div
        key={metric.id}
        className="flex flex-col sm:flex-row sm:items-center justify-between py-2 border-b border-slate-100 last:border-0 text-xs gap-1"
      >
        <div className="flex items-center space-x-2">
          <span className="font-medium text-slate-700">{metric.label}</span>
          {isDifferent && (
            <span className="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] font-semibold bg-violet-50 text-violet-700 border border-violet-200">
              Different
            </span>
          )}
          {isSlightlyDifferent && (
            <span className="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] font-semibold bg-amber-50 text-amber-700 border border-amber-200">
              Slightly Different
            </span>
          )}
        </div>
        <div className="flex items-center space-x-3 text-slate-600 text-[11px]">
          <span>
            <strong className="text-slate-800">{formattedCandidate}</strong>
          </span>
          <span className="text-slate-400">·</span>
          {metric.id === "bitrate" ? (
            <span>
              Show typically <strong className="text-slate-700">{formattedTypical}</strong> (Usually {formattedRange})
            </span>
          ) : (
            <span>Usually {formattedRange}</span>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="bg-slate-50/90 border border-slate-200 rounded-xl p-5 space-y-4 shadow-2xs">
      {/* Header */}
      <div className="flex flex-col gap-2 border-b border-slate-200/80 pb-3">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div className="flex items-center space-x-2">
            <h3 className="text-xs font-bold text-slate-800 uppercase tracking-wider">
              Show Check · {showName}
            </h3>
            {getStatusBadge()}
          </div>
          <div>{getMaturityBadge()}</div>
        </div>

        {/* Deterministic Headline Summary */}
        <div className="bg-white/80 border border-slate-200/60 rounded-lg px-3 py-2">
          <p className="text-xs font-medium text-slate-800 leading-relaxed">{summary}</p>
        </div>
      </div>

      {/* Stale Warning if source changed */}
      {isStale && (
        <div className="p-3 bg-amber-50 border border-amber-200/80 rounded-lg flex items-start space-x-2">
          <span className="text-amber-700 text-xs">⚠️</span>
          <p className="text-xs text-amber-800 font-medium">
            Note: This comparison is based on stored previous analysis and may be out of date because the source file has changed.
          </p>
        </div>
      )}

      {/* Primary Audio Characteristics */}
      {primaryMetrics.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-[11px] font-bold text-slate-600 uppercase tracking-wider">
            Audio Characteristics
          </h4>
          <div className="grid grid-cols-1 gap-2.5">
            {primaryMetrics.map((m) => renderPrimaryMetric(m))}
          </div>
        </div>
      )}

      {/* Delivery Characteristics (Compact) */}
      {(channelMetric || formatMetric || sampleRateMetric) && (
        <div className="space-y-2 pt-1">
          <h4 className="text-[11px] font-bold text-slate-600 uppercase tracking-wider">
            Delivery
          </h4>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
            {/* Channel Configuration */}
            {channelMetric && (
              <div className="bg-white/90 border border-slate-200/80 rounded-lg p-3 space-y-1 shadow-2xs">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-700">Channels</span>
                  <span
                    className={`text-xs font-bold ${
                      channelMetric.status === "TYPICAL"
                        ? "text-slate-800"
                        : "text-violet-800 font-semibold"
                    }`}
                  >
                    {formatChannelDisplay(channelMetric.candidateValue)}
                  </span>
                </div>
                <p className="text-[11px] text-slate-500">{channelMetric.message}</p>
              </div>
            )}

            {/* Format & Sample Rate */}
            {(formatMetric || sampleRateMetric) && (
              <div className="bg-white/90 border border-slate-200/80 rounded-lg p-3 space-y-1 shadow-2xs">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-700">Format & Rate</span>
                  <span className="text-xs font-bold text-slate-800">
                    {formatMetric?.candidateValue || "—"} ·{" "}
                    {formatSampleRateDisplay(sampleRateMetric?.candidateValue)}
                  </span>
                </div>
                <p className="text-[11px] text-slate-500">
                  {formatMetric?.status === "TYPICAL" && sampleRateMetric?.status === "TYPICAL"
                    ? "Matches usual delivery"
                    : [formatMetric?.message, sampleRateMetric?.message]
                        .filter(Boolean)
                        .join(" ")}
                </p>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Secondary Characteristics Disclosure ("Technical Details") */}
      {secondaryMetrics.length > 0 && (
        <div className="pt-2 border-t border-slate-200/60">
          <button
            type="button"
            onClick={() => setShowTechnicalDetails(!showTechnicalDetails)}
            className="flex items-center space-x-2 text-xs font-medium text-slate-600 hover:text-slate-900 transition-colors focus:outline-hidden"
            aria-expanded={showTechnicalDetails}
          >
            <span>{showTechnicalDetails ? "▾ Hide technical details" : "▸ Technical details"}</span>
            {secondaryDifferencesCount > 0 && (
              <span className="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] font-semibold bg-violet-50 text-violet-700 border border-violet-200">
                {secondaryDifferencesCount} {secondaryDifferencesCount === 1 ? "difference" : "differences"}
              </span>
            )}
          </button>

          {showTechnicalDetails && (
            <div className="mt-3 bg-white/70 border border-slate-200/70 rounded-lg p-3 space-y-1">
              {secondaryMetrics.map((m) => renderSecondaryMetric(m))}
              {isCodecDistinct && codecMetric && (
                <div className="flex flex-col sm:flex-row sm:items-center justify-between py-2 border-b border-slate-100 last:border-0 text-xs gap-1">
                  <div className="flex items-center space-x-2">
                    <span className="font-medium text-slate-700">Codec</span>
                    {codecMetric.status === "DIFFERENT" && (
                      <span className="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] font-semibold bg-violet-50 text-violet-700 border border-violet-200">
                        Different
                      </span>
                    )}
                  </div>
                  <div className="flex items-center space-x-3 text-slate-600 text-[11px]">
                    <strong className="text-slate-800">{codecMetric.candidateValue}</strong>
                    <span className="text-slate-400">·</span>
                    <span>Show usually {codecMetric.typicalValue}</span>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

