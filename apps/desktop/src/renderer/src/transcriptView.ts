/** How a conversation's steps are drawn. Remembered for this window, not for one conversation. */
export type TranscriptView = 'normal' | 'verbose'

const KEY = 'locust.transcript-view'

export function readTranscriptView(storage: { getItem(key: string): string | null } | undefined): TranscriptView {
  try {
    return storage?.getItem(KEY) === 'verbose' ? 'verbose' : 'normal'
  } catch {
    return 'normal'
  }
}

export function writeTranscriptView(storage: { setItem(key: string, value: string): void } | undefined, view: TranscriptView): void {
  try {
    storage?.setItem(KEY, view)
  } catch {
    // A window that refuses storage still switches for this moment.
  }
}
