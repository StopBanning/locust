import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { LinkedWords, PullLinkChipView } from './components/ThreadItems.js'
import type { LinkedPullRequest } from '../../shared/pull-request.js'

/*
 * The chip is a real class beside the link, the same size as the sentence:
 * the state in words, and the checks as one mark. The hover says it in full.
 */

const running: LinkedPullRequest = { state: 'open', checks: 'running', passed: 3, failed: 0, running: 1, total: 4 }
const failing: LinkedPullRequest = { state: 'open', checks: 'failing', passed: 1, failed: 1, running: 0, total: 2 }
const merged: LinkedPullRequest = { state: 'merged', checks: 'none', passed: 2, failed: 0, running: 0, total: 2 }

describe('the chip beside a pull request link', () => {
  it('wears the state in words, and the checks as one mark', () => {
    const open = renderToStaticMarkup(<PullLinkChipView link={running} />)
    expect(open).toContain('class="lc-prlink"')
    expect(open).toContain('data-state="open"')
    expect(open).toContain('data-checks="running"')
    expect(open).toContain('class="lc-prlink__state"')
    expect(open).toContain('>Open</span>')
    expect(open).toContain('lc-prlink__mark')
    expect(open).toContain('lc-tone-amber')
    expect(open).toContain('Open · 3 of 4 checks passed · 1 running')

    const failed = renderToStaticMarkup(<PullLinkChipView link={failing} />)
    expect(failed).toContain('data-checks="failing"')
    expect(failed).toContain('lc-tone-red')
    expect(failed).toContain('Open · 1 of 2 checks passed · 1 failing')

    const done = renderToStaticMarkup(<PullLinkChipView link={merged} />)
    expect(done).toContain('data-state="merged"')
    expect(done).toContain('>Merged</span>')
    expect(done).not.toContain('lc-prlink__mark')
    expect(done).toContain('title="Merged"')
  })

  it('makes a bare pull request in the person\'s message a link, and a bare issue one too', () => {
    expect(renderToStaticMarkup(<LinkedWords text="The first thing" />)).toBe('The first thing')
    expect(renderToStaticMarkup(<LinkedWords text="See https://github.com/acme/app/issues/3" />)).toContain('class="lc-link"')
    const html = renderToStaticMarkup(<LinkedWords text="Opened https://github.com/acme/app/pull/42 today" />)
    expect(html).toContain('class="lc-link"')
    expect(html).toContain('https://github.com/acme/app/pull/42')
    expect(html).toContain('Opened ')
    const issue = renderToStaticMarkup(<LinkedWords text="See [the issue](https://github.com/acme/app/issues/3)." />)
    expect(issue).toContain('class="lc-link"')
    expect(issue).toContain('the issue')
    expect(issue).not.toContain('lc-prlink')
  })
})
