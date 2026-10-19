// Targeted mutation controls: every sabotage must fail its intended assertion, with the same tests collected.
import { readFileSync, writeFileSync, rmSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'
import assert from 'node:assert/strict'
const root = fileURLToPath(new URL('../apps/desktop/', import.meta.url))
const tests = ['src/main/a-terminal-belongs-to-the-person-and-ends-with-the-app.test.ts', 'src/main/a-terminal-refuses-paths-and-requests-from-other-windows.test.ts', 'src/renderer/src/the-terminal-opens-before-the-first-message.test.tsx']
const report = join(root, 'terminal-control-result.json')
const mutations = [
  ['src/shared/terminal.ts', 'end -= 1', 'end -= 0', 'keeps a pasted Unicode character whole at every IPC boundary'],
  ['src/main/terminal-ipc.ts', 'clipboard.writeText(text)', 'void text', 'copies selected terminal text through the host without granting clipboard reads'],
  ['src/main/terminal-host.ts', 'for (const pid of descendantsOf(snapshot, pty.pid))', 'for (const pid of [])', 'closes Unix foreground and background descendants before ending their shell'],
  ['src/main/terminal-host.ts', 'if (refused !== undefined) throw new Error(refused)', 'if (false) throw new Error(refused)', 'an organization terminal restriction refuses even a direct host request'],
  ['src/main/terminal-host.ts', 'if (!wasEnded) await (options.end ?? endTerminalTree)(session.pty)', 'void wasEnded', 'closing a tab ends only that shell and quitting ends every remaining shell'],
  ['src/main/terminal-host.ts', 'if (disposed) { await (options.end ?? endTerminalTree)(pty);', 'if (false) { await (options.end ?? endTerminalTree)(pty);', 'a quit during an unfinished spawn ends the late shell too'],
  ['src/main/terminal-host.ts', 'session.timer = setTimeout(() => flush(session), 16)', 'flush(session)', 'keeps output in frame-sized batches and flushes the last text before exit'],
  ['src/main/terminal-folder.ts', 'return trees.find(tree => tree.teammateId === teammate.teammateId)?.path', 'return root', 'uses the current project before a first message and a conversation’s own worktree afterwards'],
  ['src/main/terminal-ipc.ts', "if (!own(event) || typeof context !== 'object'", "if (typeof context !== 'object'", 'refuses renderer-invented paths and executable names, and every request from another window'],
  ['src/main/terminal-ipc.ts', "!['https:', 'http:'].includes(new URL(url).protocol)", 'false', 'opens only HTTP and HTTPS terminal links in the person’s browser'],
  ['src/renderer/src/terminalKeys.ts', 'event.ctrlKey && !event.altKey && !event.metaKey', 'true', 'recognizes Ctrl backtick on every platform and leaves unrelated keys alone'],
  ['src/renderer/src/components/TitleBar.tsx', '{terminal}', '{null}', 'offers an accessible toggle in the title bar before a conversation exists']
]
function suite() {
  rmSync(report, { force: true })
  try { execFileSync('pnpm', ['exec', 'vitest', 'run', ...tests, '--reporter=json', '--outputFile=terminal-control-result.json'], { cwd: root, shell: process.platform === 'win32', stdio: 'pipe' }) } catch { /* assertions below judge the report */ }
  return JSON.parse(readFileSync(report, 'utf8'))
}
try {
  const baseline = suite()
  assert.equal(baseline.numFailedTests, 0)
  assert.equal(baseline.numTotalTests, 21)
  for (const [file, from, to, expected] of mutations) {
    const path = join(root, file)
    const original = readFileSync(path, 'utf8')
    assert.ok(original.includes(from), `${file}: anchor not found`)
    try {
      writeFileSync(path, original.replace(from, to))
      const result = suite()
      assert.equal(result.numTotalTests, baseline.numTotalTests, 'mutant must still collect all tests')
      const failed = result.testResults.flatMap(file => file.assertionResults ?? []).filter(test => test.status === 'failed').map(test => test.title)
      assert.ok(failed.includes(expected), `mutation survived: ${expected}: ${failed.join(', ')}`)
      console.log(`CAUGHT: ${expected}`)
    } finally { writeFileSync(path, original) }
  }
} finally { rmSync(report, { force: true }) }
console.log('ALL 12 TERMINAL CONTROLS CAUGHT')
