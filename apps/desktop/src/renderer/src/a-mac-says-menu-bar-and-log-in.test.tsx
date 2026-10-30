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
    expect(machineWords('darwin')).toEqual({ tray: 'menu bar', signIn: 'log in to this Mac' })
    expect(machineWords('win32')).toEqual({ tray: 'tray', signIn: 'sign in to Windows' })
    expect(machineWords(undefined)).toEqual(machineWords('win32'))
  })

  it('offers to keep Locust running in the menu bar, and to start it when you log in, on a Mac', () => {
    const html = draw(stay(), 'darwin')
    expect(html).toContain('Keep it running in the menu bar when you close its window, and start it when you log in to this Mac.')
    expect(html).not.toMatch(/tray|Windows/)
    const reached = draw(stay({ keepRunning: true, loginItem: { openAtLogin: true, available: true } }), 'darwin')
    expect(reached).toContain('it starts when you log in to this Mac, and it keeps running in the menu bar when you close its window. Quit from the menu bar')
  })

  it('keeps Windows exactly as it was', () => {
    expect(draw(stay(), 'win32')).toContain('Keep it running in the tray when you close its window, and start it when you sign in to Windows.')
  })
})
