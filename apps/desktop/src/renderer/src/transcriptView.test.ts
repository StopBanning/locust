import { describe, expect, it } from 'vitest'

import { readTranscriptView, writeTranscriptView, type TranscriptView } from './transcriptView.js'

function memory(): { getItem(key: string): string | null; setItem(key: string, value: string): void; store: Map<string, string> } {
  const store = new Map<string, string>()
  return {
    store,
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => { store.set(key, value) }
  }
}

describe('Normal and Verbose stay with the window', () => {
  it('reads Normal until Verbose was chosen, and ignores anything else', () => {
    const box = memory()
    expect(readTranscriptView(box)).toBe('normal')
    writeTranscriptView(box, 'verbose')
    expect(readTranscriptView(box)).toBe('verbose')
    box.setItem('locust.transcript-view', 'loud')
    expect(readTranscriptView(box)).toBe('normal')
  })

  it('stays usable when storage refuses', () => {
    const broken = {
      getItem: (): string | null => { throw new Error('no') },
      setItem: (): void => { throw new Error('no') }
    }
    expect(readTranscriptView(broken)).toBe('normal')
    expect(() => writeTranscriptView(broken, 'verbose' satisfies TranscriptView)).not.toThrow()
  })
})
