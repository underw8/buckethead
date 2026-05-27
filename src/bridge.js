// Thin wrapper around tauri-specta generated bindings.
// Exposes the same aws / s3 API surface used by the React components.
// Swap this file to target a different backend without touching components.

import { commands } from './bindings'
import { listen } from '@tauri-apps/api/event'

// Unwrap the {status, data} / {status, error} tagged union from tauri-specta
// and re-throw on error — preserves existing Promise<T> | throw behavior.
function unwrap(resultPromise) {
  return resultPromise.then((r) => {
    if (r.status === 'error') throw r.error
    return r.data
  })
}

export const aws = {
  listProfiles: () =>
    unwrap(commands.listProfiles()),

  setProfile: (profile) =>
    unwrap(commands.setProfile(profile)),

  setProfileMfa: (profile, mfaToken) =>
    unwrap(commands.setProfileMfa(profile, mfaToken)),

  headObject: (bucket, key) =>
    unwrap(commands.headObject(bucket, key)),
}

export const s3 = {
  listBuckets: () =>
    unwrap(commands.listBuckets()),

  listObjects: (bucket, prefix, continuationToken) =>
    unwrap(commands.listObjects(bucket, prefix ?? '', continuationToken ?? null)),

  presign: (bucket, key, expiresIn) =>
    unwrap(commands.presignUrl(bucket, key, expiresIn ?? null)),

  getObjectText: (bucket, key) =>
    unwrap(commands.getObjectText(bucket, key)),

  saveObject: (bucket, key) =>
    unwrap(commands.saveObject(bucket, key)),

  openObject: (bucket, key) =>
    unwrap(commands.openObject(bucket, key)),

  onDownloadProgress: (callback) =>
    listen('download:progress', (event) => callback(event.payload)),
}
