import test, { describe } from "node:test";
import assert from "node:assert";

function formatBatchPublishingDuration(seconds) {
  if (typeof seconds !== "number" || isNaN(seconds) || seconds < 0) {
    return "0s";
  }
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  }
  const mins = Math.floor(seconds / 60);
  const remainingSecs = Math.round(seconds % 60);
  if (mins < 60) {
    return `${mins}m ${remainingSecs}s`;
  }
  const hours = Math.floor(mins / 60);
  const remainingMins = mins % 60;
  return `${hours}h ${remainingMins}m`;
}

describe("Stage 5E Batch Publishing Formatters", () => {
  test("formats seconds under a minute", () => {
    assert.strictEqual(formatBatchPublishingDuration(0), "0s");
    assert.strictEqual(formatBatchPublishingDuration(14.2), "14s");
    assert.strictEqual(formatBatchPublishingDuration(59.4), "59s");
  });

  test("formats minutes and seconds", () => {
    assert.strictEqual(formatBatchPublishingDuration(60), "1m 0s");
    assert.strictEqual(formatBatchPublishingDuration(135), "2m 15s");
    assert.strictEqual(formatBatchPublishingDuration(3599), "59m 59s");
  });

  test("formats hours and minutes for long batch jobs", () => {
    assert.strictEqual(formatBatchPublishingDuration(3600), "1h 0m");
    assert.strictEqual(formatBatchPublishingDuration(7320), "2h 2m");
  });

  test("handles invalid inputs gracefully", () => {
    assert.strictEqual(formatBatchPublishingDuration(null), "0s");
    assert.strictEqual(formatBatchPublishingDuration(undefined), "0s");
    assert.strictEqual(formatBatchPublishingDuration(-5), "0s");
    assert.strictEqual(formatBatchPublishingDuration(NaN), "0s");
  });
});
