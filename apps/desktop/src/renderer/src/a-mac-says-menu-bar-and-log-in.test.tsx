import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { machineWords } from '../../shared/machine-words.js'
import { RemoteMcpSettings, type StayingReachable } from './components/RemoteMcpSettings.js'

/**
 * A MAC SAYS MENU BAR AND LOG IN (0.843). The first Mac run (2026-10-30) found "the tray" and "sign in to Windows"
 * on a Mac. A closed Locust keeps running in a Mac's menu bar, and starts when the person logs in to it; Windows
 * keeps the words it always had.
 */
const ON = { enabled: true, address: 'https://app.locust.lol/v1/room/abc/mcp', asking: [], apps: [] }
const stay = (over: Partial<StayingReachable> = {}): StayingReachable => ({ loginItem: { openAtLogin: false, available: true }, keepRunning: false, ...over })
const draw = (s: StayingReachable, platform: string): string => renderToStaticMarkup(<RemoteMcpSettings initial={ON} stay={s} platform={platform} />)

describe('a Mac says menu bar and log in', () => {
  it('names each machine as its own settings do', () => {
    expect(machineWords('darwin')).toEqual({ tray: 'menu bar', signIn: 'log in to this Mac', computer: 'Mac' })
    expect(machineWords('win32')).toEqual({ tray: 'tray', signIn: 'sign in to Windows', computer: 'PC' })
    expect(machineWords(undefined)).toEqual(machineWords('win32'))
  })

  it('offers to keep Locust running in the menu bar, and to start it when you log in, on a Mac', () => {
    const html = draw(stay(), 'darwin')
    expect(html).toContain('Keep it running in the menu bar when you close its window, and start it when you log in to this Mac.')
    expect(html).not.toMatch(/tray|Windows/)
    const reached = draw(stay({ keepRunning: true, loginItem: { openAtLogin: true, available: true } }), 'darwin')
    expect(reached).toContain('it starts when you log in to this Mac, and it keeps running in the menu bar when you close its window. Quit from the menu bar')
  })

  // F8: Settings > AI agents said "this PC's sign-in" on a Mac; every place that named the PC now asks the machine.
  it('names the computer by the machine, never "this PC" in the words themselves', () => {
    for (const file of ['./agentAccounts.ts', './components/AgentAccountsSettings.tsx', './components/CloudSetupDialog.tsx', './components/Screens.tsx']) {
      expect(readFileSync(fileURLToPath(new URL(file, import.meta.url)), 'utf8')).not.toMatch(/\bthis PC\b|\bThis PC\b/)
    }
  })

  it('keeps Windows exactly as it was', () => {
    expect(draw(stay(), 'win32')).toContain('Keep it running in the tray when you close its window, and start it when you sign in to Windows.')
  })
})
