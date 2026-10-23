import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { MAX_REPORT_URL, REPORT_DESTINATION, feedbackUrl, windowsName, systemName } from './report-problem.js'

/**
 * FEEDBACK HAS SOMEWHERE TO GO, AND TAKES ONLY WHAT THE BOX SAYS IT TAKES.
 *
 * The beta handover, 2026-09-23: Report a problem showed the log and never
 * said where to send it, so a stuck tester left without a trace. Colin: "for
 * bug reporting we can use what claude code does" -- a Send feedback box. Its
 * report opens on GitHub, filled in: the person's words, the version and the
 * Windows build, and the conversation it was sent from. Nothing else.
 */
const FACTS = { version: '0.306.0', release: '10.0.19045', arch: 'x64' }

describe('the report', () => {
  const url = new URL(feedbackUrl(FACTS, { description: 'The plan card stayed empty.', conversation: 'You: list the files\n\nWren (Codex): data.csv has the most lines.' }))
  const body = url.searchParams.get('body') ?? ''

  it('opens the one destination, as a new report', () => {
    expect(`${url.origin}${url.pathname}`).toBe(REPORT_DESTINATION)
    expect(REPORT_DESTINATION).toBe('https://github.com/StopBanning/locust/issues/new')
    expect([...url.searchParams.keys()]).toEqual(['body'])
  })

  it('carries the words, which Locust and which Windows, and the conversation', () => {
    expect(body.startsWith('The plan card stayed empty.')).toBe(true)
    expect(body).toContain('Locust 0.306.0 on Windows 10, build 19045 (x64)')
    expect(body).toContain('Wren (Codex): data.csv has the most lines.')
  })

  it('carries nothing from the machine beyond that, and says how to add the log by hand', () => {
    expect(body).not.toMatch(/[A-Za-z]:\\|\\Users\\|locust-errors\.log|AppData/)
    expect(body).toContain('Nothing else was attached.')
  })

  it('without a conversation, says nothing about one', () => {
    const plain = new URL(feedbackUrl(FACTS, { description: 'Settings would not open.' })).searchParams.get('body') ?? ''
    expect(plain).not.toContain('conversation it was sent from')
  })

  it('keeps a long conversation\'s END, and the whole address under the limit', () => {
    const long = Array.from({ length: 400 }, (_, index) => `line ${String(index)}: ${'é'.repeat(20)}`).join('\n')
    const cut = feedbackUrl(FACTS, { description: 'It froze.', conversation: long })
    expect(cut.length).toBeLessThanOrEqual(MAX_REPORT_URL)
    const text = new URL(cut).searchParams.get('body') ?? ''
    expect(text).toContain('line 399:')
    expect(text).not.toContain('line 0:')
    expect(text).toContain('(its latest part)')
    expect(text.startsWith('It froze.')).toBe(true)
  })

  it('names Windows 11 by its build, since both say 10.0', () => {
    expect(windowsName('10.0.22631', 'x64')).toBe('Windows 11, build 22631 (x64)')
    expect(windowsName('10.0.19045', 'arm64')).toBe('Windows 10, build 19045 (arm64)')
    expect(windowsName('6.1.7601', 'x64')).toBe('Windows 6.1.7601 (x64)')
  })
})

describe('the renderer names no address for it', () => {
  const preload = readFileSync(join(__dirname, '..', 'shared', 'desktop-api.ts'), 'utf8')

  it('hands over the words, never a URL', () => {
    const line = preload.split('\n').find((text) => text.includes('sendFeedback:')) ?? ''
    expect(line).toMatch(/sendFeedback: \(report: FeedbackReport\) => ipcRenderer\.invoke\(FEEDBACK_CHANNEL, report\)/)
  })
})

// The first macOS build, 2026-09-29: a Mac read "Windows 24.1.0" in its report.
describe('the system a report names', () => {
  it('is macOS on a Mac, and Windows as before', () => {
    expect(systemName({ platform: 'darwin', release: '24.1.0', arch: 'arm64' })).toBe('macOS (Darwin 24.1.0, arm64)')
    expect(systemName({ platform: 'win32', release: '10.0.26200', arch: 'x64' })).toBe('Windows 11, build 26200 (x64)')
    expect(systemName({ release: '10.0.19045', arch: 'x64' })).toBe('Windows 10, build 19045 (x64)')
  })
})
