import { readFileSync } from 'node:fs'
import { Script } from 'node:vm'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import {
  BROWSER_PICKER_SCRIPT,
  BROWSER_POINT_CANCEL,
  BROWSER_POINT_PROBE,
  BROWSER_POINT_RESTORE,
  BROWSER_POINT_SCRIPTS,
  pointInBrowser,
  type BrowserPointPage
} from './browser-point.js'
import {
  MAX_POINTED_HTML,
  PAGE_POINT_MARK,
  POINT_BLOCKED,
  beginBrowserPoint,
  browserPointBlock,
  compactStyles,
  messageOfPagePoints,
  readBrowserPoint,
  withoutPagePoint,
  type BrowserPoint,
  type PagePointChip
} from '../shared/browser-point.js'
import { policyBrowserRefusal } from '../shared/organization-policy.js'

/**
 * POINT AT A PART OF THE BROWSER. The window asks main to point. Main runs
 * its own script in the browser page, scrubs what comes back, and the chat
 * box holds the parts until Send. These are the parts that do not need a window.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const allowed = new Set<string>(BROWSER_POINT_SCRIPTS)
const box = { x: 4, y: 8, width: 180, height: 40 }
const viewport = { width: 800, height: 600 }

function pageFor(raw: unknown, probe: unknown = true): { readonly page: BrowserPointPage; readonly ran: string[]; readonly captures: () => number } {
  const ran: string[] = []
  let captured = 0
  const page: BrowserPointPage = {
    executeJavaScript: (code) => {
      ran.push(code)
      if (!allowed.has(code)) return Promise.reject(new Error('a script the window handed in'))
      if (code === BROWSER_POINT_PROBE) return Promise.resolve(probe)
      if (code === BROWSER_PICKER_SCRIPT) return Promise.resolve(raw)
      return Promise.resolve(undefined)
    },
    capturePage: () => {
      captured += 1
      return Promise.resolve({ isEmpty: () => false, toPNG: () => new Uint8Array([1, 2, 3, 4]) })
    },
    getURL: () => 'http://127.0.0.1:9786/',
    isDestroyed: () => false,
    getZoomFactor: () => 1
  }
  return { page, ran, captures: () => captured }
}

const picked = {
  selector: 'button#save',
  html: '<button id="save">Save</button>',
  htmlLength: 31,
  rect: box,
  viewport,
  styles: { width: '180px', height: '40px', display: 'flex', color: 'rgb(11, 22, 33)' }
}

describe('pointing at the browser page', () => {
  it('the picker script parses', () => {
    expect(() => new Script(BROWSER_PICKER_SCRIPT)).not.toThrow()
    expect(() => new Script(BROWSER_POINT_CANCEL)).not.toThrow()
    expect(() => new Script(BROWSER_POINT_PROBE)).not.toThrow()
    expect(() => new Script(BROWSER_POINT_RESTORE)).not.toThrow()
  })

  it('the picker request goes through main only', async () => {
    const ipc = read('./browser-ipc.ts')
    const preload = read('../shared/desktop-api.ts')
    const panel = read('../renderer/src/components/BrowserPanel.tsx')
    expect(ipc).toContain('return host.point()')
    expect(ipc).not.toContain('return host.point(event)')
    expect(preload).toContain('point: () => ipcRenderer.invoke(BROWSER_POINT_CHANNEL)')
    expect(ipc).not.toContain('executeJavaScript')
    expect(panel).not.toContain('executeJavaScript')
    const { page, ran, captures } = pageFor(picked)
    const answer = await pointInBrowser(page, 1_000)
    expect(ran.length).toBeGreaterThan(0)
    expect(ran.every((code) => allowed.has(code))).toBe(true)
    expect(answer.ok).toBe(true)
    if (answer.ok) expect(answer.png).toEqual(new Uint8Array([1, 2, 3, 4]))
    expect(captures()).toBe(1)
  })

  it('form values never appear in a pick', () => {
    const html = '<form><input type="text" value="typed-in"><input type="password" value="s3cret-pass"><textarea>typed words</textarea><select><option value="secret-opt" selected>Shown</option></select></form>'
    const readPick = readBrowserPoint({ selector: 'form', html, htmlLength: html.length, rect: box, viewport }, 'http://127.0.0.1:9786/')
    expect(readPick).toBeDefined()
    const quoted = readPick?.point.html ?? ''
    expect(quoted).not.toContain('s3cret-pass')
    expect(quoted).not.toContain('typed words')
    expect(quoted).not.toContain('typed-in')
    expect(quoted).not.toContain('secret-opt')
    expect(quoted).not.toContain('selected')
    expect(quoted).toContain('Shown')
  })

  it('lists the styles that matter for a visual fix', () => {
    const styles = compactStyles({
      width: '180px',
      height: '40px',
      'margin-top': '0px',
      'margin-right': '0px',
      'margin-bottom': '0px',
      'margin-left': '0px',
      'padding-top': '8px',
      'padding-right': '16px',
      'padding-bottom': '8px',
      'padding-left': '16px',
      display: 'flex',
      'flex-direction': 'row',
      position: 'static',
      top: '10px',
      color: 'rgb(11, 22, 33)',
      'border-top-width': '1px',
      'border-right-width': '1px',
      'border-bottom-width': '1px',
      'border-left-width': '1px',
      'border-top-style': 'solid',
      'border-top-color': 'rgb(1, 2, 3)',
      'border-top-left-radius': '6px',
      'border-top-right-radius': '6px',
      'border-bottom-right-radius': '6px',
      'border-bottom-left-radius': '6px',
      opacity: '1'
    })
    const map = Object.fromEntries(styles.map((style) => [style.name, style.value]))
    expect(map.color).toBe('rgb(11, 22, 33)')
    expect(map.display).toBe('flex')
    expect(map['flex-direction']).toBe('row')
    expect(map.padding).toBe('8px 16px')
    expect(map.margin).toBe('0px')
    expect(map.top).toBeUndefined()
    expect(map.border).toBe('1px solid rgb(1, 2, 3)')
    expect(map['border-radius']).toBe('6px')
    expect(map.opacity).toBe('1')
  })

  it('a trimmed HTML says it was trimmed', () => {
    const html = `x${'y'.repeat(MAX_POINTED_HTML + 40)}`
    const readPick = readBrowserPoint({ selector: 'div', html, htmlLength: html.length, rect: box, viewport }, 'http://127.0.0.1:9786/')
    expect(readPick).toBeDefined()
    const block = browserPointBlock(readPick?.point as BrowserPoint, '')
    expect(block).toContain('more characters of its code not shown')
    expect(block).toContain(PAGE_POINT_MARK)
  })

  it('several picks become several blocks', () => {
    const chip = (id: string): PagePointChip => ({
      id,
      note: id === 'one' ? 'the spacing is tight' : '',
      point: { url: 'http://127.0.0.1:9786/', selector: `button#${id}`, html: `<button id="${id}">${id}</button>`, htmlLength: 24, styles: [{ name: 'width', value: '180px' }] }
    })
    const message = messageOfPagePoints([chip('one'), chip('two')])
    expect(message.text.split(PAGE_POINT_MARK)).toHaveLength(3)
    expect(message.text).toContain('Part: button#one')
    expect(message.text).toContain('Part: button#two')
    expect(message.text).toContain('Note: the spacing is tight')
    expect(message.text).toContain('width: 180px')
  })

  it('removing a chip removes its picture', () => {
    const chips: PagePointChip[] = [
      { id: 'a', note: '', attachment: 'shot-a.png', point: { url: 'http://127.0.0.1:9786/', selector: 'button#save', html: '<button>Save</button>', htmlLength: 22, styles: [] } },
      { id: 'b', note: '', attachment: 'shot-b.png', point: { url: 'http://127.0.0.1:9786/', selector: 'p', html: '<p>Hi</p>', htmlLength: 9, styles: [] } }
    ]
    const left = withoutPagePoint(chips, 'a')
    expect(left.map((chip) => chip.id)).toEqual(['b'])
    expect(messageOfPagePoints(left).attachments).toEqual(['shot-b.png'])
    expect(messageOfPagePoints(chips).attachments).toEqual(['shot-a.png', 'shot-b.png'])
  })

  it('browser: false turns pointing off too', () => {
    const loaded = { path: 'C:/policy.json', present: true, policy: { browser: false as const } }
    const refused = beginBrowserPoint(policyBrowserRefusal(loaded))
    expect(refused?.ok).toBe(false)
    expect(refused?.message).toContain('The browser panel is off.')
    expect(beginBrowserPoint(undefined)).toBeUndefined()
    const host = read('./browser-host.ts')
    expect(host).toContain('const refused = beginBrowserPoint(options.policyRefusal?.())')
    expect(host).toContain('if (refused !== undefined) return refused')
  })

  it('a password field is not pictured with its contents', async () => {
    const html = '<input id="pw" type="password" value="s3cret-pass">'
    const { page, captures } = pageFor({ selector: 'input#pw', html, htmlLength: html.length, rect: box, viewport, sensitive: 'password' })
    const answer = await pointInBrowser(page, 1_000)
    expect(captures()).toBe(0)
    expect(answer.ok).toBe(true)
    if (!answer.ok) return
    expect(answer.png).toBeUndefined()
    expect(answer.picked?.pictureNote).toContain('password')
    expect(answer.picked?.html).not.toContain('s3cret-pass')
  })

  it('says when the page does not allow pointing', async () => {
    const blocked = pageFor(picked, false)
    const answer = await pointInBrowser(blocked.page, 1_000)
    expect(answer).toEqual({ ok: false, message: POINT_BLOCKED, blocked: true })
    const thrown: BrowserPointPage = {
      ...blocked.page,
      executeJavaScript: () => Promise.reject(new Error('refused'))
    }
    expect(await pointInBrowser(thrown, 1_000)).toEqual({ ok: false, message: POINT_BLOCKED, blocked: true })
  })
})
