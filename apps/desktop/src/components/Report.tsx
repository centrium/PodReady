import type {
  MediaSource,
  AssessmentStatus,
  OverallStatus,
  ProcessAudioResponse,
} from "@podready/domain";
import { BulletSparkline } from "sexy-sparklines";

interface ReportProps {
  media: MediaSource;
  isAnalysing?: boolean;
  isProcessing?: boolean;
  processingResponse?: ProcessAudioResponse | null;
  onProcessAudio?: () => void;
}

export function Report({
  media,
  isAnalysing,
  isProcessing,
  processingResponse,
  onProcessAudio,
}: ReportProps) {
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

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

  const getStatusBadge = (status: AssessmentStatus) => {
    switch (status) {
      case "GOOD":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-emerald-100 text-emerald-800">
            Good
          </span>
        );
      case "ATTENTION":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-amber-100 text-amber-800">
            Attention
          </span>
        );
      case "ISSUE":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold bg-rose-100 text-rose-800">
            Issue
          </span>
        );
      case "INFO":
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-slate-100 text-slate-700">
            Info
          </span>
        );
      case "UNKNOWN":
      default:
        return (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-600">
            Unknown
          </span>
        );
    }
  };

  const getOverallStatusBanner = (overallStatus: OverallStatus, summary: string) => {
    switch (overallStatus) {
      case "READY":
        return (
          <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-emerald-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-emerald-900 uppercase">
                  Ready
                </h3>
              </div>
              <span className="text-xs font-medium text-emerald-700">
                {summary}
              </span>
            </div>
          </div>
        );
      case "ATTENTION":
        return (
          <div className="p-4 bg-amber-50 border border-amber-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-amber-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-amber-900 uppercase">
                  Attention
                </h3>
              </div>
              <span className="text-xs font-semibold text-amber-800">
                {summary}
              </span>
            </div>
          </div>
        );
      case "NEEDS_ATTENTION":
        return (
          <div className="p-4 bg-rose-50 border border-rose-200 rounded-xl">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-rose-500"></span>
                <h3 className="text-sm font-bold tracking-wider text-rose-900 uppercase">
                  Needs Attention
                </h3>
              </div>
              <span className="text-xs font-semibold text-rose-800">
                {summary}
              </span>
            </div>
          </div>
        );
    }
  };

  const assessment = media.assessment;
  const hasActionableFixes = (media.fixPlan?.actions.length ?? 0) > 0;

  return (
    <div className="flex flex-col w-full max-w-lg p-8 bg-white border border-gray-200 rounded-2xl shadow-sm space-y-6">
      {/* EPISODE Header */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs font-bold tracking-widest text-gray-400 uppercase">
            Episode
          </span>
          {assessment && (
            <span className="text-xs font-medium text-gray-400">
              {assessment.profileName}
            </span>
          )}
        </div>
        <h2 className="text-xl font-bold tracking-tight text-gray-900 truncate">
          {media.filename}
        </h2>
        <p className="text-3xl font-light text-gray-600 mt-1">
          {formatTime(media.inspection.durationSeconds)}
        </p>
      </div>

      {/* OVERALL READINESS STATUS */}
      {assessment && (
        <div>
          {getOverallStatusBanner(assessment.overallStatus, assessment.summary)}
        </div>
      )}

      {/* AUDIO CHECKS SECTION */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase">
            Audio
          </h3>
          {isAnalysing && (
            <span className="text-xs text-indigo-600 font-medium animate-pulse">
              Analysing audio…
            </span>
          )}
        </div>

        {isAnalysing && !assessment?.audioChecks.length ? (
          <div className="py-8 text-center text-sm text-gray-500 italic bg-gray-50 rounded-xl border border-gray-100 animate-pulse">
            Measuring loudness, true peak, boundary silence and clipping…
          </div>
        ) : assessment?.audioChecks.length ? (
          <div className="space-y-4">
            {assessment.audioChecks.map((check) => (
              <div
                key={check.id}
                className="p-3.5 bg-gray-50 rounded-xl border border-gray-100 flex flex-col space-y-1.5"
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold text-gray-800">
                    {check.label}
                  </span>
                  <div className="flex items-center space-x-2">
                    <span className="font-mono text-sm font-medium text-gray-900">
                      {check.displayValue}
                    </span>
                    {getStatusBadge(check.status)}
                  </div>
                </div>

                <p className="text-xs text-gray-600 leading-relaxed">
                  {check.message}
                </p>

                {/* Sparkline from sexy-sparklines */}
                {check.sparkline && (
                  <div className="pt-1.5">
                    <BulletSparkline
                      value={check.sparkline.value}
                      min={check.sparkline.min}
                      max={check.sparkline.max}
                      target={check.sparkline.target}
                      ranges={check.sparkline.ranges}
                      height={14}
                      theme="minimal"
                      aria-label={`${check.label} chart`}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : null}
      </div>

      {/* FILE DETAILS SECTION */}
      <div>
        <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase mb-3">
          File
        </h3>

        {assessment?.fileChecks.length ? (
          <div className="grid grid-cols-2 gap-2.5">
            {assessment.fileChecks.map((check) => (
              <div
                key={check.id}
                className="p-3 bg-gray-50 rounded-xl border border-gray-100 flex flex-col justify-between"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs text-gray-500">{check.label}</span>
                  {check.status !== "GOOD" && getStatusBadge(check.status)}
                </div>
                <div className="font-mono text-sm font-semibold text-gray-900">
                  {check.displayValue}
                </div>
                <div className="text-[11px] text-gray-500 mt-1 line-clamp-1">
                  {check.message}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-gray-500">
            {media.format} · {(media.inspection.sampleRate / 1000).toFixed(1)} kHz · {media.inspection.channels === 1 ? "Mono" : "Stereo"}
          </div>
        )}
      </div>

      {/* PROCESSING STATE INDICATOR */}
      {isProcessing && (
        <div className="p-4 bg-indigo-50/70 border border-indigo-100 rounded-xl space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-bold tracking-widest text-indigo-500 uppercase">
              PODREADY
            </span>
            <span className="w-2 h-2 rounded-full bg-indigo-600 animate-ping" />
          </div>
          <div>
            <h4 className="text-sm font-bold text-gray-900">Preparing your episode…</h4>
            <p className="text-xs text-gray-600 mt-0.5">Applying deterministic audio adjustments.</p>
          </div>
          <div className="space-y-1.5 pt-2 border-t border-indigo-100">
            <span className="text-xs font-semibold text-gray-700 block">Applying:</span>
            {media.fixPlan?.actions.map((act) => (
              <div key={act.id} className="flex items-center space-x-2 text-xs text-indigo-900">
                <span className="text-indigo-600 font-bold">✓</span>
                <span>{act.title}</span>
              </div>
            ))}
          </div>
          <p className="text-xs text-indigo-700 font-medium italic animate-pulse">
            Checking final output…
          </p>
        </div>
      )}

      {/* POST-PROCESSING VERIFICATION VIEW */}
      {processingResponse && !isProcessing && (
        <div className="p-5 bg-white border border-gray-200 rounded-2xl shadow-sm space-y-5">
          {/* 1. PROCESSING STATUS HEADER */}
          <div className="flex items-center justify-between pb-3 border-b border-gray-100">
            <div className="flex items-center space-x-2">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500" />
              <h4 className="text-xs font-bold text-gray-900 tracking-wider uppercase">
                Processing Status
              </h4>
            </div>
            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold bg-emerald-100 text-emerald-800">
              Complete ✓
            </span>
          </div>

          {/* 2. CHANGES APPLIED */}
          <div className="space-y-2.5">
            <h5 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
              Changes applied
            </h5>
            {processingResponse.result.actionsApplied.length > 0 ? (
              <div className="space-y-2">
                {processingResponse.result.actionsApplied.map((applied, idx) => (
                  <div
                    key={idx}
                    className="p-3 bg-indigo-50/40 border border-indigo-100/60 rounded-xl space-y-1.5"
                  >
                    <div className="flex items-center space-x-1.5 text-xs font-bold text-gray-900">
                      <span className="text-indigo-600 font-bold">✓</span>
                      <span>{applied.title}</span>
                    </div>
                    {applied.fromValue && applied.toValue ? (
                      <div className="flex items-center space-x-2 text-xs font-mono text-gray-700 bg-white/90 px-2.5 py-1 rounded-lg border border-indigo-100/70">
                        <span className="text-gray-500">{applied.fromValue}</span>
                        <span className="text-indigo-400 font-bold">→</span>
                        <span className="font-semibold text-gray-900">{applied.toValue}</span>
                      </div>
                    ) : (
                      <p className="text-xs text-gray-600">{applied.description}</p>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-xs text-gray-500 italic p-2 bg-gray-50 rounded-lg">
                No audio changes were required.
              </div>
            )}
          </div>

          {/* 3. FINAL VERIFICATION STATUS */}
          <div className="space-y-2.5 pt-1">
            <h5 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
              Final verification
            </h5>
            {getOverallStatusBanner(
              processingResponse.afterAssessment.overallStatus,
              processingResponse.afterAssessment.summary
            )}
          </div>

          {/* 4. BEFORE VS AFTER MEASUREMENTS */}
          <div className="grid grid-cols-2 gap-3 text-xs">
            <div className="p-3 bg-gray-50 rounded-xl border border-gray-100">
              <span className="text-[10px] font-bold text-gray-400 uppercase tracking-wider block mb-1">
                Before
              </span>
              <div className="font-mono text-gray-800 space-y-1">
                <div>Loudness: {formatLoudness(processingResponse.beforeMeasurements?.integratedLoudnessLufs)}</div>
                <div>Peak: {formatPeak(processingResponse.beforeMeasurements?.truePeakDbtp)}</div>
              </div>
            </div>
            <div className="p-3 bg-emerald-50/50 rounded-xl border border-emerald-100">
              <span className="text-[10px] font-bold text-emerald-700 uppercase tracking-wider block mb-1">
                After (Candidate)
              </span>
              <div className="font-mono font-bold text-emerald-950 space-y-1">
                <div>Loudness: {formatLoudness(processingResponse.afterMeasurements.integratedLoudnessLufs)}</div>
                <div>Peak: {formatPeak(processingResponse.afterMeasurements.truePeakDbtp)}</div>
              </div>
            </div>
          </div>

          {/* 5. REMAINING REVIEW RECOMMENDATIONS (e.g. Clipping) */}
          {processingResponse.result.reviewAdvisories.length > 0 && (
            <div className="space-y-2 pt-1 border-t border-gray-100">
              <h5 className="text-[11px] font-bold text-amber-800 uppercase tracking-widest">
                Review recommended
              </h5>
              {processingResponse.result.reviewAdvisories.map((advisory, idx) => (
                <div
                  key={idx}
                  className="p-3 bg-amber-50/70 border border-amber-200/70 rounded-xl text-xs text-amber-900 space-y-1"
                >
                  <div className="flex items-center space-x-1.5 font-bold text-[11px] text-amber-800">
                    <span>⚠</span>
                    <span>Manual review suggested</span>
                  </div>
                  <p className="leading-relaxed text-amber-950">{advisory}</p>
                </div>
              ))}
            </div>
          )}

          {/* 6. CANDIDATE OUTPUT FILENAME */}
          <div className="pt-2 border-t border-gray-100 flex items-center justify-between text-xs text-gray-500">
            <span className="font-medium text-gray-600">Candidate output:</span>
            <span
              className="font-mono font-semibold text-gray-800 truncate max-w-[260px]"
              title={processingResponse.candidateFilename}
            >
              {processingResponse.candidateFilename}
            </span>
          </div>
        </div>
      )}


      {/* PODREADY FIXPLAN SECTION */}
      {media.fixPlan && !processingResponse && !isProcessing && (
        <div className="pt-2 border-t border-gray-100">
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-xs font-bold tracking-widest text-gray-400 uppercase">
              PodReady Plan
            </h3>
            <span className="text-xs font-semibold text-gray-600">
              {media.fixPlan.summary}
            </span>
          </div>

          {media.fixPlan.actions.length > 0 ? (
            <div className="space-y-3">
              {media.fixPlan.actions.map((action) => (
                <div
                  key={action.id}
                  className="p-3.5 bg-indigo-50/50 border border-indigo-100/80 rounded-xl space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-bold text-gray-900">
                      {action.title}
                    </span>
                    <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold bg-indigo-100 text-indigo-800">
                      {action.confidence === "HIGH" ? "Safe · High Confidence" : action.confidence}
                    </span>
                  </div>

                  {action.fromValue && action.toValue && (
                    <div className="flex items-center space-x-2 text-xs font-mono text-gray-700 bg-white/80 px-2.5 py-1.5 rounded-lg border border-indigo-100">
                      <span className="text-gray-500">{action.fromValue}</span>
                      <span className="text-indigo-400 font-bold">→</span>
                      <span className="font-semibold text-gray-900">{action.toValue}</span>
                    </div>
                  )}

                  <p className="text-xs text-gray-600 leading-relaxed">
                    <strong className="text-gray-700">Why:</strong> {action.reason}
                  </p>

                  <div className="text-[11px] text-gray-500 flex items-center justify-between pt-1 border-t border-indigo-100/60">
                    <span>Modifies audio:</span>
                    <span className="font-semibold text-gray-700">
                      {action.changesAudio ? "Yes" : "No"}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          ) : media.fixPlan.reviewAdvisories.length === 0 ? (
            <div className="p-3.5 bg-emerald-50/60 border border-emerald-100 rounded-xl text-xs text-emerald-800 font-medium">
              Your episode already meets the PodReady profile. No processing changes required.
            </div>
          ) : null}

          {/* Review Advisories (e.g. Clipping) */}
          {media.fixPlan.reviewAdvisories.length > 0 && (
            <div className="mt-3 space-y-2">
              {media.fixPlan.reviewAdvisories.map((advisory, idx) => (
                <div
                  key={idx}
                  className="p-3 bg-amber-50/70 border border-amber-200/70 rounded-xl text-xs text-amber-900 space-y-1"
                >
                  <span className="font-bold uppercase tracking-wider text-[10px] text-amber-800 block">
                    Review Recommended
                  </span>
                  <p className="leading-relaxed">{advisory}</p>
                </div>
              ))}
            </div>
          )}

          {/* Make PodReady Execution Action Button */}
          <div className="pt-4">
            <button
              onClick={onProcessAudio}
              disabled={!hasActionableFixes || isAnalysing}
              className={`w-full py-2.5 px-4 text-xs font-bold tracking-wider rounded-xl uppercase transition-colors ${
                hasActionableFixes
                  ? "bg-indigo-600 hover:bg-indigo-700 text-white shadow-sm cursor-pointer"
                  : "bg-gray-100 text-gray-400 cursor-not-allowed border border-gray-200"
              }`}
            >
              {hasActionableFixes ? "Make PodReady" : "Episode Ready"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}


