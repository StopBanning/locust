import { useCallback, useEffect, useRef, useState } from 'react'
import type { ReactElement, RefObject } from 'react'

import {
  PROMPT_RAIL_KEYS,
  currentPromptMark,
  promptMarkPlace,
  promptRailLine,
  promptRailMotion,
  promptRailRoom,
  promptRailScrollable,
  promptRailShows,
  promptRailStep
} from '../promptRail.js'
import { windowKeys } from '../keyboardShortcuts.js'

interface RailMark {
  readonly line: string
  readonly place: number
}

const sameMarks = (held: readonly RailMark[], next: readonly RailMark[]): boolean =>
  held.length === next.length && held.every((mark, index) => mark.line === next[index]?.line && mark.place === next[index]?.place)

function paintHit(bubble: HTMLElement, highlight: boolean, timer: { current: number }): void {
  window.clearTimeout(timer.current)
  document.querySelector('.lc-bubble.is-railhit')?.classList.remove('is-railhit')
  CSS.highlights?.delete('lc-promptrail')
  if (!highlight) return
  bubble.classList.add('is-railhit')
  if (typeof Highlight !== 'undefined' && CSS.highlights !== undefined) {
    const range = document.createRange()
    range.selectNodeContents(bubble)
    CSS.highlights.set('lc-promptrail', new Highlight(range))
  }
  timer.current = window.setTimeout(() => {
    bubble.classList.remove('is-railhit')
    CSS.highlights?.delete('lc-promptrail')
  }, 2400)
}

function menuIsOpen(): boolean {
  return [...document.querySelectorAll<HTMLElement>('[role="menu"], [role="listbox"]')].some((menu) => {
    const box = menu.getBoundingClientRect()
    return box.height > 0 && box.width > 0 && getComputedStyle(menu).visibility !== 'hidden'
  })
}

/**
 * A slim strip of your own messages, stuck to the thread's right edge.
 * Measuring happens when the thread changes size. Scrolling only picks which
 * cached position is nearest the top.
 */
export function PromptRail({
  scroller,
  column
}: {
  readonly scroller: RefObject<HTMLDivElement | null>
  readonly column: RefObject<HTMLDivElement | null>
}): ReactElement | null {
  const [marks, setMarks] = useState<readonly RailMark[]>([])
  const [current, setCurrent] = useState(0)
  const [open, setOpen] = useState<number | undefined>(undefined)
  const topsRef = useRef<readonly number[]>([])
  const currentRef = useRef(0)
  const hitTimer = useRef(0)

  const jump = useCallback((index: number): void => {
    const box = scroller.current
    const root = column.current
    if (box === null || root === null) return
    const bubble = [...root.querySelectorAll<HTMLElement>('[data-prompt-mark]')][index]
    if (bubble === undefined) return
    const motion = promptRailMotion(window.matchMedia('(prefers-reduced-motion: reduce)').matches)
    const top = box.scrollTop + bubble.getBoundingClientRect().top - box.getBoundingClientRect().top
    setOpen(undefined)
    box.scrollTo({ top: Math.max(0, top), behavior: motion.scroll })
    paintHit(bubble, motion.highlight, hitTimer)
    currentRef.current = index
    setCurrent(index)
  }, [column, scroller])

  useEffect(() => {
    let frame = 0
    let tries = 0
    let stopped = false
    let watching: ResizeObserver | undefined
    let mutations: MutationObserver | undefined
    const measure = (box: HTMLDivElement, root: HTMLElement): void => {
      const scrollHeight = box.scrollHeight
      const clientHeight = box.clientHeight
      const scrollTop = box.scrollTop
      if (clientHeight < 1) return
      const scrollbar = box.offsetWidth - box.clientWidth
      const paddingRight = Number.parseFloat(getComputedStyle(box).paddingRight) || 0
      const threadRight = box.getBoundingClientRect().right
      const columnRight = root.getBoundingClientRect().right
      const origin = box.getBoundingClientRect().top
      const bubbles = [...root.querySelectorAll<HTMLElement>('[data-prompt-mark]')]
      const tops = bubbles.map((bubble) => Math.round(bubble.getBoundingClientRect().top - origin + scrollTop))
      const room = promptRailRoom({ threadRight, scrollbar, paddingRight, columnRight })
      const show = promptRailShows({
        messages: bubbles.length,
        scrollable: promptRailScrollable(scrollHeight, clientHeight),
        compare: root.closest('.lc-compare') !== null,
        room
      })
      const view = `${String(clientHeight)}px`
      if (box.style.getPropertyValue('--lc-rail-view') !== view) box.style.setProperty('--lc-rail-view', view)
      topsRef.current = show ? tops : []
      const nextCurrent = tops.length === 0 || !show ? 0 : currentPromptMark(tops, scrollTop)
      currentRef.current = nextCurrent
      const next = show
        ? bubbles.map((bubble, index) => ({
          line: bubble.getAttribute('data-prompt-mark') ?? promptRailLine(bubble.innerText),
          place: Math.round(promptMarkPlace(tops[index] ?? 0, scrollHeight) * 10000) / 10000
        }))
        : []
      setMarks((held) => sameMarks(held, next) ? held : next)
      setCurrent((held) => held === nextCurrent ? held : nextCurrent)
      setOpen((held) => held !== undefined && held < next.length ? held : undefined)
    }
    const connect = (): void => {
      if (stopped) return
      const box = scroller.current
      const found = box?.querySelector(':scope > .lc-thread__column')
      const root = found instanceof HTMLElement ? found : column.current
      // The thread's own ref is not set yet on the first layout pass. A later
      // frame sees it; giving up after a second means the thread never arrived.
      if (box === null || root === null) {
        tries += 1
        if (tries < 60) frame = requestAnimationFrame(connect)
        return
      }
      const run = (): void => measure(box, root)
      run()
      watching = new ResizeObserver(() => {
        cancelAnimationFrame(frame)
        frame = requestAnimationFrame(run)
      })
      watching.observe(box)
      watching.observe(root)
      // The browser or terminal beside the conversation shrinks an ancestor.
      // Watching only the thread missed that, and the strip stayed on the words.
      for (let node: HTMLElement | null = box.parentElement; node !== null && node !== document.body; node = node.parentElement) {
        watching.observe(node)
      }
      // A new message changes the column's children. Size often changes with
      // them; this catches the one that does not. The shell's class is how a
      // panel beside the conversation is announced.
      mutations = new MutationObserver(() => {
        cancelAnimationFrame(frame)
        frame = requestAnimationFrame(run)
      })
      mutations.observe(root, { childList: true, subtree: true })
      const shell = box.closest('.lc-shell')
      if (shell !== null) mutations.observe(shell, { attributes: true, attributeFilter: ['class'] })
    }
    connect()
    return () => {
      stopped = true
      cancelAnimationFrame(frame)
      watching?.disconnect()
      mutations?.disconnect()
      window.clearTimeout(hitTimer.current)
      CSS.highlights?.delete('lc-promptrail')
    }
  }, [column, scroller])

  useEffect(() => {
    const box = scroller.current
    if (box === null || marks.length === 0) return undefined
    const onScroll = (): void => {
      const next = currentPromptMark(topsRef.current, box.scrollTop)
      if (next === currentRef.current) return
      currentRef.current = next
      setCurrent(next)
    }
    box.addEventListener('scroll', onScroll, { passive: true })
    return () => box.removeEventListener('scroll', onScroll)
  }, [marks.length, scroller])

  useEffect(() => {
    if (marks.length === 0) return undefined
    const onKey = (event: KeyboardEvent): void => {
      const step = promptRailStep(event)
      if (step === undefined || menuIsOpen()) return
      const count = topsRef.current.length
      if (count === 0) return
      event.preventDefault()
      event.stopPropagation()
      const next = Math.min(count - 1, Math.max(0, currentRef.current + step))
      jump(next)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [jump, marks.length])

  if (marks.length === 0) return null
  return (
    <nav className="lc-promptrail" aria-label="Your messages" aria-keyshortcuts="Control+Alt+ArrowUp Control+Alt+ArrowDown">
      <div className="lc-promptrail__track">
        {marks.map((mark, index) => (
          <button
            key={`${mark.line}:${String(index)}`}
            type="button"
            className={`lc-promptrail__mark${index === current ? ' is-current' : ''}`}
            style={{ top: `${String(mark.place * 100)}%` }}
            aria-label={mark.line}
            {...(index === current ? { 'aria-current': 'true' as const } : {})}
            onPointerEnter={() => setOpen(index)}
            onPointerLeave={() => setOpen((held) => held === index ? undefined : held)}
            onFocus={() => setOpen(index)}
            onBlur={() => setOpen((held) => held === index ? undefined : held)}
            onClick={() => jump(index)}
          >
            {open === index && (
              <span className="lc-promptrail__tip" role="tooltip">
                <span className="lc-promptrail__line">{mark.line}</span>
                <span className="lc-promptrail__keys">{windowKeys(PROMPT_RAIL_KEYS, true)}</span>
              </span>
            )}
          </button>
        ))}
      </div>
    </nav>
  )
}
