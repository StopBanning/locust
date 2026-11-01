import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent, ReactElement } from 'react'

import { Icon } from './Icon.js'
import { isChord, Keycaps } from './Keycaps.js'

export interface PaletteAction {
  readonly id: string
  readonly group: string
  readonly label: string
  readonly hint?: string
  /** More words it is found by than its label -- a conversation's every turn, its teammate. */
  readonly keywords?: string
  /** Listed only once something is typed: every conversation is too many to scroll past. */
  readonly whenTyped?: boolean
  readonly run: () => void
}

/**
 * The command palette.
 *
 * Every entry does something. The reference lists actions this build does not
 * have -- switch runtime, toggle swarm -- and a palette that offers a command
 * and then ignores it is worse than a shorter palette, so the caller supplies
 * only actions it can actually run.
 */
export function CommandPalette({
  actions,
  onClose
}: {
  readonly actions: readonly PaletteAction[]
  readonly onClose: () => void
}): ReactElement {
  const [query, setQuery] = useState('')
  const [index, setIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (needle.length === 0) return actions.filter((action) => action.whenTyped !== true)
    return actions.filter((action) => `${action.group} ${action.label} ${action.keywords ?? ''}`.toLowerCase().includes(needle))
  }, [actions, query])

  /*
   * GROUPED BY SORTING, not by suppressing a repeat.
   *
   * The header was emitted whenever the group changed, which is right for a
   * sorted list and wrong for an unsorted one: `Workroom` sat in `Go to`,
   * the swarm action sat between it and the rest of `Go to`, and the group
   * broke and re-opened -- so the palette printed `GO TO` twice (frame pass,
   * 2026-09-15).
   *
   * Suppressing the repeated header was the other option and it is worse: it
   * leaves `Workroom` orphaned above an unrelated row, and the reason it is
   * first is that it is the most likely action. That would read as a mistake
   * rather than as a promotion.
   *
   * Group order is the order each group FIRST appears in the actions, and
   * order within a group is untouched, so the caller still decides both and
   * a duplicate header becomes structurally impossible.
   *
   * THIS is the list, not `matches`: the arrow keys, the highlight and Enter
   * all index it, and a palette whose selection and rendering disagree would
   * run the row above or below the one lit up.
   */
  const grouped = useMemo(() => {
    const order = new Map<string, number>()
    for (const action of matches) {
      if (!order.has(action.group)) order.set(action.group, order.size)
    }
    return [...matches].sort((left, right) => (order.get(left.group) ?? 0) - (order.get(right.group) ?? 0))
  }, [matches])

  useEffect(() => {
    setIndex(0)
  }, [query])

  /*
   * Escape closes it from anywhere, not only from inside it.
   *
   * The handler below is on the palette's own element, so it only hears a
   * key when focus is inside. Grok, pass 11: palette open, Escape, still
   * open. The plus menu already listens at the document, and a person who
   * has clicked elsewhere and presses the key the rest of the app trained
   * expects the same here.
   */
  useEffect(() => {
    const onDocumentKey = (event: globalThis.KeyboardEvent): void => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onDocumentKey)
    return () => document.removeEventListener('keydown', onDocumentKey)
  }, [onClose])

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
      return
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setIndex((current) => (grouped.length === 0 ? 0 : (current + 1) % grouped.length))
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      setIndex((current) => (grouped.length === 0 ? 0 : (current - 1 + grouped.length) % grouped.length))
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      const action = grouped[index]
      if (action !== undefined) {
        onClose()
        action.run()
      }
    }
  }

  let lastGroup: string | undefined

  return (
    <div className="lc-scrim lc-scrim--top" onKeyDown={onKeyDown}>
      <div className="lc-palette" role="dialog" aria-modal="true" aria-label="Command palette">
        <div className="lc-palette__head">
          <Icon name="search" size={14} />
          <input
            ref={inputRef}
            className="lc-palette__input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search conversations, or type a command…"
            aria-label="Command palette search"
            autoComplete="off"
          />
          <button type="button" className="lc-dialog__close" aria-label="Close palette" onClick={onClose}>
            <Icon name="close" size={13} />
          </button>
        </div>
        <div className="lc-palette__list">
          {grouped.length === 0 ? (
            <p className="lc-inspector__empty" style={{ padding: '10px' }}>
              Nothing matches that.
            </p>
          ) : (
            grouped.map((action, position) => {
              const header = action.group === lastGroup ? undefined : action.group
              lastGroup = action.group
              return (
                <div key={action.id}>
                  {header !== undefined && <div className="lc-palette__group">{header}</div>}
                  <button
                    type="button"
                    className={`lc-palette__item${position === index ? ' is-active' : ''}`}
                    onMouseEnter={() => setIndex(position)}
                    onClick={() => {
                      onClose()
                      action.run()
                    }}
                  >
                    <span className="lc-palette__label">{action.label}</span>
                    {action.hint !== undefined && (isChord(action.hint)
                      ? <Keycaps keys={action.hint} mac={typeof window !== 'undefined' && window.desktop?.platform === 'darwin'} />
                      : <span className="lc-palette__hint">{action.hint}</span>)}
                  </button>
                </div>
              )
            })
          )}
        </div>
      </div>
    </div>
  )
}
