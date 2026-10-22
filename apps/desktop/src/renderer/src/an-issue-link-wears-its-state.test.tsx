import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { IssueLinkChipView, LinkedWords } from './components/ThreadItems.js'
import { createIssueLinkWatch } from './pullLinkWatch.js'
import type { LinkedIssue } from '../../shared/pull-request.js'

/*
 * The chip beside an issue link is the pull request chip's own class and
 * size: the state in GitHub's words and colors (open green, completed violet,
 * not planned grey; never the red of an abandoned pull request), no mark for
 * checks it has not got. The hover says its standing, then the issue's title.
 * It is read when shown, and again when the window comes back after a minute —
 * never polled while it sits on screen.
 */

const open: LinkedIssue = { state: 'open', title: 'The cart forgets' }
const completed: LinkedIssue = { state: 'closed-completed', title: 'The cart forgets' }
const notPlanned: LinkedIssue = { state: 'closed-not-planned', title: 'The cart forgets' }

describe('the chip beside an issue link', () => {
  it('wears open, closed and not planned, its standing and title on hover', () => {
    const openHtml = renderToStaticMarkup(<IssueLinkChipView link={open} />)
    expect(openHtml).toContain('class="lc-prlink"')
    expect(openHtml).toContain('data-issue-state="open"')
    expect(openHtml).toContain('class="lc-prlink__state"')
    expect(openHtml).toContain('>Open</span>')
    expect(openHtml).toContain('title="Open · The cart forgets"')
    expect(openHtml).not.toContain('lc-prlink__mark')
    // Not the pull request's states: a closed pull request's red would say a finished issue failed.
    expect(openHtml).not.toContain('data-state=')

    const completedHtml = renderToStaticMarkup(<IssueLinkChipView link={completed} />)
    expect(completedHtml).toContain('data-issue-state="closed-completed"')
    expect(completedHtml).toContain('>Closed</span>')
    expect(completedHtml).toContain('title="Closed as completed · The cart forgets"')
    expect(completedHtml).toContain('aria-label="Closed as completed · The cart forgets"')
    expect(completedHtml).not.toContain('data-state=')

    const notPlannedHtml = renderToStaticMarkup(<IssueLinkChipView link={notPlanned} />)
    expect(notPlannedHtml).toContain('data-issue-state="closed-not-planned"')
    expect(notPlannedHtml).toContain('>Not planned</span>')
    expect(notPlannedHtml).toContain('title="Closed as not planned · The cart forgets"')
  })

  it('makes a bare issue in the person\'s message a link, as a pull request is', () => {
    const html = renderToStaticMarkup(<LinkedWords text="See https://github.com/acme/app/issues/3 today" />)
    expect(html).toContain('class="lc-link"')
    expect(html).toContain('https://github.com/acme/app/issues/3')
    expect(html).toContain('See ')
    // No answer has arrived in a static render, so no chip is worn yet.
    expect(html).not.toContain('lc-prlink')
    const written = renderToStaticMarkup(<LinkedWords text="See [the issue](https://github.com/acme/app/issues/3)." />)
    expect(written).toContain('class="lc-link"')
    expect(written).toContain('the issue')
    expect(written).not.toContain('lc-prlink')
  })
})

const flush = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await new Promise((resolve) => setTimeout(resolve, 0))
}

describe('when an issue link is read again', () => {
  it('reads on show, again on focus after a minute, and never on a timer', async () => {
    let time = 10_000
    let calls = 0
    let onFocus: (() => void) | undefined
    const watch = createIssueLinkWatch({
      now: () => time,
      read: () => {
        calls += 1
        return Promise.resolve(open)
      },
      onFocus: (fn) => {
        onFocus = fn
        return () => undefined
      }
    })
    const address = { owner: 'acme', repo: 'app', number: 3 }
    // Not on screen: nothing is read.
    expect(watch.current(address)).toBeUndefined()
    expect(calls).toBe(0)

    const unwatch = watch.watch(address, () => undefined)
    await flush()
    expect(calls).toBe(1)
    expect(watch.current(address)).toEqual(open)

    // The window comes back inside the minute: no new read.
    time += 30_000
    onFocus?.()
    await flush()
    expect(calls).toBe(1)

    // The window comes back after the minute: read again.
    time += 31_000
    onFocus?.()
    await flush()
    expect(calls).toBe(2)
    unwatch()
  })

  it('says nothing and wears nothing when gh cannot answer', async () => {
    const watch = createIssueLinkWatch({
      read: () => Promise.reject(new Error('spawn gh ENOENT'))
    })
    const address = { owner: 'acme', repo: 'app', number: 3 }
    const unwatch = watch.watch(address, () => undefined)
    await flush()
    expect(watch.current(address)).toBeUndefined()
    unwatch()
  })
})
