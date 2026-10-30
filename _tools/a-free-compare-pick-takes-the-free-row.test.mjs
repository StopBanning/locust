import assert from 'node:assert/strict'
import { afterEach, test } from 'vitest'
import { comparePick, comparePickScript } from './drive-lib.mjs'

// The first Mac run (2026-10-30): Compare's free pick "deepseek-v4.1-flash-free" ticked Cursor's paid "DeepSeek V4.1
// Flash 1017", listed above OpenCode's free row, because the pick matched the model's words without "free".
const row = (label) => ({
  disabled: false, pressed: false,
  querySelector: () => ({ textContent: label }),
  getAttribute() { return String(this.pressed) },
  click() { this.pressed = true }
})
const picker = (labels) => {
  const rows = labels.map(row)
  globalThis.document = {
    querySelector: (selector) => (selector === '.lc-picker__input' ? Object.create(HTMLInputLike) : null),
    querySelectorAll: () => rows
  }
  globalThis.Event = class { constructor(type) { this.type = type } }
  return rows
}
const HTMLInputLike = { dispatchEvent() {} }
globalThis.HTMLInputElement = { prototype: { set value(v) {} } }
afterEach(() => { delete globalThis.document })

test('a free pick says it wants the free row', () => {
  assert.equal(comparePick('opencode/deepseek-v4.1-flash-free').free, true)
  assert.equal(comparePick('gpt-5.5').free, false)
})

test('a free pick ticks the row that says Free, though a paid row of the same model comes first', async () => {
  const rows = picker(['DeepSeek V4.1 Flash 1017', 'Deepseek V4.1 Flash Free'])
  const got = JSON.parse(await eval(comparePickScript(0, comparePick('deepseek-v4.1-flash-free'))))
  assert.deepEqual(got, { picked: true, label: 'Deepseek V4.1 Flash Free' })
  assert.deepEqual(rows.map((one) => one.pressed), [false, true])
})

test('with no row saying Free, the pick takes the first that matches, as before', async () => {
  const rows = picker(['Mimo V2.6 Flash', 'Mimo V2.6 Flash Pro'])
  const got = JSON.parse(await eval(comparePickScript(0, comparePick('mimo-v2.6-flash-free'))))
  assert.equal(got.label, 'Mimo V2.6 Flash')
  assert.deepEqual(rows.map((one) => one.pressed), [true, false])
})
