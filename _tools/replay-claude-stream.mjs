// Replay a recorded Claude Code stream through Locust's adapter and thread.
//
//   node _tools/replay-claude-stream.mjs <stream.jsonl> <report.json>
//
// Record the stream with the argv Locust itself uses (commands.ts), e.g.
//
//   claude --restricted --print --output-format stream-json --verbose \
//     --include-partial-messages --permission-mode acceptEdits \
//     --tools "Read,Glob,Grep,Edit,Write,NotebookEdit,Bash,Task,mcp__*" \
//     --model haiku "<task>" > stream.jsonl
//
// Keep recorded streams out of the repository: the init record lists the
// account's connectors by name.
//
// Bundles `_tools/replay/claude-stream.ts` with the workspace's own esbuild
// (the adapter and the thread are TypeScript sources), then runs it.

import { spawnSync } from 'node:child_process'
import { readdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const ROOT = localPath(new URL('../', import.meta.url))
const [input, output] = process.argv.slice(2)
if (input === undefined || output === undefined) {
  console.error('usage: node _tools/replay-claude-stream.mjs <stream.jsonl> <report.json>')
  process.exit(1)
}
const esbuildDir = readdirSync(join(ROOT, 'node_modules', '.pnpm')).filter((name) => name.startsWith('esbuild@')).sort().at(-1)
if (esbuildDir === undefined) {
  console.error('no esbuild in node_modules/.pnpm -- run pnpm install')
  process.exit(1)
}
const esbuild = join(ROOT, 'node_modules', '.pnpm', esbuildDir, 'node_modules', 'esbuild', 'bin', 'esbuild')
const bundle = join(tmpdir(), `locust-replay-${String(process.pid)}.mjs`)
const built = spawnSync(process.execPath, [esbuild, join(ROOT, '_tools', 'replay', 'claude-stream.ts'), '--bundle', '--platform=node', '--format=esm', `--outfile=${bundle}`, '--log-level=warning'], { stdio: 'inherit' })
if (built.status !== 0) process.exit(built.status ?? 1)
const ran = spawnSync(process.execPath, [bundle, input, output], { stdio: 'inherit' })
process.exit(ran.status ?? 1)
