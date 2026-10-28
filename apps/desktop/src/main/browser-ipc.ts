import { ipcMain } from 'electron'
import type { IpcMainInvokeEvent } from 'electron'

import {
  BROWSER_BACK_CHANNEL,
  BROWSER_BOUNDS_CHANNEL,
  BROWSER_CLOSE_PANEL_CHANNEL,
  BROWSER_CLOSE_TAB_CHANNEL,
  BROWSER_FORWARD_CHANNEL,
  BROWSER_NAVIGATE_CHANNEL,
  BROWSER_NEW_TAB_CHANNEL,
  BROWSER_OPEN_PANEL_CHANNEL,
  BROWSER_RELOAD_CHANNEL,
  BROWSER_POINT_CANCEL_CHANNEL,
  BROWSER_POINT_CHANNEL,
  BROWSER_SELECT_TAB_CHANNEL,
  BROWSER_STATE_CHANNEL,
  BROWSER_STOP_CHANNEL,
  type BrowserRect
} from '../shared/browser.js'
import type { BrowserHost } from './browser-host.js'

const isTabId = (id: unknown): id is string => typeof id === 'string' && /^tab_\d+$/.test(id)

function parseRect(raw: unknown): BrowserRect | null {
  if (raw === null || raw === undefined) return null
  if (typeof raw !== 'object' || Array.isArray(raw)) return null
  const r = raw as Record<string, unknown>
  const x = Number(r.x)
  const y = Number(r.y)
  const width = Number(r.width)
  const height = Number(r.height)
  if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(width) || !Number.isFinite(height)) return null
  if (width <= 0 || height <= 0 || width > 10000 || height > 10000) return null
  return { x, y, width, height }
}

export function registerBrowserIpc(host: BrowserHost, own: (event: IpcMainInvokeEvent) => boolean): void {
  ipcMain.handle(BROWSER_STATE_CHANNEL, (event) => {
    if (!own(event)) throw new Error('Only Locust’s window can read browser state.')
    return host.getState()
  })
  ipcMain.handle(BROWSER_OPEN_PANEL_CHANNEL, async (event, rawUrl: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can open the browser panel.')
    const url = typeof rawUrl === 'string' && rawUrl.length <= 4096 ? rawUrl : undefined
    return host.openPanel(url)
  })
  ipcMain.handle(BROWSER_CLOSE_PANEL_CHANNEL, (event) => {
    if (!own(event)) throw new Error('Only Locust’s window can close the browser panel.')
    return host.closePanel()
  })
  ipcMain.handle(BROWSER_BOUNDS_CHANNEL, (event, rawRect: unknown) => {
    if (!own(event)) return
    host.setBounds(parseRect(rawRect))
  })
  ipcMain.handle(BROWSER_NAVIGATE_CHANNEL, async (event, rawUrl: unknown, rawTabId: unknown) => {
    if (!own(event) || typeof rawUrl !== 'string' || rawUrl.length > 4096) {
      return { ok: false, message: 'Invalid address. The current page stayed open.', state: host.getState() }
    }
    return host.navigate(rawUrl, isTabId(rawTabId) ? rawTabId : undefined)
  })
  ipcMain.handle(BROWSER_BACK_CHANNEL, async (event, rawTabId: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can navigate the browser.')
    return host.goBack(isTabId(rawTabId) ? rawTabId : undefined)
  })
  ipcMain.handle(BROWSER_FORWARD_CHANNEL, async (event, rawTabId: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can navigate the browser.')
    return host.goForward(isTabId(rawTabId) ? rawTabId : undefined)
  })
  ipcMain.handle(BROWSER_RELOAD_CHANNEL, async (event, rawTabId: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can reload the browser.')
    return host.reload(isTabId(rawTabId) ? rawTabId : undefined)
  })
  ipcMain.handle(BROWSER_STOP_CHANNEL, async (event, rawTabId: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can stop loading in the browser.')
    return host.stop(isTabId(rawTabId) ? rawTabId : undefined)
  })
  ipcMain.handle(BROWSER_NEW_TAB_CHANNEL, async (event, rawUrl: unknown, rawProfile: unknown) => {
    if (!own(event)) throw new Error('Only Locust’s window can open a browser tab.')
    const url = typeof rawUrl === 'string' && rawUrl.length <= 4096 ? rawUrl : undefined
    // A teammate's id, whose own sign-ins the tab opens in (0.839); the host checks it still has them.
    const profile = typeof rawProfile === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(rawProfile) ? rawProfile : undefined
    return host.newTab(url, profile)
  })
  ipcMain.handle(BROWSER_SELECT_TAB_CHANNEL, (event, rawTabId: unknown) => {
    if (!own(event) || !isTabId(rawTabId)) return host.getState()
    return host.selectTab(rawTabId)
  })
  ipcMain.handle(BROWSER_CLOSE_TAB_CHANNEL, (event, rawTabId: unknown) => {
    if (!own(event) || !isTabId(rawTabId)) return host.getState()
    return host.closeTab(rawTabId)
  })
  // The window asks to point. It does not send a script.
  ipcMain.handle(BROWSER_POINT_CHANNEL, (event) => {
    if (!own(event)) return { ok: false, message: 'That request was rejected. The page was not pointed at.' }
    return host.point()
  })
  ipcMain.handle(BROWSER_POINT_CANCEL_CHANNEL, (event) => {
    if (!own(event)) return
    return host.cancelPoint()
  })
}
