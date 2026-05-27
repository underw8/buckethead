// Thin wrapper around Tauri's invoke() that mirrors the old
// window.aws / window.s3 API surface used by the React components.
// Swap this file to target a different backend without touching components.

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const aws = {
  listProfiles: () =>
    invoke('list_profiles'),

  setProfile: (profile) =>
    invoke('set_profile', { profile }),

  headObject: (bucket, key) =>
    invoke('head_object', { bucket, key }),
}

export const s3 = {
  listBuckets: () =>
    invoke('list_buckets'),

  listObjects: (bucket, prefix = '', continuationToken) =>
    invoke('list_objects', {
      bucket,
      prefix,
      continuationToken: continuationToken ?? null,
    }),

  presign: (bucket, key, expiresIn) =>
    invoke('presign_url', {
      bucket,
      key,
      expiresIn: expiresIn ?? null,
    }),

  getObjectText: (bucket, key) =>
    invoke('get_object_text', { bucket, key }),

  saveObject: (bucket, key) =>
    invoke('save_object', { bucket, key }),

  openObject: (bucket, key) =>
    invoke('open_object', { bucket, key }),

  onDownloadProgress: (callback) =>
    listen('download:progress', (event) => callback(event.payload)),
}
