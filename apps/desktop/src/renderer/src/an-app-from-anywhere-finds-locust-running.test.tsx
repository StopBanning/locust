import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { keepItRunning, RemoteMcpSettings, type StayingReachable } from './components/RemoteMcpSettings.js'

/**
 * AN APP FROM ANYWHERE FINDS LOCUST RUNNING (0.842, Colin: "right now it only works if it's open on my pc"). No app
 * can open Locust: once it quits, nothing on the computer hears the relay. Use from anywhere says so, and its one
 * button turns on the Startup switches that keep it reachable -- the tray when the window closes, and Windows sign-in.
 */
const ON = { enabled: true, address: 'https://app.locust.lol/v1/room/abc/mcp', asking: [], apps: [] }
const stay = (over: Partial<StayingReachable> = {}): StayingReachable => ({ loginItem: { openAtLogin: false, available: true }, keepRunning: false, ...over })
const draw = (s: StayingReachable): string => renderToStaticMarkup(<RemoteMcpSettings initial={ON} stay={s} />)
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('an app from anywhere finds Locust running', () => {
  it('says no app can open a Locust that quit, and offers to keep it running', () => {
    const html = draw(stay())
    expect(html).toContain('once it quits, nothing on this computer can open it')
    expect(html).toContain('and start it when you sign in to Windows.')
    expect(html).toContain('>Keep it running</button>')
  })

  it('says it stays reachable once both are on, with no button', () => {
    const html = draw(stay({ keepRunning: true, loginItem: { openAtLogin: true, available: true } }))
    expect(html).toContain('Locust stays reachable while this computer is on and awake: it starts when you sign in to Windows, and it keeps running in the tray')
    expect(html).not.toContain('Keep it running</button>')
    // Where Windows sign-in cannot be set, the tray alone is enough.
    expect(draw(stay({ keepRunning: true, loginItem: { openAtLogin: false, available: false } }))).toContain('Locust stays reachable while this computer is on and awake: it keeps running')
    expect(draw(stay({ keepRunning: false, loginItem: { openAtLogin: true, available: true } }))).toContain('>Keep it running</button>')
  })

  it('turns on only what is off and can be on, and turns nothing off', () => {
    const onKeepRunningChange = vi.fn()
    const onLoginItemChange = vi.fn()
    keepItRunning(stay({ onKeepRunningChange, onLoginItemChange }))
    expect(onKeepRunningChange).toHaveBeenCalledWith(true)
    expect(onLoginItemChange).toHaveBeenCalledWith(true)
    onKeepRunningChange.mockClear(); onLoginItemChange.mockClear()
    keepItRunning(stay({ keepRunning: true, loginItem: { openAtLogin: false, available: false }, onKeepRunningChange, onLoginItemChange }))
    expect(onKeepRunningChange).not.toHaveBeenCalled()
    expect(onLoginItemChange).not.toHaveBeenCalled()
  })

  it('is given the Startup switches themselves, and the relay says so when Locust is not running', () => {
    expect(read('./components/Screens.tsx')).toContain('<RemoteMcpSettings stay={{ loginItem, keepRunning, ...(onLoginItemChange === undefined ? {} : { onLoginItemChange }), ...(onKeepRunningChange === undefined ? {} : { onKeepRunningChange }) }} />')
    expect(read('../../../../relay/src/room.ts')).toContain('nothing there can open it. Open Locust there and try again. To keep it reachable, turn on Keep it running under Use from anywhere')
  })
})
