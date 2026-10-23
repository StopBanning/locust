// Types for _tools/window-zip.mjs, which the relay's tests read the zips of (apps/relay/test).
export function crc32(bytes: Uint8Array): number
export function writeZip(files: readonly { readonly path: string; readonly bytes: Uint8Array }[]): Buffer
export function filesUnder(folder: string): { path: string; bytes: Buffer }[]
