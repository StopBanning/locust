import { useEffect, useRef, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import type { TerminalContext, TerminalEvent, TerminalShell, TerminalTab } from '../../../shared/terminal.js'
import { terminalInputChunks } from '../../../shared/terminal.js'
import { useDrawnTheme } from '../drawnTheme.js'
import { Icon } from './Icon.js'

function colors() {
  const css = getComputedStyle(document.documentElement)
  const token = (name: string) => css.getPropertyValue(name).trim()
  return { background: token('--lc-bg-chrome'), foreground: token('--lc-text-primary'), cursor: token('--lc-text-primary'),
    selectionBackground: token('--lc-bg-selected'), selectionForeground: token('--lc-text-primary'),
    black: token('--lc-bg-app'), brightBlack: token('--lc-text-muted'), white: token('--lc-text-secondary'), brightWhite: token('--lc-text-primary'),
    red: token('--lc-red'), brightRed: token('--lc-red'), green: token('--lc-green'), brightGreen: token('--lc-green'),
    yellow: token('--lc-amber'), brightYellow: token('--lc-amber'), blue: token('--lc-blue'), brightBlue: token('--lc-blue'),
    magenta: token('--lc-violet'), brightMagenta: token('--lc-violet'), cyan: token('--lc-blue'), brightCyan: token('--lc-blue') }
}

/** Mounted once, after the first opening. Hidden panels and inactive tabs retain their xterm scrollback. */
export default function TerminalPanel({ visible, context, onClose }: { readonly visible: boolean; readonly context: TerminalContext; readonly onClose: () => void }) {
  const [tabs, setTabs] = useState<readonly TerminalTab[]>([])
  const [active, setActive] = useState<string>()
  const [shells, setShells] = useState<readonly TerminalShell[]>([])
  const [message, setMessage] = useState<string>()
  const [opening, setOpening] = useState(false)
  const sinks = useRef(new Map<string, (event: TerminalEvent) => void>())
  const pending = useRef(new Map<string, TerminalEvent[]>())
  const closed = useRef(new Set<string>())
  const mounted = useRef(false)
  const contextNow = useRef(context)
  contextNow.current = context
  const create = async (shell?: string) => {
    const api = window.desktop?.terminal
    if (api === undefined) return
    setOpening(true)
    setMessage(undefined)
    try {
      const answer = await api.create(contextNow.current, shell)
      if (!mounted.current) { if (answer.ok) await api.close(answer.data.id); return }
      if (!answer.ok) { setMessage(answer.message); return }
      setTabs(current => [...current, answer.data])
      setActive(answer.data.id)
    } catch { setMessage('The terminal could not be opened. Your conversations are unchanged.') }
    finally { if (mounted.current) setOpening(false) }
  }
  useEffect(() => {
    mounted.current = true
    const api = window.desktop?.terminal
    const off = api?.onEvent(event => {
      if (closed.current.has(event.id)) return
      const sink = sinks.current.get(event.id)
      if (sink !== undefined) sink(event)
      else {
        const held = pending.current.get(event.id) ?? []
        // Only startup can precede the new tab's mount; discard events for a closed tab.
        if (pending.current.size < 8 && held.length < 64) { held.push(event); pending.current.set(event.id, held) }
      }
    })
    void api?.shells().then(answer => { if (mounted.current && answer.ok) setShells(answer.data) }).catch(() => undefined)
    void create()
    return () => { mounted.current = false; off?.() }
  }, [])
  return <aside className="lc-viewer lc-terminal" aria-label="Terminal panel" hidden={!visible}>
    <div className="lc-viewer__head">
      <span className="lc-viewer__name">Terminal</span>
      <span className="lc-viewer__spacer" />
      <button className="lc-viewer__action" type="button" disabled={opening || tabs.length >= 8} aria-label="New terminal" onClick={() => void create()}>+</button>
      <select className="lc-terminal__shells" aria-label="New terminal with shell" value="" disabled={opening || tabs.length >= 8} onChange={event => void create(event.target.value)}>
        <option value="" disabled>Shell…</option>{shells.map(shell => <option key={shell.id} value={shell.id}>{shell.name}</option>)}
      </select>
      <button className="lc-viewer__close" type="button" aria-label="Hide terminal" title="Hide terminal (shells keep running)" onClick={onClose}><Icon name="close" size={13} /></button>
    </div>
    <div className="lc-terminal__tabs" role="tablist" aria-label="Terminals">{tabs.map(tab => <div className="lc-terminal__tab" key={tab.id}>
      <button role="tab" type="button" aria-selected={active === tab.id} aria-controls={`terminal-${tab.id}`} id={`tab-${tab.id}`} title={`${tab.shell} · ${tab.folder}`}
        onClick={() => setActive(tab.id)}>{tab.shell} · {tab.folder.split(/[/\\]/).filter(Boolean).at(-1) ?? tab.folder}</button>
      <button type="button" aria-label={`Close ${tab.shell} terminal`} title="Close terminal and end its shell" onClick={() => {
        void window.desktop?.terminal.close(tab.id).then(() => {
          closed.current.add(tab.id)
          if (closed.current.size > 64) closed.current.delete(closed.current.values().next().value!)
          setTabs(current => { const next = current.filter(one => one.id !== tab.id); if (active === tab.id) setActive(next.at(-1)?.id); return next })
          sinks.current.delete(tab.id); pending.current.delete(tab.id)
        }).catch(() => setMessage('The shell could not be ended. The tab is still open.'))
      }}><Icon name="close" size={12} /></button>
    </div>)}</div>
    {message !== undefined && <p role="alert" className="lc-terminal__message">{message}</p>}
    {tabs.length === 0 && <p className="lc-terminal__message">{opening ? 'Opening terminal…' : 'Open a terminal with +. New tabs use the conversation’s folder.'}</p>}
    {tabs.map(tab => <TerminalScreen key={tab.id} tab={tab} visible={visible && active === tab.id} onFailure={setMessage} attach={sink => {
      sinks.current.set(tab.id, sink)
      for (const event of pending.current.get(tab.id) ?? []) sink(event)
      pending.current.delete(tab.id)
      return () => { sinks.current.delete(tab.id) }
    }} />)}
  </aside>
}

function TerminalScreen({ tab, visible, attach, onFailure }: { readonly tab: TerminalTab; readonly visible: boolean; readonly attach: (sink: (event: TerminalEvent) => void) => () => void; readonly onFailure: (message: string) => void }) {
  const element = useRef<HTMLDivElement>(null)
  const instance = useRef<{ terminal: Terminal; fit: FitAddon } | undefined>(undefined)
  const theme = useDrawnTheme()
  useEffect(() => {
    if (element.current === null) return
    const api = window.desktop?.terminal
    const terminal = new Terminal({ scrollback: 10_000, screenReaderMode: true, fontSize: 13, fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--lc-font-mono'), theme: colors(), allowProposedApi: false })
    const fit = new FitAddon()
    terminal.loadAddon(fit)
    const link = (_event: MouseEvent, url: string) => {
      if (!/^https?:\/\//i.test(url)) { onFailure('Only HTTP and HTTPS links can open from a terminal. Your shell is unchanged.'); return }
      void api?.openLink(url).then(answer => { if (!answer.ok) onFailure(answer.message) }).catch(() => onFailure('The browser could not be opened. Your shell is unchanged.'))
    }
    terminal.loadAddon(new WebLinksAddon(link))
    // OSC 8 hyperlinks use xterm's link handler, not the regex addon; guard those too.
    terminal.options.linkHandler = { activate: link }
    terminal.open(element.current)
    const data = terminal.onData(text => {
      // Paste may exceed one IPC message; retain every character in order.
      for (const chunk of terminalInputChunks(text)) void api?.input(tab.id, chunk).catch(() => undefined)
    })
    terminal.attachCustomKeyEventHandler(event => {
      const mac = /Mac/.test(navigator.platform)
      const modifier = mac ? event.metaKey : event.ctrlKey
      if (!modifier || event.altKey) return true
      if (event.key.toLowerCase() === 'c' && terminal.hasSelection()) {
        if (event.type === 'keydown') {
          event.preventDefault()
          void api?.copy(terminal.getSelection()).then(answer => { if (!answer.ok) onFailure(answer.message) }).catch(() => onFailure('That selection could not be copied. Your terminal is unchanged.'))
        }
        return false
      }
      // Let the platform's native paste event reach xterm. Clipboard reads through the web permission API are
      // refused by this window's permission guard; native Ctrl+V / Cmd+V needs no new permission.
      return true
    })
    const off = attach(event => {
      if (event.kind === 'data') terminal.write(event.data)
      else terminal.write(`\r\n[Shell exited: ${String(event.exitCode)}]\r\n`)
    })
    const resize = () => {
      if (element.current === null || element.current.clientWidth === 0 || element.current.clientHeight === 0) return
      fit.fit()
      void api?.resize(tab.id, terminal.cols, terminal.rows).catch(() => undefined)
    }
    const observer = new ResizeObserver(resize)
    observer.observe(element.current)
    instance.current = { terminal, fit }
    resize()
    return () => { observer.disconnect(); data.dispose(); off(); terminal.dispose(); instance.current = undefined }
  }, [tab.id])
  useEffect(() => { if (instance.current !== undefined) instance.current.terminal.options.theme = colors() }, [theme])
  useEffect(() => {
    if (!visible) return
    const frame = requestAnimationFrame(() => { instance.current?.fit.fit(); instance.current?.terminal.focus() })
    return () => cancelAnimationFrame(frame)
  }, [visible])
  return <div className="lc-terminal__screen" role="tabpanel" id={`terminal-${tab.id}`} aria-labelledby={`tab-${tab.id}`} hidden={!visible} ref={element} />
}
