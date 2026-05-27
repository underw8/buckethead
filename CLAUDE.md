# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Install JS deps (once, or after package.json changes)
npm install --legacy-peer-deps

# Dev mode (hot-reload frontend + Rust watch)
npm run tauri dev

# Build release .dmg
npm run tauri build
# Output: src-tauri/target/release/bundle/dmg/

# Rust-only build check (faster than full tauri dev)
cd src-tauri && cargo build

# Regenerate icons from a square PNG source
npm run tauri icon icon.png
# source must be square — use: sips -z <size> <size> icon.png --out icon.png
```

No test suite exists. No linter configured beyond Vite's build warnings.

## Architecture

Two-process model: Vite/React frontend ↔ Tauri IPC ↔ Rust backend.

### IPC boundary

`src/bridge.js` is the only file that calls `invoke()`. All frontend components import from bridge — never call `invoke()` directly. Adding a new Rust command requires: `#[tauri::command]` in `commands.rs` → register in `lib.rs` `invoke_handler![]` → add wrapper in `bridge.js`.

### Rust backend (`src-tauri/src/`)

- `s3_client.rs` — `AppState` = `Arc<RwLock<S3State>>`. `S3State` holds the AWS `SdkConfig` (set on connect) and a `HashMap<bucket→region>` cache. No S3 `Client` stored — clients are built per-request via `make_client(cfg, region)`.
- `commands.rs` — all `#[tauri::command]` handlers. Key pattern: every S3 operation calls `bucket_region()` first, which calls `GetBucketLocation` (cached) to resolve the bucket's actual region, then builds a region-specific client. This handles cross-region buckets transparently.
- `list_buckets` failure (AccessDenied on `s3:ListAllMyBuckets`) is swallowed and returns `[]` — user can add buckets manually via the UI.

### Frontend state (`src/App.jsx`)

Single top-level state machine: `stage ∈ {profile, browser}`. All bucket/prefix/preview state lives here and is passed down as props. Theme is applied via `document.documentElement.setAttribute('data-theme', ...)` and persisted in `localStorage`. Manual bucket additions are persisted under `thathoo:manual-buckets:<profile>` and merged with auto-discovered buckets on connect.

### Theming

Five themes defined as `[data-theme="..."]` CSS variable overrides in `app.css`. Variables: `--bg-0/1/2/3`, `--border`, `--border-hi`, `--text-0/1/2`, `--accent`, `--green`, `--red`, `--blue` (all with `-dim` and `-border` variants for accent/green/blue).

### File preview

`FilePreview.jsx` fetches presigned URLs directly from the browser for text types (including XML). XML is formatted client-side via `indentXml()`. Images and PDFs use `<img>` / `<iframe>`. Binary files show a "cannot preview" placeholder.

### LocalStorage keys

| Key | Contents |
|-----|----------|
| `theme` | active theme id |
| `thathoo:last-profile` | last successfully connected AWS profile name |
| `thathoo:manual-buckets:<profile>` | JSON array of manually added bucket names |

## Known constraints

- `s3:GetBucketLocation` is required per bucket for cross-region detection. Without it, the app falls back to `us-east-1` which will fail for buckets in other regions.
- Bundle identifier: `com.awsthathoo.desktop` (changed from original `.app` suffix which conflicts with macOS bundle extension).
- Vite 8 uses OXC (not esbuild) — `vite.config.js` imports `@vitejs/plugin-react-oxc` and sets `minify: 'oxc'`.
