import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import http from 'node:http';
import crypto from 'node:crypto';
import {
  computeFileSha256,
  verifyFileChecksum,
  downloadAndVerifyAsset,
  provisionAssets,
  DEFAULT_MANIFEST_PATH,
} from './provision-assets.js';

describe('PodReady Runtime Asset Provisioning', () => {
  describe('Authoritative Manifest Integrity', () => {
    it('manifest file exists and has valid structure', async () => {
      assert.ok(fs.existsSync(DEFAULT_MANIFEST_PATH), 'manifest.json must exist');

      const content = JSON.parse(await fs.promises.readFile(DEFAULT_MANIFEST_PATH, 'utf-8'));
      assert.ok(Array.isArray(content.models), 'models must be an array');
      assert.ok(content.models.length > 0, 'must define at least one model');

      const smallModel = content.models.find((m) => m.filename === 'ggml-small.bin');
      assert.ok(smallModel, 'ggml-small.bin must be defined in manifest');
      assert.strictEqual(
        smallModel.sha256,
        '1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b',
        'Pinned SHA-256 must match authoritative production Whisper small model'
      );
      assert.strictEqual(
        smallModel.url,
        'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin',
        'Authoritative URL must point to HuggingFace whisper.cpp repo'
      );
      assert.strictEqual(smallModel.sizeBytes, 487601967, 'Exact size must match 487601967 bytes');
      assert.strictEqual(smallModel.required, true, 'Production model must be marked required');
    });
  });

  describe('Download, Streaming, Verification & Promotion', () => {
    let tempDir;
    let server;
    let serverBaseUrl;
    let requestCount = 0;
    const testFixtureData = Buffer.from('PodReady tiny test speech model fixture content 1234567890\n');
    const testFixtureSha256 = crypto.createHash('sha256').update(testFixtureData).digest('hex');

    before(async () => {
      tempDir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'podready-test-'));

      server = http.createServer((req, res) => {
        requestCount++;
        const url = new URL(req.url, 'http://localhost');

        if (url.pathname === '/good-model.bin') {
          res.writeHead(200, {
            'Content-Type': 'application/octet-stream',
            'Content-Length': testFixtureData.length.toString(),
          });
          res.end(testFixtureData);
        } else if (url.pathname === '/corrupted-model.bin') {
          res.writeHead(200, {
            'Content-Type': 'application/octet-stream',
            'Content-Length': '14',
          });
          res.end('BAD_HASH_DATA!');
        } else if (url.pathname === '/network-abort.bin') {
          res.writeHead(200, {
            'Content-Type': 'application/octet-stream',
            'Content-Length': '1000',
          });
          res.write('some partial data');
          setTimeout(() => req.destroy(), 20);
        } else {
          res.writeHead(404);
          res.end('Not Found');
        }
      });

      await new Promise((resolve) => {
        server.listen(0, '127.0.0.1', () => {
          const address = server.address();
          serverBaseUrl = `http://127.0.0.1:${address.port}`;
          resolve();
        });
      });
    });

    after(async () => {
      if (server) {
        await new Promise((resolve) => server.close(resolve));
      }
      if (tempDir && fs.existsSync(tempDir)) {
        await fs.promises.rm(tempDir, { recursive: true, force: true });
      }
    });

    it('successfully downloads to .part, verifies SHA-256 and promotes to target path', async () => {
      const model = {
        filename: 'test-model.bin',
        url: `${serverBaseUrl}/good-model.bin`,
        sha256: testFixtureSha256,
        sizeBytes: testFixtureData.length,
      };

      const result = await downloadAndVerifyAsset(model, {
        targetDir: tempDir,
        silent: true,
      });

      assert.strictEqual(result.status, 'downloaded_and_verified');
      assert.strictEqual(result.sha256, testFixtureSha256);

      const targetPath = path.join(tempDir, 'test-model.bin');
      const partPath = path.join(tempDir, 'test-model.bin.part');

      assert.ok(fs.existsSync(targetPath), 'Target file must exist after promotion');
      assert.ok(!fs.existsSync(partPath), '.part file must be cleaned up/promoted');

      const savedData = await fs.promises.readFile(targetPath);
      assert.deepStrictEqual(savedData, testFixtureData, 'Saved content must match fixture');
    });

    it('is idempotent and skips download if valid model already exists', async () => {
      const initialRequestCount = requestCount;

      const model = {
        filename: 'test-model.bin',
        url: `${serverBaseUrl}/good-model.bin`,
        sha256: testFixtureSha256,
        sizeBytes: testFixtureData.length,
      };

      const result = await downloadAndVerifyAsset(model, {
        targetDir: tempDir,
        silent: true,
      });

      assert.strictEqual(result.status, 'already_verified');
      assert.strictEqual(
        requestCount,
        initialRequestCount,
        'Zero network requests should be made when valid file already exists'
      );
    });

    it('cleans up .part file and fails cleanly when checksum mismatches', async () => {
      const model = {
        filename: 'bad-hash-model.bin',
        url: `${serverBaseUrl}/corrupted-model.bin`,
        sha256: testFixtureSha256, // Expects fixture hash, but server sends BAD_HASH_DATA!
        sizeBytes: testFixtureData.length,
      };

      const targetPath = path.join(tempDir, 'bad-hash-model.bin');
      const partPath = path.join(tempDir, 'bad-hash-model.bin.part');

      await assert.rejects(
        async () => {
          await downloadAndVerifyAsset(model, {
            targetDir: tempDir,
            silent: true,
          });
        },
        /SHA-256 checksum mismatch/i,
        'Must reject with checksum mismatch error'
      );

      assert.ok(!fs.existsSync(targetPath), 'Target file must NEVER be created with corrupt data');
      assert.ok(!fs.existsSync(partPath), '.part file must be deleted immediately on checksum failure');
    });

    it('re-provisions if existing destination file is corrupt', async () => {
      const targetPath = path.join(tempDir, 'corrupt-existing.bin');
      await fs.promises.writeFile(targetPath, 'CORRUPT_LOCAL_CONTENT');

      const model = {
        filename: 'corrupt-existing.bin',
        url: `${serverBaseUrl}/good-model.bin`,
        sha256: testFixtureSha256,
        sizeBytes: testFixtureData.length,
      };

      const result = await downloadAndVerifyAsset(model, {
        targetDir: tempDir,
        silent: true,
      });

      assert.strictEqual(result.status, 'downloaded_and_verified');
      const valid = await verifyFileChecksum(targetPath, testFixtureSha256);
      assert.ok(valid, 'Target file must now be valid after re-provisioning');
    });

    it('cleans up .part file on network abort/failure', async () => {
      const model = {
        filename: 'aborted-model.bin',
        url: `${serverBaseUrl}/network-abort.bin`,
        sha256: testFixtureSha256,
        sizeBytes: 1000,
      };

      const targetPath = path.join(tempDir, 'aborted-model.bin');
      const partPath = path.join(tempDir, 'aborted-model.bin.part');

      await assert.rejects(async () => {
        await downloadAndVerifyAsset(model, {
          targetDir: tempDir,
          silent: true,
        });
      });

      assert.ok(!fs.existsSync(targetPath), 'Target file must not exist');
      assert.ok(!fs.existsSync(partPath), '.part file must be cleaned up on failure');
    });

    it('provisionAssets processes a full manifest file end-to-end', async () => {
      const manifestPath = path.join(tempDir, 'test-manifest.json');
      const manifestContent = {
        version: '1.0.0',
        models: [
          {
            name: 'fixture1',
            filename: 'fixture1.bin',
            url: `${serverBaseUrl}/good-model.bin`,
            sha256: testFixtureSha256,
            sizeBytes: testFixtureData.length,
            required: true,
          },
        ],
      };
      await fs.promises.writeFile(manifestPath, JSON.stringify(manifestContent, null, 2));

      const results = await provisionAssets({
        manifestPath,
        targetDir: tempDir,
        silent: true,
      });

      assert.strictEqual(results.length, 1);
      assert.strictEqual(results[0].status, 'downloaded_and_verified');
      assert.strictEqual(results[0].model, 'fixture1');

      // Second run is idempotent
      const secondRunResults = await provisionAssets({
        manifestPath,
        targetDir: tempDir,
        silent: true,
      });
      assert.strictEqual(secondRunResults.length, 1);
      assert.strictEqual(secondRunResults[0].status, 'already_verified');
    });
  });
});
