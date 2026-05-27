# Backlog — Best Practices Fixes

Findings from Tauri v2 best-practices audit (2026-05-27).

---

## 🔴 Critical

### 1. Create `src-tauri/capabilities/default.json`
Tauri v2 ACL blocks all plugin JS commands without explicit capability grants.
Two frontend calls silently fail today:
- `src/App.jsx:54` — `@tauri-apps/plugin-updater` `check()`
- `src/App.jsx:75` — `@tauri-apps/plugin-process` `relaunch()`

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Main window permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "updater:allow-check",
    "updater:allow-download-and-install",
    "process:allow-relaunch"
  ]
}
```

### 2. Generate real updater signing keypair
`tauri.conf.json` has `"pubkey": "PLACEHOLDER_PUBLIC_KEY"` — auto-update is broken and insecure if shipped.

Steps:
1. `npm run tauri -- signer generate -w ~/.tauri/awsthathoo.key`
2. Copy public key output → replace `PLACEHOLDER_PUBLIC_KEY` in `tauri.conf.json`
3. Store private key in CI as `TAURI_SIGNING_PRIVATE_KEY` secret — never commit it
4. Store passphrase (if set) as `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

---

## 🟡 Medium

### 3. Remove `'unsafe-inline'` from CSP style-src
Tauri v2 auto-injects nonces/hashes for styles — `'unsafe-inline'` is explicitly discouraged.

In `tauri.conf.json`, change:
```
style-src 'self' 'unsafe-inline' https://fonts.googleapis.com
```
to:
```
style-src 'self' https://fonts.googleapis.com
```

### 4. Remove redundant outer `Arc` from `AppState`
`src-tauri/src/s3_client.rs`: `AppState(pub Arc<RwLock<S3State>>)`

Tauri's `manage()` wraps state in `Arc` internally. Outer `Arc` is a redundant allocation.

Change to:
```rust
pub struct AppState(pub RwLock<S3State>);
// lib.rs:
.manage(AppState(RwLock::new(S3State::default())))
```
Update all `state.0.read().await` / `state.0.write().await` callers in `commands.rs` — method calls stay identical, only the wrapper type changes.

### 5. Split `commands.rs` into domain modules
`src-tauri/src/commands.rs` is 733 lines. Split by domain:

```
src-tauri/src/
  commands/
    mod.rs      ← re-exports all pub commands
    auth.rs     ← list_profiles, set_profile, set_profile_mfa
    buckets.rs  ← list_buckets
    objects.rs  ← list_objects, presign_url, get_object_text,
                   save_object, open_object, head_object
```

`lib.rs` invoke_handler registration unchanged — just update `mod commands;` import.

### 6. Add `.scannerwork/` to `.gitignore`
SonarQube artifact present in repo root. Not tracked yet but not ignored — a future `git add .` would accidentally include it.

Add to `.gitignore`:
```
# SonarQube
.scannerwork/
```

---

## 🔵 Low / Optional Enhancements

### 7. Add `tauri-specta` for type-safe IPC bindings
Auto-generates TypeScript bindings from Rust command signatures. Eliminates `invoke<T>` guessing and runtime type errors. See https://github.com/specta-rs/tauri-specta.

### 8. Add `tracing` crate for structured backend logging
Replace `eprintln!` debug output with structured logs — makes field debugging possible. Add `tracing` + `tracing-subscriber` to `Cargo.toml`.

### 9. Delete empty `docs/` directory
Remove once this file is committed, or keep as home for future documentation.
