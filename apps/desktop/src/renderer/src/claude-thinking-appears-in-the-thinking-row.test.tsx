import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { createClaudeEventNormalizer } from '@teammate/runtime-adapters'
import { ActivityCard } from './components/ActivityCard.js'
import { buildThread } from './missionView.js'

// The real adapter feeds the existing row; Claude needs no special rendering path.
describe('Claude thinking appears in the thinking row', () => {
  it('renders the shown thinking from the adapter in one expandable row', () => {
    const claude = createClaudeEventNormalizer({ runId: 'r', now: () => new Date('2026-11-10T12:00:00Z') })
    const events = [
      { type: 'stream_event', event: { type: 'content_block_start', index: 0, content_block: { type: 'thinking', thinking: '' } } },
      { type: 'stream_event', event: { type: 'content_block_delta', index: 0, delta: { type: 'thinking_delta', thinking: 'Read the configuration before changing the port.' } } },
      { type: 'stream_event', event: { type: 'content_block_stop', index: 0 } }
    ].flatMap((value, sequence) => claude.accept({ sequence: sequence + 1, raw: JSON.stringify(value) }))
    const details = buildThread(events, { running: false }).flatMap(item => item.type === 'steps' ? item.details : [])
    expect(details.filter(detail => detail.kind === 'reasoning')).toHaveLength(1)
    const html = renderToStaticMarkup(<ActivityCard variant="steps" summary="" trace={[]} finished openByDefault details={details} runtimeName="Claude Code" workspacePath={undefined} />)
    expect(html).toContain('lc-steps__thought')
    expect(html).toContain('Read the configuration before changing the port.')
  })
})
