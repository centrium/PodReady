import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import {
  formatShowCheckLoudness,
  formatShowCheckPeak,
  formatShowCheckDuration,
  formatShowCheckSilence,
  formatShowCheckBitrate,
  formatSampleRateDisplay,
  formatChannelDisplay,
} from '../packages/domain/dist/index.js';

describe('Show Check Product Refinement Formatters (Stage 5D.1)', () => {
  test('formatShowCheckLoudness formats negative LUFS with minus sign and 1 decimal place', () => {
    const metric = {
      id: 'loudness',
      label: 'Integrated Loudness',
      unit: 'LUFS',
      candidateValue: -23.0,
      typicalValue: -16.0,
      usualLow: -18.2,
      usualHigh: -15.1,
      status: 'DIFFERENT',
      direction: 'BELOW_USUAL',
      message: 'Noticeably quieter',
      sampleCount: 4,
    };
    const f = formatShowCheckLoudness(metric);
    assert.equal(f.candidate, '−23.0 LUFS');
    assert.equal(f.typical, '−16.0 LUFS');
    assert.equal(f.range, '−18.2 → −15.1 LUFS');
  });

  test('formatShowCheckPeak formats true peak including legitimate 0.0 dBTP', () => {
    const metric = {
      id: 'true_peak',
      label: 'True Peak',
      unit: 'dBTP',
      candidateValue: 0.0,
      typicalValue: -1.4,
      usualLow: -2.5,
      usualHigh: -0.8,
      status: 'SLIGHTLY_DIFFERENT',
      direction: 'ABOVE_USUAL',
      message: 'Peak slightly high',
      sampleCount: 4,
    };
    const f = formatShowCheckPeak(metric);
    assert.equal(f.candidate, '0.0 dBTP');
    assert.equal(f.typical, '−1.4 dBTP');
    assert.equal(f.range, '−2.5 → −0.8 dBTP');
  });

  test('formatShowCheckDuration formats seconds to m:ss cleanly', () => {
    const metric = {
      id: 'duration',
      label: 'Duration',
      unit: 'seconds',
      candidateValue: 79.0,
      typicalValue: 61.5,
      usualLow: 40.75,
      usualHigh: 80.5,
      status: 'TYPICAL',
      direction: 'WITHIN_USUAL',
      message: 'Within usual duration',
      sampleCount: 4,
    };
    const f = formatShowCheckDuration(metric);
    assert.equal(f.candidate, '1:19');
    assert.equal(f.typical, '1:02');
    assert.equal(f.range, '0:41 → 1:21');
  });

  test('formatShowCheckSilence formats silence in seconds with 1 decimal place', () => {
    const metric = {
      id: 'leading_silence',
      label: 'Opening Silence',
      unit: 'seconds',
      candidateValue: 0.3,
      typicalValue: 0.1,
      usualLow: 0.0,
      usualHigh: 0.2,
      status: 'TYPICAL',
      direction: 'WITHIN_USUAL',
      message: 'Opening silence matches',
      sampleCount: 4,
    };
    const f = formatShowCheckSilence(metric);
    assert.equal(f.candidate, '0.3s');
    assert.equal(f.typical, '0.1s');
    assert.equal(f.range, '0.0s → 0.2s');
  });

  test('formatShowCheckBitrate converts raw bps and handles kbps rounding', () => {
    const metricBps = {
      id: 'bitrate',
      label: 'Bitrate',
      unit: 'bps',
      candidateValue: 90384.0,
      typicalValue: 224000.0,
      usualLow: 118596.0,
      usualHigh: 592800.0,
      status: 'TYPICAL',
      direction: 'WITHIN_USUAL',
      message: 'Bitrate matches',
      sampleCount: 4,
    };
    const f = formatShowCheckBitrate(metricBps);
    assert.equal(f.candidate, '90 kbps');
    assert.equal(f.typical, '224 kbps');
    assert.equal(f.range, '119 → 593 kbps');
  });

  test('formatSampleRateDisplay converts Hz numbers and strings to human kHz', () => {
    assert.equal(formatSampleRateDisplay(44100), '44.1 kHz');
    assert.equal(formatSampleRateDisplay('44100 Hz'), '44.1 kHz');
    assert.equal(formatSampleRateDisplay(48000), '48 kHz');
    assert.equal(formatSampleRateDisplay('48000 Hz'), '48 kHz');
    assert.equal(formatSampleRateDisplay(96000), '96 kHz');
    assert.equal(formatSampleRateDisplay(22050), '22.05 kHz');
  });

  test('formatChannelDisplay formats channel counts and names cleanly', () => {
    assert.equal(formatChannelDisplay(1), 'Mono');
    assert.equal(formatChannelDisplay('1'), 'Mono');
    assert.equal(formatChannelDisplay('mono'), 'Mono');
    assert.equal(formatChannelDisplay(2), 'Stereo');
    assert.equal(formatChannelDisplay('2'), 'Stereo');
    assert.equal(formatChannelDisplay('stereo'), 'Stereo');
    assert.equal(formatChannelDisplay(6), '6 Channels');
  });
});
