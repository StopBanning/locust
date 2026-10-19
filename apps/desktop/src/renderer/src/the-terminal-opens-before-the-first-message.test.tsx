import { renderToStaticMarkup } from 'react-dom/server'
import { Buffer } from 'node:buffer'
import { expect, it } from 'vitest'
import { TerminalToggle } from './components/TerminalToggle.js'
import { TitleBar } from './components/TitleBar.js'
import { isTerminalToggle } from './terminalKeys.js'
import { terminalInputChunks } from '../../shared/terminal.js'

it('offers an accessible toggle in the title bar before a conversation exists', () => {
  const html = renderToStaticMarkup(<TitleBar workspaceName="Project" runningCount={0} swarm={false} terminal={<TerminalToggle open={false} onToggle={() => undefined} />} />)
  expect(html).toContain('aria-label="Terminal"')
  expect(html).toContain('aria-pressed="false"')
  expect(html).toContain('Control+`')
})
it('shows the organization restriction on the disabled terminal control', () => {
  const html = renderToStaticMarkup(<TerminalToggle open={false} refusal="Set by your organization" onToggle={() => undefined} />)
  expect(html).toContain('disabled=""')
  expect(html).toContain('Set by your organization')
})
it('recognizes Ctrl backtick on every platform and leaves unrelated keys alone', () => {
  const key = { ctrlKey: true, altKey: false, metaKey: false, key: '`', code: 'Backquote' }
  expect(isTerminalToggle(key)).toBe(true)
  expect(isTerminalToggle({ ...key, key: '~' })).toBe(true)
  expect(isTerminalToggle({ ...key, ctrlKey: false })).toBe(false)
  expect(isTerminalToggle({ ...key, altKey: true })).toBe(false)
  expect(isTerminalToggle({ ...key, code: 'KeyA', key: 'a' })).toBe(false)
})
it('keeps a pasted Unicode character whole at every IPC boundary', () => {
  const text = 'x'.repeat(65535) + String.fromCodePoint(0x1f600) + 'tail'
  const chunks = [...terminalInputChunks(text)]
  expect(chunks.every(chunk => chunk.length <= 65536)).toBe(true)
  // Separate writes are encoded separately by the PTY, unlike merely joining UTF-16 strings again.
  expect(Buffer.concat(chunks.map(chunk => Buffer.from(chunk))).toString('utf8')).toBe(text)
})
