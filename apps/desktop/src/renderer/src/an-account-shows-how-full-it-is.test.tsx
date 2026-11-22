import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { LocalRuntimeId, PublicRuntimeStatus } from '../../shared/ipc.js'
import { AgentMark } from './components/AgentMark.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { FirstLaunch } from './components/FirstLaunch.js'
import { usageWindowsOf } from './missionView.js'

/**
 * AN ACCOUNT SHOWS HOW FULL IT IS -- ON HOVER (0.389).
 *
 * 0.388 ringed each mark on Home with its usage. Colin, 2026-09-27: "it was
 * just very clean showing the agents earlier ... im sure you can cook up
 * something better", "the hover logo wasnt working", and "when you hover the
 * logo the usage window can appear". So the row is plain marks again, and
 * each opens a card on hover or focus: name, state, and a bar per window.
 */
// Far ahead: a reset that has passed is drawn as reset (0.406), and a date
// that is "later today" when written stops being so the next morning.
const RESETS = '2099-09-27T22:10:00.000Z'
const ready = (id: LocalRuntimeId, name: string): PublicRuntimeStatus => ({ id, displayName: name, installed: true, version: '2.1.283', auth: 'authenticated', ready: true, status: 'ready' })

describe('a usage reading', () => {
  it('reads as windows, fullest first, each with its reset', () => {
    const windows = usageWindowsOf(`5-hour window 35% used · resets ${RESETS} · 7-day window 12% used`)
    expect(windows.map((window) => [window.name, window.percent])).toEqual([['5-hour window', 35], ['7-day window', 12]])
    expect(windows[0]?.resets).toMatch(/^\d{1,2}:\d{2} [AP]M$|^\w{3} \d{1,2}:\d{2} [AP]M$/)
    expect(windows[1]?.resets).toBeUndefined()
  })
})

describe("an agent's mark on Home", () => {
  const card = (usage?: string): string =>
    renderToStaticMarkup(<AgentMark runtime="claude" name="Claude Code" state="Ready · 2.1.283" {...(usage === undefined ? {} : { usage })} />)

  it('is a thing to point at, named with everything its card says', () => {
    const html = card(`5-hour window 35% used · resets ${RESETS}`)
    expect(html).toMatch(/^<span class="lc-agentmark" tabindex="0" role="img" aria-label="Claude Code\. Ready · 2\.1\.283\. 35% of the 5-hour window used, resets [^"]+"/)
  })

  it('opens a card with its name, its state, and a bar for each window', () => {
    const html = card(`5-hour window 35% used · resets ${RESETS} · 7-day window 12% used`)
    expect(html).toContain('<span class="lc-agentcard" aria-hidden="true">')
    expect(html).toContain('<span class="lc-agentcard__name">Claude Code</span><span class="lc-agentcard__state">Ready · 2.1.283</span>')
    expect(html).toContain('<span class="lc-agentcard__label">5-hour window</span><span class="lc-agentcard__percent">35%</span>')
    expect(html).toContain('style="width:35%"')
    expect(html).toContain('style="width:12%"')
    expect(html).toMatch(/<span class="lc-agentcard__resets">resets [^<]+<\/span>/)
  })

  it('draws a bar amber from 80% and red when spent', () => {
    expect(card('5-hour window 79% used')).toContain('class="lc-agentcard__window"')
    expect(card('5-hour window 80% used')).toContain('class="lc-agentcard__window is-pressing"')
    expect(card('weekly window 100% used')).toContain('class="lc-agentcard__window is-spent"')
  })

  it('draws no meter for an agent whose runs reported none: its name and state alone', () => {
    const html = card()
    expect(html).not.toContain('lc-agentcard__window')
    expect(html).toContain('aria-label="Claude Code. Ready · 2.1.283"')
  })

  /*
   * 0.406, Colin: "my claude code usage hasnt seem to have updated since
   * implementation". His reading was from Locust's last Claude run the night
   * before; its 5-hour window had reset hours ago and was still drawn at 20%.
   */
  it('draws a window reset since the reading as reset, without its old bar, and says when the reading was taken', () => {
    const said = '7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used · resets 2026-09-27T05:30:00.000Z · from a run at 2026-09-27T01:23:19.233Z'
    const html = renderToStaticMarkup(<AgentMark runtime="claude" name="Claude Code" state="Ready" usage={said} now={new Date('2026-09-27T15:30:00.000Z')} />)
    expect(html).toContain('style="width:66%"')
    expect(html).not.toContain('style="width:20%"')
    expect(html).toMatch(/<span class="lc-agentcard__window is-reset"><span class="lc-agentcard__label">5-hour window<\/span><span class="lc-agentcard__resets">reset since \([^)]+\)<\/span><\/span>/)
    expect(html).toMatch(/<span class="lc-agentcard__read">From Locust&#x27;s last run on it, [^<]+\. Use outside Locust since then isn&#x27;t counted\.<\/span>/)
  })
})

describe("Home's agents line", () => {
  it('is the marks alone, each carrying its card; no rings', () => {
    const html = renderToStaticMarkup(
      <FirstLaunch
        runtimes={[ready('codex', 'Codex CLI'), ready('claude', 'Claude Code')]}
        limitedRuntimes={new Map()}
        usageWindows={new Map([['claude', `5-hour window 35% used · resets ${RESETS}`]])}
        discoveryPhase="ready"
        workspacePath="C:\\work"
        teammateCount={1}
        team={[]}
        onMessageTeammate={() => undefined}
        onChooseFolder={() => undefined}
        onInstall={() => undefined}
      />
    )
    expect([...html.matchAll(/class="lc-agentmark"/g)]).toHaveLength(2)
    expect(html).not.toContain('lc-usagering')
    expect(html).toMatch(/aria-label="Claude Code\. Ready · 2\.1\.283\. 35% of the 5-hour window used/)
    expect(html).toContain('aria-label="Codex CLI. Ready · 2.1.283"')
  })
})

function chip(used: number | undefined): string {
  const props = {
    runtimes: [ready('claude', 'Claude Code')],
    limitedRuntimes: new Map(),
    usageWindows: used === undefined ? new Map() : new Map([['claude', `5-hour window ${String(used)}% used · resets ${RESETS}`]]),
    discoveryPhase: 'ready',
    models: [],
    resolvedModels: new Map(),
    route: { runtime: 'claude', model: 'sonnet', routeId: 'claude:sonnet' },
    mode: 'accept-edits',
    onModeChange: () => undefined,
    onRouteChange: () => undefined,
    onSend: () => undefined,
    running: false,
    platform: 'win32'
  } as unknown as ComposerProps
  const html = renderToStaticMarkup(<Composer {...props} />)
  const start = html.lastIndexOf('<button', html.indexOf('aria-haspopup="listbox"'))
  return html.slice(start, html.indexOf('</button>', start))
}

describe('the model button', () => {
  it('is quiet with room left, and wears its amber dot from 80% -- as before the rings', () => {
    expect(chip(40)).not.toContain('is-pressing')
    expect(chip(85)).toMatch(/class="lc-control lc-control--boxed is-pressing"/)
    expect(chip(85)).not.toContain('lc-usagering')
  })
})
