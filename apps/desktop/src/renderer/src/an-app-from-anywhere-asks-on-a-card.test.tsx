import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { RemoteConnectCard } from './components/RemoteConnectCard.js'
import { RemoteMcpSettings } from './components/RemoteMcpSettings.js'

/**
 * AN APP FROM ANYWHERE ASKS ON A CARD (0.841). Claude on the web signing in to Locust is answered only here: a card on
 * every screen names the app, where it sends the person back, and the six digits its sign-in page shows. Settings
 * gives the address to paste into Claude, and lists each app with Remove.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('an app from anywhere', () => {
  it('asks on a card naming it, where it goes back to, and the digits its page shows', () => {
    const html = renderToStaticMarkup(<RemoteConnectCard initial={[{ request: 'rq', name: 'Claude', host: 'claude.ai', match: '482913', until: 0 }]} />)
    expect(html).toContain('role="alertdialog"')
    expect(html).toContain('Claude wants to use your Locust team</h2>')
    expect(html).toContain('It sends you back to claude.ai.')
    expect(html).toContain('<b class="lc-remote-connect__match">482 913</b>')
    expect(html).toMatch(/>Allow<\/button>.*>Deny<\/button>/)
    expect(renderToStaticMarkup(<RemoteConnectCard initial={[]} />)).toBe('')
  })

  it('is answered on every screen, and only from the window', () => {
    expect(read('./App.tsx')).toMatch(/<OrganizationPolicyNotice \/>\s*\{\/\*[^*]*\*\/\}\s*<RemoteConnectCard \/>/)
    const host = read('../../main/index.ts')
    expect(host).toContain('ipcMain.handle(REMOTE_MCP_DECIDE, (event, request: unknown, allow: unknown) => fromOwnWindow(event) && remoteMcp !== undefined ? remoteMcp.answerSignIn(request, allow)')
    // The relay's asks go to the PC module, and its answers back by name.
    expect(read('../../main/phone-anywhere.ts')).toContain("send({ t: 'mcp-answer', ask, status: answer.status, body: answer.body })")
  })

  it('gives Settings the address for Claude, and each app with Remove', () => {
    const html = renderToStaticMarkup(<RemoteMcpSettings initial={{ enabled: true, address: 'https://app.locust.lol/v1/room/abc/mcp', asking: [],
      apps: [{ id: 'g1', name: 'Claude', host: 'claude.ai', added: '2026-10-29T10:00:00.000Z' }] }} />)
    expect(html).toContain('<h3 class="lc-settings__heading">Use from anywhere</h3>')
    expect(html).toContain('In Claude, open Settings, then Connectors, choose Add custom connector, and paste this address:')
    expect(html).toContain('<pre class="lc-mcp-setup lc-mono">https://app.locust.lol/v1/room/abc/mcp</pre>')
    expect(html).toContain('<span class="lc-phone__name">Claude</span>')
    expect(html).toMatch(/claude\.ai · connected Oct 29/)
    expect(html).toContain('>Remove</button>')
    const off = renderToStaticMarkup(<RemoteMcpSettings initial={{ enabled: false, apps: [], asking: [] }} />)
    expect(off).toContain('aria-checked="false"')
    expect(off).not.toContain('app.locust.lol')
    expect(read('./components/Screens.tsx')).toMatch(/Let your other AI apps use Locust<\/h2>\} \/>\s*<RemoteMcpSettings stay=\{\{ loginItem, keepRunning,/)
  })
})
