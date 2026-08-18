#!/usr/bin/env node

/**
 * PodReady Runtime Asset Provisioning
 *
 * Provisions production Whisper speech models and runtime assets required by PodReady.
 * Streams downloads to temporary .part files, verifies SHA-256 integrity, and promotes
 * them atomically to production resource paths.
 */

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { once } from 'node:events';
import { pipeline } from 'node:stream/promises';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '..');

export const DEFAULT_MANIFEST_PATH = path.join(
  REPO_ROOT,
  'apps/desktop/src-tauri/resources/models/manifest.json'
);

export const DEFAULT_MODELS_DIR = path.join(
  REPO_ROOT,
  'apps/desktop/src-tauri/resources/models'
);

/**
 * Computes the SHA-256 hex digest of a file on disk by streaming.
 * @param {string} filePath
 * @returns {Promise<string>}
 */
export async function computeFileSha256(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`File not found: ${filePath}`);
  }
  const hash = crypto.createHash('sha256');
  const readStream = fs.createReadStream(filePath);
  await pipeline(readStream, async function* (source) {
    for await (const chunk of source) {
      hash.update(chunk);
    }
  });
  return hash.digest('hex').toLowerCase();
}

/**
 * Checks if a file exists and matches the expected SHA-256 checksum.
 * @param {string} filePath
 * @param {string} expectedSha256
 * @returns {Promise<boolean>}
 */
export async function verifyFileChecksum(filePath, expectedSha256) {
  if (!fs.existsSync(filePath)) {
    return false;
  }
  try {
    const actualHash = await computeFileSha256(filePath);
    return actualHash === expectedSha256.toLowerCase();
  } catch {
    return false;
  }
}

/**
 * Format byte counts into human-readable strings (e.g. 465.0 MB).
 * @param {number} bytes
 * @returns {string}
 */
export function formatBytes(bytes) {
  if (bytes == null || isNaN(bytes) || bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

/**
 * Formats a progress bar string.
 * @param {number} percent - 0 to 100
 * @param {number} width - character width
 * @returns {string}
 */
function renderProgressBar(percent, width = 20) {
  const completeLength = Math.max(0, Math.min(width, Math.round((percent / 100) * width)));
  const emptyLength = width - completeLength;
  const bar = '='.repeat(completeLength) + (emptyLength > 0 ? '>' : '') + ' '.repeat(Math.max(0, emptyLength - 1));
  return `[${bar}]`;
}

/**
 * Downloads and verifies a single runtime asset model.
 *
 * @param {object} model - Model configuration from manifest
 * @param {string} model.filename - e.g. "ggml-small.bin"
 * @param {string} model.url - Authoritative download URL
 * @param {string} model.sha256 - Pinned SHA-256 hex digest
 * @param {number} [model.sizeBytes] - Expected size in bytes
 * @param {string} [model.description] - Description
 * @param {object} [options]
 * @param {string} [options.targetDir] - Destination directory
 * @param {boolean} [options.silent] - Suppress console output
 * @param {typeof fetch} [options.fetchFn] - Custom fetch function for tests
 * @param {(progress: { bytesDownloaded: number, totalBytes: number, percent: number, speedMBs: number }) => void} [options.onProgress]
 * @returns {Promise<{ status: 'already_verified' | 'downloaded_and_verified', path: string, sha256: string }>}
 */
export async function downloadAndVerifyAsset(model, options = {}) {
  const {
    targetDir = DEFAULT_MODELS_DIR,
    silent = false,
    fetchFn = globalThis.fetch,
    onProgress,
  } = options;

  const targetPath = path.join(targetDir, model.filename);
  const partPath = path.join(targetDir, `${model.filename}.part`);
  const expectedSha256 = model.sha256.toLowerCase();

  // Ensure target directory exists
  await fs.promises.mkdir(targetDir, { recursive: true });

  // Check if target file already exists and is valid
  if (fs.existsSync(targetPath)) {
    if (!silent) {
      console.log(`[PodReady Setup] Checking existing ${model.filename}...`);
    }
    const isValid = await verifyFileChecksum(targetPath, expectedSha256);
    if (isValid) {
      if (!silent) {
        console.log(
          `[PodReady Setup] ✓ ${model.filename} verified (SHA-256: ${expectedSha256.slice(0, 12)}...). Up to date.`
        );
      }
      return {
        status: 'already_verified',
        path: targetPath,
        sha256: expectedSha256,
      };
    } else {
      if (!silent) {
        console.log(
          `[PodReady Setup] ⚠️  Existing ${model.filename} is corrupt or invalid. Re-downloading from authoritative source...`
        );
      }
    }
  }

  // Clean up any stale partial download before starting
  if (fs.existsSync(partPath)) {
    try {
      await fs.promises.unlink(partPath);
    } catch {
      // Ignore unlink errors for stale partial files
    }
  }

  if (!silent) {
    console.log(`[PodReady Setup] Downloading ${model.filename} (${formatBytes(model.sizeBytes)})...`);
    console.log(`[PodReady Setup] Source: ${model.url}`);
  }

  let writeStream = null;
  const startTime = Date.now();
  let bytesDownloaded = 0;
  let lastProgressLogTime = 0;

  try {
    const response = await fetchFn(model.url, {
      redirect: 'follow',
      headers: {
        'User-Agent': 'PodReady-Asset-Provisioner/1.0',
      },
    });

    if (!response.ok) {
      throw new Error(
        `Failed to download ${model.filename}: HTTP ${response.status} ${response.statusText}`
      );
    }

    if (!response.body) {
      throw new Error(`Response body for ${model.filename} is null or empty`);
    }

    const totalBytes = Number(response.headers?.get?.('content-length')) || model.sizeBytes || 0;
    writeStream = fs.createWriteStream(partPath);
    const hasher = crypto.createHash('sha256');

    const isTTY = Boolean(process.stdout && process.stdout.isTTY && !silent);

    for await (const chunk of response.body) {
      bytesDownloaded += chunk.length;
      hasher.update(chunk);

      if (!writeStream.write(chunk)) {
        await once(writeStream, 'drain');
      }

      const now = Date.now();
      const elapsedSec = Math.max(0.001, (now - startTime) / 1000);
      const speedMBs = bytesDownloaded / elapsedSec / (1024 * 1024);
      const percent = totalBytes > 0 ? Math.min(100, (bytesDownloaded / totalBytes) * 100) : 0;

      if (onProgress) {
        onProgress({ bytesDownloaded, totalBytes, percent, speedMBs });
      }

      if (!silent) {
        if (isTTY) {
          const bar = renderProgressBar(percent, 20);
          const line = `[PodReady Setup] ${bar} ${percent.toFixed(1)}% (${formatBytes(bytesDownloaded)} / ${formatBytes(totalBytes)}) @ ${speedMBs.toFixed(1)} MB/s`;
          process.stdout.write(`\r${line}`);
        } else if (now - lastProgressLogTime > 3000 || bytesDownloaded === totalBytes) {
          lastProgressLogTime = now;
          console.log(
            `[PodReady Setup] Progress: ${percent.toFixed(1)}% (${formatBytes(bytesDownloaded)} / ${formatBytes(totalBytes)}) @ ${speedMBs.toFixed(1)} MB/s`
          );
        }
      }
    }

    writeStream.end();
    await once(writeStream, 'finish');

    if (isTTY) {
      process.stdout.write('\n');
    }

    // Verify SHA-256 before promotion
    const actualSha256 = hasher.digest('hex').toLowerCase();
    if (!silent) {
      console.log(`[PodReady Setup] Verifying SHA-256 checksum...`);
    }

    if (actualSha256 !== expectedSha256) {
      // Immediately remove corrupted .part file
      try {
        await fs.promises.unlink(partPath);
      } catch {}

      throw new Error(
        `SHA-256 checksum mismatch for ${model.filename}!\n` +
          `  Expected: ${expectedSha256}\n` +
          `  Actual:   ${actualSha256}\n` +
          `The partial download has been deleted. Please check your network connection and retry.`
      );
    }

    // Promote verified .part file to final destination
    await fs.promises.rename(partPath, targetPath);

    if (!silent) {
      console.log(
        `[PodReady Setup] ✓ ${model.filename} verified successfully (SHA-256: ${actualSha256.slice(0, 12)}...) and promoted to production path.`
      );
    }

    return {
      status: 'downloaded_and_verified',
      path: targetPath,
      sha256: actualSha256,
    };
  } catch (err) {
    if (writeStream) {
      try {
        writeStream.destroy();
      } catch {}
    }
    if (fs.existsSync(partPath)) {
      try {
        await fs.promises.unlink(partPath);
      } catch {}
    }
    throw err;
  }
}

/**
 * Loads the asset manifest and provisions all required models.
 *
 * @param {object} [options]
 * @param {string} [options.manifestPath] - Path to manifest.json
 * @param {string} [options.targetDir] - Destination directory
 * @param {boolean} [options.silent] - Suppress console output
 * @param {typeof fetch} [options.fetchFn] - Custom fetch function for tests
 * @returns {Promise<Array<{ model: string, status: string, path: string }>>}
 */
export async function provisionAssets(options = {}) {
  const manifestPath = options.manifestPath || DEFAULT_MANIFEST_PATH;
  const targetDir = options.targetDir || path.dirname(manifestPath);

  if (!fs.existsSync(manifestPath)) {
    throw new Error(`Asset manifest not found at: ${manifestPath}`);
  }

  const manifestRaw = await fs.promises.readFile(manifestPath, 'utf-8');
  const manifest = JSON.parse(manifestRaw);

  if (!manifest.models || !Array.isArray(manifest.models) || manifest.models.length === 0) {
    throw new Error(`Asset manifest has no valid models array: ${manifestPath}`);
  }

  const results = [];
  for (const model of manifest.models) {
    if (model.required !== false) {
      const result = await downloadAndVerifyAsset(model, {
        ...options,
        targetDir,
      });
      results.push({
        model: model.name || model.filename,
        ...result,
      });
    }
  }

  return results;
}

// CLI execution
const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  console.log('======================================================================');
  console.log('PodReady Runtime Asset Provisioning');
  console.log('======================================================================');

  try {
    const results = await provisionAssets();
    console.log('======================================================================');
    console.log(`✓ Provisioning complete. ${results.length} model(s) ready.`);
    console.log('======================================================================');
  } catch (err) {
    console.error('\n======================================================================');
    console.error('PROVISIONING ERROR: Failed to provision runtime assets:');
    console.error(err.message || err);
    console.error('======================================================================\n');
    process.exitCode = 1;
  }
}

