// Records what Electron's command and path words return (missionView.ts: shellCommandText, withoutShellWrapper,
// relativePath, mcpToolParts), for the Rust port to replay (parity/recorded/words.jsonl). Run through record.sh.
// Inputs: every string written in the functions' own Electron test files, plus traps (quoting, Windows hosts, slashes,
// case, empty, emoji and other text JavaScript counts in UTF-16 units).
import { readFileSync } from 'node:fs'
import { mcpToolParts, relativePath, shellCommandText, withoutShellWrapper } from '../../../desktop/src/renderer/src/missionView.ts'

// The renderer's source folder, from record.sh (import.meta.url is the bundle's temporary file).
const here = new URL(`file://${process.env.ELECTRON_RENDERER_SRC}/`)
const TEST_FILES = [
  'missionView.test.ts',
  'a-command-reads-as-a-command.test.ts',
  'a-cloud-turn-says-where-it-ran.test.tsx',
  'a-skill-row-names-its-skill.test.ts',
  'mcp-tool-rows.test.ts',
  'commands-and-paths-share-one-left-edge.test.tsx',
  'a-diff-notes-place-is-relative-to-its-folder.test.tsx',
]

const strings = new Set<string>()
for (const name of TEST_FILES) {
  let text = ''
  try {
    text = readFileSync(new URL(name, here), 'utf8')
  } catch {
    continue
  }
  for (const m of text.matchAll(/'((?:\\.|[^'\\\n])*)'/g)) strings.add(m[1].replace(/\\(.)/g, '$1'))
  for (const m of text.matchAll(/"((?:\\.|[^"\\\n])*)"/g)) strings.add(m[1].replace(/\\(.)/g, '$1'))
  for (const m of text.matchAll(/`((?:\\.|[^`\\$\n])*)`/g)) strings.add(m[1].replace(/\\(.)/g, '$1'))
}

const COMMANDS = ['', ' ', 'ls', '  ls -la  ', 'bash -lc "npm test"', "bash -lc 'npm test'", 'bash -c ls', '/bin/bash -lc ls',
  '/bin/zsh -lc "git status"', 'zsh -c "a && b"', 'sh -c \'echo "hi"\'', 'bash -lc', 'bash -l', 'bash -lc ""',
  'cmd /c dir', 'cmd.exe /C "dir /b"', 'cmd /k echo', 'powershell -Command Get-ChildItem',
  'powershell.exe -NoProfile -Command "Get-Content \'a b.txt\'"', 'pwsh -NoProfile -NonInteractive -Command "ls"',
  '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Item ""x"""',
  'C:\\Windows\\System32\\cmd.exe /d /s /c "npm run build"', 'PowerShell.EXE -command dir', 'CMD /C dir',
  'bash -lc "echo \\"quoted\\""', 'bash -lc "a" extra', 'bash  -lc   "spaced"', 'fish -c ls', 'bash -x ls',
  'bash -lc "🚀 launch"', 'bash -lc "émile"', 'sudo bash -c ls', 'env bash -c ls', 'bash -ic "x"', 'bash -lic "x"']
const PATHS = ['', '/', 'a.txt', '/repo/a.txt', '/repo/src/a.ts', '/repo', '/repo/', '/repo2/a.txt', '/rep/a.txt',
  'C:\\repo\\src\\a.ts', 'C:/repo/src/a.ts', 'c:\\repo\\a.ts', '\\\\server\\share\\a.txt', '/Repo/a.txt',
  '/repo//src///a.ts', './a.ts', '../a.ts', '~/a.ts', '/repo/🚀.ts', '/repo/émile.ts', '/other/place/file.md',
  '/home/me/.locust/compare/abc/src/a.ts', '/Users/me/.locust/compare/x1/a.ts', '/tmp/x/a.ts']
const ROOTS: (string | undefined)[] = [undefined, '', '/', '/repo', '/repo/', 'C:\\repo', 'C:/repo', 'c:\\repo', '/Repo',
  '/repo//', '\\\\server\\share', '/home/me/project', '/Users/me/project']
const NAMES = ['', 'mcp__', 'mcp__a__b', 'mcp__github__create_issue', 'mcp__claude_ai_Gmail__search', 'mcp__a__',
  'mcp____b', 'mcp__a__b__c', 'mcp__plugin_x_y__tool', 'MCP__a__b', 'mcp_a_b', 'github.create_issue', 'github/create_issue',
  'mcp:github:create_issue', 'read', 'Bash', 'mcp__5ca4be83-f613-4cd0-84d7-a9e2af802fba__send_message', 'mcp__🚀__go',
  'mcp__claude-in-chrome__navigate', 'mcp__Claude_Browser__computer', 'mcp.github.create.issue', 'mcp_tool.a.b.c',
  'mcp_github.list.repos', 'mcp.a.', 'mcp..b', 'mcptool.x.y']

const short = [...strings].filter((s) => s.length <= 200)
const commands = [...new Set([...COMMANDS, ...short])]
const paths = [...new Set([...PATHS, ...short.filter((s) => /[\/]/.test(s) && s.length <= 120)])]
const names = [...new Set([...NAMES, ...short.filter((s) => /mcp|__|[.:/]/.test(s) && s.length <= 100)])]
const roots = [...new Set([...ROOTS, ...short.filter((s) => /^([A-Za-z]:)?[\\/]/.test(s) && !/\.[a-z]+$/i.test(s))])]

const out: string[] = []
const record = (fn: string, args: unknown[], value: unknown) =>
  out.push(JSON.stringify({ fn, args, out: value === undefined ? null : value }))
const attempt = (f: () => unknown) => {
  try {
    return f()
  } catch (e) {
    return { threw: String(e) }
  }
}
for (const c of commands) {
  record('shellCommandText', [c], attempt(() => shellCommandText(c)))
  record('withoutShellWrapper', [c], attempt(() => withoutShellWrapper(c)))
}
for (const p of paths) for (const r of roots) record('relativePath', [p, r ?? null], attempt(() => relativePath(p, r)))
for (const n of names) record('mcpToolParts', [n], attempt(() => mcpToolParts(n)))
process.stdout.write(out.join('\n') + '\n')
