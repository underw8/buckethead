<p align="center">
  <img src="logo.png" alt="Buckethead" width="160" />
</p>

<h1 align="center">Buckethead</h1>

<p align="center">All HEAD, no PUT — S3 browser for macOS, Windows, and Linux.<br>Built with Tauri v2 + React. ~10 MB bundle. Rust backend via <code>aws-sdk-s3</code>.</p>

## Features

- Browse S3 buckets and objects across all regions (auto-detects bucket region)
- Preview images, PDFs, text, JSON, XML (formatted), and more
- Presigned URL generation for file sharing
- Works with restricted IAM policies (only `s3:ListBucket` + `s3:GetObject` required)
- Manually add buckets by name (persisted per profile)
- Theme switcher: Default, Night Owl, Solarized Dark, Solarized Light, Dracula
- Remembers last connected profile

## Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# JS deps
npm install
```

## Dev

```bash
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/`

## Release

Push a version tag — CI builds all platforms and creates a draft GitHub Release:

```bash
git tag v1.0.0 && git push origin v1.0.0
```

## Minimum IAM permissions

```json
{
  "Effect": "Allow",
  "Action": ["s3:ListAllMyBuckets"],
  "Resource": "*"
},
{
  "Effect": "Allow",
  "Action": ["s3:ListBucket", "s3:GetBucketLocation"],
  "Resource": "arn:aws:s3:::your-bucket"
},
{
  "Effect": "Allow",
  "Action": ["s3:GetObject"],
  "Resource": "arn:aws:s3:::your-bucket/*"
}
```

Without `s3:ListAllMyBuckets`, buckets can be added manually via the `+` button in the sidebar.  
Without `s3:GetBucketLocation`, specify the correct region at profile connect time.

## Project structure

```
src/                        ← React frontend (Vite + OXC)
  bridge.js                 ← invoke() wrappers
  App.jsx                   ← root — theme, bucket state, layout
  app.css                   ← all styles + 5 theme definitions
  components/
    ProfileSelector.jsx     ← profile picker, last-profile persistence
    BucketList.jsx          ← sidebar bucket list
    ObjectBrowser.jsx       ← object listing, breadcrumbs, sort, pagination
    FilePreview.jsx         ← image / PDF / text / XML preview + download

src-tauri/                  ← Rust backend
  src/
    main.rs                 ← binary entry
    lib.rs                  ← Tauri builder + command registration
    commands.rs             ← list_profiles, set_profile, list_buckets,
                              list_objects, presign_url
    s3_client.rs            ← AppState (SdkConfig + bucket-region cache)
  Cargo.toml
  tauri.conf.json
```

## Credential support

Uses `ProfileFileCredentialsProvider` from `aws-config`, which supports:
- Named profiles in `~/.aws/credentials`
- Role chaining via `~/.aws/config`
- SSO profiles via `aws sso login`
- AWS Web Console session reuse via `aws login`
