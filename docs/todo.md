# AWS Thathoo — Backlog

Read-only S3 browser. No write operations (upload, delete, copy, rename) in scope.

---

## Must-Have — Low Complexity

- [x] **Error surfacing on bad credentials** — `set_profile` silently returns empty bucket list on any error (expired token, wrong profile, no internet). Return the error and display it on the connect screen instead of swallowing it.

- [x] **SSO token expiry detection** — Expired SSO tokens hit the same `Err(_) => vec![]` branch as missing credentials, giving no feedback. Detect `CredentialsError` / expired token, surface a message, and offer a button to re-run `aws sso login`.

- [x] **Cap presigned URL lifetime in Rust** — No server-side maximum. A caller can request a URL valid for years. Clamp `secs` to 604800 (7 days) in `commands.rs` regardless of the caller-supplied value.

- [x] **Copy `s3://bucket/key` URI** — Essential for CLI/SDK handoff. Add a copy action (row context or preview meta row) that writes `s3://<bucket>/<key>` to clipboard. No backend command needed.

- [x] **Client-side prefix filter** — No way to search within the current folder. Add a filter input in the `ObjectBrowser` toolbar that client-side filters `items.objects` and `items.folders` by name substring.

- [x] **Bucket filter in sidebar** — No search with 30+ buckets in the sidebar. Add a small filter input above the bucket list that filters by name client-side.

- [x] **Persist sort preference** — `sortKey` and `sortDir` reset to `name / asc` on every bucket switch. Persist to `localStorage` keyed by bucket name.

- [x] **Presign error shown in UI** — `handleFileClick` catches presign failures with only `console.error`. Show a visible error banner in the `ObjectBrowser` toolbar instead.

- [x] **Resizable sidebar** — Fixed at `--sidebar-w: 220px`. Apply the same drag-handle pattern already built for the preview pane to the sidebar's right edge.

- [x] **Image dimensions in preview** — No width × height displayed. Read `img.naturalWidth` / `img.naturalHeight` in the `onLoad` handler and add to the preview metadata rows.

- [x] **`Range` request for text preview** — `get_object_text` calls `collect()` on the full body then checks the 2 MB cap — buffers everything before rejecting. Use a `Range: bytes=0-2097151` header on the `GetObject` request so only 2 MB is ever transferred.

- [x] **Temp file cleanup** — `open_object` writes to `$TMPDIR/aws-thathoo/<filename>` and never cleans up. Sensitive objects (credentials, PII) accumulate on disk. Switch to the `tempfile` crate (auto-deletes on drop) or record paths and delete on next app launch.

- [x] **Temp filename collision fix** — Two objects at `a/report.pdf` and `b/report.pdf` map to the same temp path; second download silently overwrites the first. Include a hash of `bucket+key` in the temp filename.

- [x] **CSP tighten** — `frame-src https:` and `img-src https:` allow loading from any HTTPS origin. Scope to `*.s3.amazonaws.com` and `*.s3-accelerate.amazonaws.com`.

- [x] **Universal binary (arm64 + Intel)** — `release.sh` hardcodes `aarch64` output path. Add a `--target universal-apple-darwin` build path to `release.sh` and update the output path glob.

---

## Must-Have — Medium Complexity

- [ ] **Auto-update (Tauri updater plugin)** — No `updater` section in `tauri.conf.json` and no update manifest. Add `tauri-plugin-updater`, host a `latest.json` manifest on GitHub Releases, and add an update check on app launch.

- [x] **GitHub Actions CI/CD** — No `.github/workflows/` directory. Create a release workflow triggered on `git push --tags` that: runs `release.sh patch --build`, signs and notarizes, uploads the DMG to GitHub Releases, and publishes the updater manifest.

- [ ] **Object versioning — list only** — `ListObjectsV2` hides all non-current versions. Add a `list_object_versions(bucket, prefix)` command using `ListObjectVersions` and a versions panel or toggle in `ObjectBrowser` that shows version ID, modified date, size, and delete markers. Read-only; no restore.

- [x] **Keyboard navigation in file table** — No `tabIndex`, arrow key, or Enter/Escape handler on rows. Add keyboard navigation: arrow keys move selection, Enter navigates into folders or opens preview, Escape closes preview.

- [x] **Typed error enum** — All Rust errors are `.map_err(|e| e.to_string())`, discarding the AWS error code, HTTP status, and request ID. Introduce a typed `AppError` enum with `thiserror` (already in `Cargo.toml`) carrying `code`, `message`, and `request_id`. The frontend can then distinguish `AccessDenied` from `NoSuchKey`.

- [x] **Cache `Client` per region** — `make_client` rebuilds a full `aws_sdk_s3::Client` (including HTTP connector pool) on every command invocation. Cache a `HashMap<String, Client>` keyed by region in `S3State`; create only on first use per region.

- [ ] **Download progress events** — `save_object` and `open_object` are opaque — UI shows a spinner with no progress. Stream the `GetObject` body via `tokio::io::copy` to the destination file and emit `download:progress` Tauri events (`{ bytes_received, total_bytes }`) at each chunk boundary. Pass `AppHandle` into the commands as a Tauri v2 parameter injection.

- [x] **Stream large files** — `get_object_bytes` loads the entire object into `Vec<u8>` before writing to disk. A multi-GB object will OOM the process. Stream `GetObject` body directly to the file handle using `tokio::io::copy` instead of `collect()`.

- [x] **Back/forward prefix navigation** — No browser-style history. Maintain a prefix stack in `App.jsx`; add Cmd+[ / Cmd+] keyboard shortcuts and optional toolbar back/forward buttons.

---

## Nice-to-Have

- [ ] **`HeadObject` metadata inspector** — `ObjectInfo` exposes only `key`, `size`, `modified`, `etag`. Add a `head_object(bucket, key)` command and an expandable metadata section in the preview panel showing `ContentType`, `CacheControl`, storage class, `x-amz-meta-*` headers, and object tags.

- [ ] **Storage class warning before opening** — Objects in `GLACIER`, `DEEP_ARCHIVE`, or `INTELLIGENT_TIERING` tiers fail silently on `GetObject`. Surface storage class in the object list and show a warning in the preview panel before the user tries to open or download.

- [ ] **Syntax highlighting in text preview** — All code and data files render plain monochrome in `<pre>`. Add Shiki or Prism (async import) for the extensions already classified as text: `js`, `ts`, `json`, `yaml`, `xml`, `html`, `css`, `md`.

- [ ] **Jump-to-prefix path bar** — No way to paste a full S3 path. Make the breadcrumb editable on click (or via `/` shortcut): clicking the breadcrumb area enters a text input that accepts `bucket/some/deep/prefix/` and navigates on Enter.

- [ ] **Multi-account / role switcher** — Profile list is a flat `<select>` with no grouping. Group profiles by AWS account (parsed from `sso_account_id` or `role_arn` in `~/.aws/config`) and add an inline account/role switcher in the bottom bar without requiring full disconnect.

- [ ] **MFA / TOTP prompt for role assumption** — When a profile has `mfa_serial` in `~/.aws/config`, the SDK silently fails on credential resolution. Intercept the error and show a TOTP input dialog.

- [ ] **S3 Select for large structured files** — Large CSV, JSON, and Parquet objects hit the 2 MB preview cap. Implement `SelectObjectContent` with a simple SQL input, allowing users to query and preview columnar data without downloading the full object.

- [x] **Empty-state differentiation** — "No bucket selected" and "empty prefix" produce nearly identical states. Show distinct messages and, for empty prefixes, add the total folder count confirmation ("0 objects, 0 folders here").
