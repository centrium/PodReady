# PodReady

PodReady is a professional podcast pre-flight and audio readiness desktop application built with Tauri, React, Vite, and Rust.

## Distribution & Usage Roles

- **End Users / Customers**: Download and run the standalone, self-contained PodReady application (`PodReady.app` / `.dmg`). End users **never** need to install FFmpeg, Whisper, speech model weights, Homebrew, Node.js, or Rust—all dependencies and models are pre-bundled inside the application.
- **Developers**: Follow the developer quick-start below to provision assets and run locally.

## Prerequisites (Developers Only)

- [Node.js](https://nodejs.org/) (v18+ / v20+ / v24+)
- [pnpm](https://pnpm.io/) (v9+ / v10+ / v11+)
- [Rust](https://www.rust-lang.org/) (v1.77.2+)

## Developer Quick Start (Fresh Clone Workflow)

```bash
# 1. Install workspace dependencies (no 465MB model download during install)
pnpm install

# 2. Provision required runtime Whisper speech models
pnpm setup

# 3. Start local development server
pnpm --filter desktop tauri dev
```

## Runtime Asset Provisioning

PodReady excludes large binary model files from Git. Speech models are authoritatively specified in `apps/desktop/src-tauri/resources/models/manifest.json`.

- **`pnpm setup`**: Downloads `ggml-small.bin` (465MB) from Hugging Face into `apps/desktop/src-tauri/resources/models/`, streams to `.part`, validates SHA-256 (`1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b`), and atomically promotes it. Running `pnpm setup` again is idempotent and completes instantaneously without network overhead.
- **`pnpm test`**: Runs unit tests for provisioning using tiny fixtures without downloading large production models.
- **Compile-time Guarantee**: Tauri's `build.rs` verifies that the required `ggml-small.bin` model is present and intact before compiling the application.
- **Production Bundling**: The final release `.app` packages `ggml-small.bin` directly inside `Resources/resources/models/`, so customers never have to download or configure models.

## Development Scripts

```bash
pnpm setup        # Provision production Whisper model
pnpm test         # Run provisioning unit tests
pnpm build        # Build all frontend workspaces
pnpm lint         # Lint all workspaces (oxlint + tsc)
pnpm typecheck    # Typecheck all TypeScript code
```
