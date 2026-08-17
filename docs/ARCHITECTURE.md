# PodReady Architecture

## Workspace Structure
PodReady uses a Turborepo + pnpm workspace structure:
- `apps/desktop`: The main Tauri + React + Vite application.
  - `src/`: React frontend UI.
  - `src-tauri/`: Rust backend, holding media inspection logic.
- `packages/domain`: Shared TypeScript interfaces between the frontend and backend.
- `packages/ui`: Shared Tailwind configuration and future shared UI components.

## Boundaries
### React ↔ Tauri ↔ Rust
The React UI communicates exclusively via Tauri IPC commands. It does not invoke media tools directly. The UI requests information from Rust, and Rust responds with domain models (e.g., `MediaSource`).

### Media Inspection Boundary
Rust invokes `ffprobe` internally inside the `apps/desktop/src-tauri/src/media/ffprobe.rs` module. It parses the raw JSON output from `ffprobe` into strongly typed `AudioMeasurements` and `MediaFormat` structs. 

### Error Strategy
`ffprobe` errors, parsing errors, or unsupported formats are mapped into the user-oriented `AppError` enum on the Rust side before being returned to the UI. The UI only displays clean, human-readable errors.

### File Handling
The original files are strictly read-only during inspection and are never modified by PodReady.
