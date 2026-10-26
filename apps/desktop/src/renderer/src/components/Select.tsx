import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from 'react'

import { useDismissOnOutsidePress } from '../useDismissOnOutsidePress.js'
import { ChevronGlyph } from './ChatGlyphs.js'
import { Icon } from './Icon.js'
import { menuRowFromDigit } from '../menuDigit.js'

/**
 * A choice control drawn as Locust's own menu, in place of the operating
 * system's list.
 *
 * The button keeps focus. Arrows, Home, End and typed letters move a
 * highlight; Enter or Space is what chooses. Escape closes it and nothing
 * else: it is caught on the way down, before a dialog's own Escape, so
 * closing the list does not close the dialog under it.
 */
export interface SelectOption {
  readonly value: string
  readonly label: string
  /** A second line, as a composer menu item has. */
  readonly description?: string
  readonly disabled?: boolean
  /** A heading shared by the run of options that name the same one. */
  readonly group?: string
}

export interface SelectKeyInput {
  readonly open: boolean
  /** The highlighted option, or -1 when nothing is. */
  readonly active: number
  /** The committed option, or -1 when the value matches none. */
  readonly selected: number
  readonly enabled: readonly number[]
  readonly labels: readonly string[]
  readonly key: string
  readonly altKey: boolean
  readonly ctrlKey: boolean
  readonly metaKey: boolean
  /** Letters typed so far, before this key. */
  readonly query: string
  /** This key arrived while the previous letters still count. */
  readonly queryLive: boolean
}

export interface SelectKeyResult {
  readonly open: boolean
  readonly active: number
  readonly choose: boolean
  readonly prevent: boolean
  readonly query: string
}

const TYPEAHEAD_MS = 500
/** useLayoutEffect warns when a test renders markup and there is no document. */
const usePlaceEffect = typeof document === 'undefined' ? useEffect : useLayoutEffect

/** The next enabled option in `delta`'s direction, staying put at either end. */
export function stepSelect(enabled: readonly number[], from: number, delta: number): number {
  if (enabled.length === 0) return -1
  const at = enabled.indexOf(from)
  if (at < 0) return delta < 0 ? enabled[enabled.length - 1]! : enabled[0]!
  const next = at + delta
  if (next < 0 || next >= enabled.length) return enabled[at]!
  return enabled[next]!
}

/**
 * The option a typed prefix jumps to.
 *
 * One letter starts after the highlight and wraps, so the same letter walks
 * through every match. A longer prefix stays on the highlight when it still
 * matches, and otherwise takes the first match in the list.
 */
export function typeaheadIndex(labels: readonly string[], enabled: readonly number[], from: number, query: string): number {
  const wanted = query.toLowerCase()
  const matches = enabled.filter((index) => labels[index]?.toLowerCase().startsWith(wanted) === true)
  if (matches.length === 0) return from
  if (wanted.length <= 1) return matches.find((index) => index > from) ?? matches[0]!
  return matches.includes(from) ? from : matches[0]!
}

/** What one key does to an open or closed select. Choosing is separate from moving. */
export function applySelectKey(input: SelectKeyInput): SelectKeyResult {
  const held = input.queryLive ? input.query : ''
  const stay: SelectKeyResult = { open: input.open, active: input.active, choose: false, prevent: false, query: held }
  if (input.enabled.length === 0 || input.ctrlKey || input.metaKey) return stay
  const selected = input.selected >= 0 && input.enabled.includes(input.selected) ? input.selected : input.enabled[0]!
  if (input.open) {
    const nth = menuRowFromDigit(input.key, { ctrl: input.ctrlKey, meta: input.metaKey, alt: input.altKey, shift: false }, false)
    if (nth !== undefined) {
      const chosen = input.enabled[nth]
      if (chosen === undefined) return { ...stay, prevent: true }
      return { open: false, active: chosen, choose: true, prevent: true, query: '' }
    }
  }

  if (input.key.length === 1 && input.key !== ' ' && !input.altKey) {
    const letter = input.key.toLowerCase()
    const query = held + letter
    const from = input.open ? input.active : input.selected
    const prefixed = typeaheadIndex(input.labels, input.enabled, from, query)
    const repeated = [...query].every((character) => character === letter)
    const active = prefixed === from && query.length > 1 && repeated
      ? typeaheadIndex(input.labels, input.enabled, from, letter)
      : prefixed
    const landed = active < 0 ? undefined : input.labels[active]?.toLowerCase()
    if (landed === undefined || (!landed.startsWith(query) && !(repeated && landed.startsWith(letter)))) {
      return { ...stay, prevent: true }
    }
    return { open: true, active, choose: false, prevent: true, query: landed.startsWith(query) ? query : letter }
  }

  if (input.key === 'ArrowDown' || input.key === 'ArrowUp') {
    if (input.altKey && input.key === 'ArrowUp') {
      return input.open ? { open: false, active: selected, choose: false, prevent: true, query: '' } : stay
    }
    const from = input.open ? input.active : input.selected
    return { open: true, active: stepSelect(input.enabled, from, input.key === 'ArrowDown' ? 1 : -1), choose: false, prevent: true, query: '' }
  }

  if (input.key === 'Home' || input.key === 'End') {
    const active = input.key === 'Home' ? input.enabled[0]! : input.enabled[input.enabled.length - 1]!
    return { open: true, active, choose: false, prevent: true, query: '' }
  }

  if (input.key === 'Enter' || input.key === ' ') {
    if (!input.open) return { open: true, active: selected, choose: false, prevent: true, query: '' }
    const active = input.enabled.includes(input.active) ? input.active : selected
    return { open: false, active, choose: true, prevent: true, query: '' }
  }

  if (input.key === 'Escape') {
    return input.open ? { open: false, active: selected, choose: false, prevent: true, query: '' } : stay
  }

  if (input.key === 'Tab' && input.open) {
    return { open: false, active: selected, choose: false, prevent: false, query: '' }
  }

  return stay
}

interface SelectBlock {
  readonly group?: string
  readonly options: readonly { readonly option: SelectOption; readonly index: number }[]
}

function blocksOf(options: readonly SelectOption[]): readonly SelectBlock[] {
  const blocks: { group?: string; options: { option: SelectOption; index: number }[] }[] = []
  options.forEach((option, index) => {
    const last = blocks[blocks.length - 1]
    if (last !== undefined && last.group === option.group) last.options.push({ option, index })
    else blocks.push({ ...(option.group === undefined ? {} : { group: option.group }), options: [{ option, index }] })
  })
  return blocks
}

export function Select({
  id,
  className,
  value,
  options,
  disabled = false,
  onChange,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy
}: {
  readonly id?: string
  readonly className?: string
  readonly value: string | number
  readonly options: readonly SelectOption[]
  readonly disabled?: boolean
  readonly onChange: (value: string) => void
  readonly 'aria-label'?: string
  readonly 'aria-labelledby'?: string
}): ReactElement {
  const listId = useId()
  const ownId = useId()
  const buttonId = id ?? ownId
  const buttonRef = useRef<HTMLButtonElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const typed = useRef({ query: '', at: 0 })
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  const [place, setPlace] = useState<{ readonly top: number; readonly left: number; readonly minWidth: number }>()
  const current = String(value)
  const selected = options.findIndex((option) => option.value === current)
  const enabled = options.flatMap((option, index) => option.disabled === true ? [] : [index])
  const labels = options.map((option) => option.label)
  const shown = options[selected]?.label ?? ''
  const highlighted = open ? (enabled.includes(active) ? active : (enabled.includes(selected) ? selected : enabled[0] ?? -1)) : selected

  useDismissOnOutsidePress(open, () => setOpen(false), buttonRef, listRef)

  useEffect(() => {
    if (disabled) setOpen(false)
  }, [disabled])

  /*
   * Escape is taken on the way down. A dialog listens for it on the document
   * during capture and closes itself; a listener on the button would be too
   * late, and closing the list would close the dialog with it.
   */
  useEffect(() => {
    if (!open) return undefined
    const onKey = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [open])

  usePlaceEffect(() => {
    if (!open) {
      setPlace(undefined)
      return
    }
    const button = buttonRef.current
    const list = listRef.current
    if (button === null || list === null) return
    const box = button.getBoundingClientRect()
    const minWidth = Math.max(Math.ceil(box.width), 192)
    list.style.minWidth = `${String(minWidth)}px`
    const menu = list.getBoundingClientRect()
    const margin = 8
    const gap = 4
    const roomBelow = window.innerHeight - box.bottom - margin
    const roomAbove = box.top - margin
    const openUp = menu.height > roomBelow && roomAbove > roomBelow
    const top = openUp
      ? Math.max(margin, box.top - gap - menu.height)
      : Math.min(box.bottom + gap, Math.max(margin, window.innerHeight - margin - menu.height))
    const width = Math.max(box.width, menu.width)
    const left = Math.max(margin, Math.min(box.left, window.innerWidth - margin - width))
    setPlace({ top, left, minWidth })
    list.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' })
  }, [open, highlighted, options])

  const commit = (index: number): void => {
    const option = options[index]
    if (option === undefined || option.disabled === true) return
    if (option.value !== current) onChange(option.value)
    setOpen(false)
    buttonRef.current?.focus()
  }

  const onKeyDown = (event: ReactKeyboardEvent): void => {
    if (disabled) return
    const now = Date.now()
    const result = applySelectKey({
      open,
      active: highlighted,
      selected,
      enabled,
      labels,
      key: event.key,
      altKey: event.altKey,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      query: typed.current.query,
      queryLive: now - typed.current.at < TYPEAHEAD_MS
    })
    if (!result.prevent && result.open === open) return
    typed.current = { query: result.query, at: now }
    if (result.prevent) event.preventDefault()
    setActive(result.active)
    setOpen(result.choose ? false : result.open)
    if (result.choose) commit(result.active)
  }

  const optionOf = (option: SelectOption, index: number): ReactElement => {
    const chosen = option.value === current
    return (
      <button
        key={`${option.value}:${String(index)}`}
        type="button"
        role="option"
        id={`${listId}-opt-${String(index)}`}
        className={`lc-menu__item${index === highlighted ? ' is-active' : ''}`}
        tabIndex={-1}
        aria-selected={chosen}
        data-value={option.value}
        data-active={index === highlighted ? 'true' : undefined}
        disabled={option.disabled === true}
        onMouseDown={(event) => event.preventDefault()}
        onMouseEnter={() => { if (option.disabled !== true) setActive(index) }}
        onClick={() => commit(index)}
      >
        <span className="lc-menu__text">
          <span className="lc-menu__name">{option.label}</span>
          {option.description !== undefined && <span className="lc-menu__desc">{option.description}</span>}
        </span>
        {chosen && <Icon name="check" size={13} />}
      </button>
    )
  }

  const list = (
    <div
      ref={listRef}
      id={listId}
      role="listbox"
      hidden={!open}
      className="lc-menu lc-select__menu"
      aria-labelledby={ariaLabelledBy ?? buttonId}
      style={place === undefined ? { visibility: 'hidden' } : { top: place.top, left: place.left, minWidth: place.minWidth }}
      onKeyDown={onKeyDown}
    >
      {blocksOf(options).map((block, blockIndex) => {
        const rows = block.options.map(({ option, index }) => optionOf(option, index))
        if (block.group === undefined) return rows
        return (
          <div role="group" aria-label={block.group} key={`${block.group}:${String(blockIndex)}`}>
            <div className="lc-select__group" aria-hidden="true">{block.group}</div>
            {rows}
          </div>
        )
      })}
    </div>
  )

  return (
    <span className="lc-select">
      <button
        ref={buttonRef}
        type="button"
        id={buttonId}
        className={`lc-select__button${className === undefined ? '' : ` ${className}`}`}
        role="combobox"
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open && highlighted >= 0 ? `${listId}-opt-${String(highlighted)}` : undefined}
        disabled={disabled}
        onClick={() => {
          if (disabled) return
          setOpen((held) => {
            if (!held) setActive(enabled.includes(selected) ? selected : enabled[0] ?? -1)
            return !held
          })
        }}
        onKeyDown={onKeyDown}
      >
        <span className="lc-select__value">{shown}</span>
        <ChevronGlyph />
      </button>
      {open && typeof document !== 'undefined' ? createPortal(list, document.body) : list}
    </span>
  )
}
