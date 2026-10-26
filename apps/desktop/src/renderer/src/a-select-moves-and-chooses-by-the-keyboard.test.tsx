import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { Select, applySelectKey, type SelectKeyInput } from './components/Select.js'

/**
 * A SELECT MOVES, THEN CHOOSES.
 *
 * Up and Down, Home and End, and a typed letter move the highlight. Enter
 * and Space are what set the value. Escape closes and leaves the value.
 */
const labels = ['Ask', 'Brief', 'Detailed', 'Folder']
const enabled = [0, 1, 2, 3]

function press(key: string, over: Partial<SelectKeyInput> = {}) {
  return applySelectKey({
    open: false,
    active: -1,
    selected: 1,
    enabled,
    labels,
    key,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    query: '',
    queryLive: false,
    ...over
  })
}

describe('a select moves and chooses by the keyboard', () => {
  it('opens on the chosen row, and arrows move without choosing', () => {
    const opened = press('Enter')
    expect(opened).toMatchObject({ open: true, active: 1, choose: false, prevent: true })
    const down = press('ArrowDown', { open: true, active: 1 })
    expect(down).toMatchObject({ open: true, active: 2, choose: false })
    expect(press('ArrowDown', { open: true, active: 3 }).active).toBe(3)
    expect(press('ArrowUp', { open: true, active: 0 }).active).toBe(0)
    expect(press('Home', { open: true, active: 2 })).toMatchObject({ active: 0, choose: false })
    expect(press('1', { open: true, active: 3 })).toMatchObject({ open: false, active: 0, choose: true })
    expect(press('4', { open: true, active: 0 })).toMatchObject({ active: 3, choose: true })
    expect(press('9', { open: true, active: 0 }).choose).toBe(false)
    expect(press('End', { open: true, active: 0 })).toMatchObject({ active: 3, choose: false })
  })

  it('chooses the highlight with Enter or Space, and Escape leaves the value', () => {
    expect(press('Enter', { open: true, active: 2 })).toMatchObject({ open: false, active: 2, choose: true })
    expect(press(' ', { open: true, active: 0 })).toMatchObject({ choose: true, active: 0 })
    expect(press('Escape', { open: true, active: 3 })).toMatchObject({ open: false, choose: false, active: 1 })
    expect(press('Tab', { open: true, active: 3 })).toMatchObject({ open: false, choose: false, prevent: false })
    expect(press('ArrowDown')).toMatchObject({ open: true, active: 2, choose: false })
  })

  it('skips a disabled row, and a letter jumps, then the same letter walks', () => {
    const skip = [0, 2, 3]
    expect(press('ArrowDown', { open: true, active: 0, enabled: skip }).active).toBe(2)
    expect(press('d', { open: true, active: 0 })).toMatchObject({ open: true, active: 2, choose: false, query: 'd' })
    expect(press('d', { open: true, active: 2, query: 'd', queryLive: true }).active).toBe(2)
    expect(press('e', { open: true, active: 2, query: 'd', queryLive: true })).toMatchObject({ active: 2, query: 'de' })
    expect(press('z', { open: true, active: 1 })).toMatchObject({ open: true, active: 1, query: '' })
    expect(press('b', { ctrlKey: true })).toMatchObject({ open: false, prevent: false })
  })

  it('is a combobox whose list names the chosen row, a description, and a disabled row', () => {
    const html = renderToStaticMarkup(
      <Select
        aria-label="Kind"
        value="brief"
        onChange={() => undefined}
        options={[
          { value: 'ask', label: 'Ask', description: 'Read only' },
          { value: 'brief', label: 'Brief' },
          { value: 'folder', label: 'Folder', disabled: true }
        ]}
      />
    )
    expect(html).toMatch(/role="combobox"[^>]*aria-haspopup="listbox"/)
    expect(html).toContain('aria-expanded="false"')
    expect(html).toContain('role="listbox"')
    expect(html).toContain('aria-label="Kind"')
    expect(html).toContain('lc-select__value">Brief<')
    expect(html).toContain('lc-menu__desc">Read only<')
    expect(html).toMatch(/aria-selected="true"[^>]*data-value="brief"/)
    expect(html).toMatch(/data-value="folder"[^>]*disabled=""|disabled=""[^>]*data-value="folder"/)
    expect(html).toContain('class="lc-chevron"')
    expect(html).not.toContain('<select')
  })

  it('draws a disabled control as a disabled combobox', () => {
    const html = renderToStaticMarkup(
      <Select aria-label="Hours between runs" value="4" disabled onChange={() => undefined} options={[{ value: '4', label: '4 hours' }]} />
    )
    expect(html).toMatch(/role="combobox"[^>]*disabled=""|disabled=""[^>]*role="combobox"/)
    expect(html).toContain('lc-select__value">4 hours<')
  })
})
