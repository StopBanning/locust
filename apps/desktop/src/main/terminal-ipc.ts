import { clipboard, ipcMain, shell } from 'electron'
import type { IpcMainInvokeEvent } from 'electron'
import { TERMINAL_SHELLS, TERMINAL_CREATE, TERMINAL_INPUT, TERMINAL_RESIZE, TERMINAL_CLOSE, TERMINAL_LINK, TERMINAL_COPY, type TerminalContext } from '../shared/terminal.js'
import type { createTerminalHost } from './terminal-host.js'

/** Every channel is window-only. Creation accepts identities and a shell id, never an executable or directory. */
export function registerTerminalIpc(host: ReturnType<typeof createTerminalHost>, own: (event: IpcMainInvokeEvent) => boolean): void {
  const idOk = (id: unknown): id is string => typeof id === 'string' && /^[a-f0-9-]{36}$/.test(id)
  const wrap = async <T>(run: () => T | Promise<T>) => {
    try { return { ok: true, data: await run() } }
    catch (error) { return { ok: false, message: error instanceof Error ? error.message : 'The terminal is unavailable. Your conversations are unchanged.' } }
  }
  ipcMain.handle(TERMINAL_SHELLS, event => wrap(() => { if (!own(event)) throw new Error('Only Locust’s window can open terminals.'); return host.shells() }))
  ipcMain.handle(TERMINAL_CREATE, (event, context: unknown, shellId: unknown) => wrap(() => {
    if (!own(event) || typeof context !== 'object' || context === null || Array.isArray(context)) throw new Error('The terminal request was refused.')
    const value = context as Record<string, unknown>
    if (Object.keys(value).some(key => key !== 'missionId' && key !== 'teammateId') || Object.values(value).some(id => typeof id !== 'string' || id.length === 0 || id.length > 128)
      || (shellId !== undefined && (typeof shellId !== 'string' || shellId.length > 40))) throw new Error('The terminal request was refused.')
    return host.create(value as TerminalContext, shellId as string | undefined)
  }))
  ipcMain.handle(TERMINAL_INPUT, (event, id: unknown, data: unknown) => {
    if (own(event) && idOk(id) && typeof data === 'string' && data.length <= 64 * 1024) host.input(id, data)
  })
  ipcMain.handle(TERMINAL_RESIZE, (event, id: unknown, cols: unknown, rows: unknown) => {
    if (own(event) && idOk(id) && Number.isInteger(cols) && Number.isInteger(rows) && Number(cols) >= 2 && Number(cols) <= 1000 && Number(rows) >= 1 && Number(rows) <= 500) host.resize(id, Number(cols), Number(rows))
  })
  ipcMain.handle(TERMINAL_CLOSE, (event, id: unknown) => { if (own(event) && idOk(id)) return host.close(id) })
  ipcMain.handle(TERMINAL_COPY, (event, text: unknown) => wrap(() => {
    if (!own(event) || typeof text !== 'string' || text.length > 16 * 1024 * 1024) throw new Error('That selection could not be copied. Your terminal is unchanged.')
    clipboard.writeText(text)
  }))
  ipcMain.handle(TERMINAL_LINK, (event, url: unknown) => wrap(async () => {
    if (!own(event) || typeof url !== 'string' || url.length > 8192 || !['https:', 'http:'].includes(new URL(url).protocol)) throw new Error('Only HTTP and HTTPS links can open from a terminal. Your shell is unchanged.')
    await shell.openExternal(url)
  }))
}
