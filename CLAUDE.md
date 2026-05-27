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

- `s3_client.rs` — `AppState(RwLock<S3State>)`. `S3State` holds the AWS `SdkConfig` (set on connect), `bucket_regions: HashMap<bucket→region>` cache, and `clients: HashMap<region→Arc<Client>>` cache. Tauri's `manage()` wraps state in `Arc` internally — no outer `Arc` needed.
- `commands.rs` — all `#[tauri::command]` handlers. Key pattern: every S3 operation calls `bucket_region()` first (cached `GetBucketLocation`), then `get_cached_client()` for a region-specific client. This handles cross-region buckets transparently.
- `error.rs` — `AppError` enum with `Serialize` impl. Variants: `Aws`, `Credentials`, `SsoExpired`, `AccessDenied`, `Other`. Serializes to `{code, message}` JSON for frontend pattern-matching.
- `list_buckets` failure (`AccessDenied` on `s3:ListAllMyBuckets`) is swallowed and returns `[]` — user can add buckets manually via the UI.
- Error sentinel strings returned by `set_profile`: `SSO_EXPIRED::<profile>`, `MFA_REQUIRED::<profile>`, `CREDENTIALS_ERROR::<msg>`. Frontend matches on these prefixes.
- `set_profile_mfa` shells out to `aws sts get-session-token` and injects `AWS_ACCESS_KEY_ID/SECRET/SESSION_TOKEN` env vars, then delegates to `set_profile`.
- `save_object` / `open_object` both stream in 64 KB chunks and emit `download:progress` events: `{bytes_received, total_bytes, key}`.
- `get_object_text` fetches via `GetObject` (no CORS) with `Range: bytes=0-2097151` (2 MB cap), returns UTF-8 string.
- `head_object` returns `ObjectMeta`: `content_type`, `content_length`, `last_modified`, `etag`, `storage_class`, `cache_control`, `content_encoding`, `user_meta: Vec<(String,String)>`.
- `open_object` downloads to `$TMPDIR/aws-thathoo/<hash16>_<filename>`, cleans files older than 1 day, then calls macOS `open`.
- `extract_debug_msg` / `fmt_sdk_err` — internal helpers that extract human-readable messages from AWS SDK debug strings (SDK wraps connector errors in `DispatchFailure` with empty `ProvideErrorMetadata`).

### Frontend state (`src/App.jsx`)

Single top-level state machine: `stage ∈ {profile, browser}`. All bucket/prefix/preview state lives here and is passed down as props. Theme is applied via `document.documentElement.setAttribute('data-theme', ...)` and persisted in `localStorage`. Manual bucket additions are persisted under `thathoo:manual-buckets:<profile>` and merged with auto-discovered buckets on connect. Sidebar and preview panel widths are resizable via drag handles; sidebar width persisted under `thathoo:sidebar-width`. Prefix navigation maintains a history stack (`prefixHistory`, `historyIdx`) for back/forward. Auto-update check runs once on mount via `@tauri-apps/plugin-updater`; install triggers relaunch via `@tauri-apps/plugin-process`.

### Theming

Five themes defined as `[data-theme="..."]` CSS variable overrides in `app.css`. Variables: `--bg-0/1/2/3`, `--border`, `--border-hi`, `--text-0/1/2`, `--accent`, `--green`, `--red`, `--blue` (all with `-dim` and `-border` variants for accent/green/blue).

### File preview

`FilePreview.jsx` fetches presigned URLs directly from the browser for image/PDF types. Text and XML files use `get_object_text` (Rust command, bypasses CORS). XML is formatted client-side via `indentXml()`. Binary files show a "cannot preview" placeholder. All Tauri event listeners are cleaned up in `finally` blocks.

### Tauri capabilities

`src-tauri/capabilities/default.json` grants: `core:default`, `updater:allow-check`, `updater:allow-download-and-install`, `process:allow-restart`. Adding a new plugin that exposes JS commands requires adding its permission here.

### LocalStorage keys

| Key | Contents |
|-----|----------|
| `theme` | active theme id |
| `thathoo:last-profile` | last successfully connected AWS profile name |
| `thathoo:manual-buckets:<profile>` | JSON array of manually added bucket names |
| `thathoo:sidebar-width` | sidebar width in px (number) |

## Known constraints

- `s3:GetBucketLocation` is required per bucket for cross-region detection. Without it, the app falls back to `us-east-1` which will fail for buckets in other regions.
- Bundle identifier: `com.awsthathoo.desktop` (changed from original `.app` suffix which conflicts with macOS bundle extension).
- Vite 8 uses OXC (not esbuild) — `vite.config.js` imports `@vitejs/plugin-react-oxc` and sets `minify: 'oxc'`.
- Auto-update signing: `tauri.conf.json` has `"pubkey": "PLACEHOLDER_PUBLIC_KEY"` — generate a real keypair with `npm run tauri -- signer generate -w ~/.tauri/awsthathoo.key` before shipping. Store private key as `TAURI_SIGNING_PRIVATE_KEY` in CI secrets.
