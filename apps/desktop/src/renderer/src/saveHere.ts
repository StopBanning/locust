import type { DesktopApi } from '../../shared/ipc.js'

/**
 * KEPT IN THIS BROWSER'S DOWNLOADS (0.853): what the computer's save dialog does for the window, done by Locust in a
 * browser itself -- a report it built (reportHere.ts), or a copy of a file a teammate wrote, whose bytes the computer
 * sends (WORKSPACE_DOWNLOAD_CHANNEL, with Save a copy's containment).
 */
export function saveInDownloads(name: string, data: string | Uint8Array): void {
  const part: BlobPart = typeof data === 'string' ? data : new Uint8Array(data)
  const href = URL.createObjectURL(new Blob([part], { type: typeof data === 'string' ? 'text/plain;charset=utf-8' : 'application/octet-stream' }))
  const link = document.createElement('a')
  link.href = href
  link.download = name
  document.body.appendChild(link)
  link.click()
  link.remove()
  window.setTimeout(() => URL.revokeObjectURL(href), 10_000)
}

/** A copy of a file a teammate wrote, from the computer to this browser's downloads: what went wrong, or undefined. */
export async function downloadCopy(
  path: string,
  api: Pick<DesktopApi, 'downloadFile'> | undefined = typeof window === 'undefined' ? undefined : window.desktop,
  save: (name: string, data: Uint8Array) => void = saveInDownloads
): Promise<string | undefined> {
  if (api === undefined) return 'Locust is not ready yet. Nothing was saved.'
  const answer = await api.downloadFile(path).catch(() => undefined)
  if (answer === undefined) return 'Locust on your computer did not answer. Nothing was saved.'
  if (!answer.ok) return answer.message
  save(answer.name, answer.bytes)
  return undefined
}
