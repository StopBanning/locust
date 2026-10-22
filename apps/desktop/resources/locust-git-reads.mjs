// Dependency-free per-run stdio MCP server: git that only reads, for a teammate in a mode with no shell (0.818).
// OpenCode starts it for one Ask or Plan run, in that run's folder (LOCUST_GIT_FOLDER); it is never written to an
// agent's own config.
//
// WHY NOT THE SHELL. 0.744 let Ask mode's shell run `git status*`, `git diff*` and the other reads. OpenCode checks a
// shell command against those patterns with its redirects taken off, so `git status > notes.txt` was let through and
// wrote the file; `git log -1 > ../outside.txt` wrote one outside the folder (MEASURED 2026-10-22, OpenCode 1.18.27,
// deepseek-v4.1-flash-free, Locust's own Ask config). A pattern cannot see a redirect, so Ask mode's shell refuses
// everything again, and git reaches the model here instead: git itself, run with no shell, each argument checked.
//
// Nothing here writes: GIT_OPTIONAL_LOCKS=0 keeps `git status` from refreshing the index, a diff reads a copy of
// the index (indexCopy), every git variable the run inherited is dropped, and git is told what Locust's own git is
// told (git-guard.ts: core.fsmonitor off; plain diffs, no external diff program and no textconv filter).
import { execFile } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { copyFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { isAbsolute, join, relative, resolve, sep } from 'node:path'

const FOLDER = process.env.LOCUST_GIT_FOLDER
const MAX_CHARS = 60_000
const TIME_LIMIT_MS = 30_000
const OWN_CONFIG = ['--no-pager', '-c', 'core.fsmonitor=false']
const PLAIN = ['--no-color', '--no-ext-diff', '--no-textconv']

const textResult = (text, isError = false) => ({
  content: [{ type: 'text', text }],
  ...(isError ? { isError: true } : {})
})
const schema = (properties = {}, required = []) => ({
  type: 'object',
  properties,
  required,
  additionalProperties: false
})
const READ_ONLY = { readOnlyHint: true, destructiveHint: false, openWorldHint: false }
const REVISION = { type: 'string', minLength: 1, maxLength: 300 }
const PATHS = {
  type: 'array',
  items: { type: 'string', minLength: 1, maxLength: 400 },
  maxItems: 50,
  description: 'Only these files or folders, relative to this folder.'
}

const TOOLS = [
  {
    name: 'status',
    description: 'git status for this folder: its branch, and each changed, staged, deleted and untracked file. Read only.',
    inputSchema: schema(),
    annotations: READ_ONLY
  },
  {
    name: 'diff',
    description:
      'git diff: the uncommitted change by default. staged: true shows what is staged; against compares with a commit, branch or range such as main...HEAD. Untracked files are not in a diff: status lists them, and your read tool shows them. Read only.',
    inputSchema: schema({
      staged: { type: 'boolean', description: 'What is staged for the next commit, rather than what is not.' },
      against: { ...REVISION, description: 'A commit, branch or range to compare with, such as HEAD~1 or main...HEAD.' },
      paths: PATHS,
      stat: { type: 'boolean', description: 'Only which files changed, and by how many lines.' }
    }),
    annotations: READ_ONLY
  },
  {
    name: 'log',
    description: 'git log: the latest commits, newest first, on this branch or on revision, and only those touching paths if given. Read only.',
    inputSchema: schema({
      revision: { ...REVISION, description: 'A commit, branch or range such as main..HEAD. The current branch if left out.' },
      paths: PATHS,
      count: { type: 'integer', minimum: 1, maximum: 200, description: 'How many commits, 20 if left out.' },
      oneline: { type: 'boolean', description: 'One line per commit.' },
      patch: { type: 'boolean', description: 'Each commit with its change.' },
      stat: { type: 'boolean', description: 'Each commit with the files it changed.' }
    }),
    annotations: READ_ONLY
  },
  {
    name: 'show',
    description: 'git show: one commit with its change, or commit:path for a file as it was in that commit. Read only.',
    inputSchema: schema(
      {
        object: { ...REVISION, description: 'A commit such as HEAD or a1b2c3d, or commit:path such as HEAD~2:src/app.ts.' },
        stat: { type: 'boolean', description: 'Only the files the commit changed, not the change itself.' }
      },
      ['object']
    ),
    annotations: READ_ONLY
  },
  {
    name: 'blame',
    description: 'git blame: who last changed each line of a file, and in which commit. Read only.',
    inputSchema: schema(
      {
        path: { type: 'string', minLength: 1, maxLength: 400, description: 'The file, relative to this folder.' },
        revision: { ...REVISION, description: 'As of this commit; the working file if left out.' }
      },
      ['path']
    ),
    annotations: READ_ONLY
  },
  {
    name: 'branches',
    description: 'git branch: every local and remote branch, with the commit it is on and what it tracks. Read only.',
    inputSchema: schema(),
    annotations: READ_ONLY
  }
]

class Refusal extends Error {}

/** A revision, range or commit:path, as one argument git cannot take for an option. */
function revision(value, what) {
  if (value === undefined) return undefined
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > 300 || /[\0\r\n]/.test(value)) {
    throw new Refusal(`${what} must be a commit, branch or range, such as HEAD~1 or main...HEAD. Nothing was read.`)
  }
  if (value.startsWith('-')) throw new Refusal(`${what} cannot start with "-": options are not taken here. Nothing was read.`)
  return value
}

/** A path inside this folder, as git takes it after `--`. */
function inside(value) {
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > 400 || /[\0\r\n]/.test(value)) {
    throw new Refusal('Each path must be a file or folder in this folder. Nothing was read.')
  }
  // Git's own pathspec syntax (`:(top)`, `:/`) reaches past the folder; a plain path is all that is needed here.
  if (value.startsWith(':')) throw new Refusal(`${value} is not a plain path. Name the file or folder itself. Nothing was read.`)
  const within = relative(FOLDER, resolve(FOLDER, value))
  if (within === '..' || within.startsWith(`..${sep}`) || isAbsolute(within)) {
    throw new Refusal(`${value} is outside this folder. Nothing was read.`)
  }
  return within.length === 0 ? '.' : within.split(sep).join('/')
}

function paths(value) {
  if (value === undefined) return []
  if (!Array.isArray(value) || value.length > 50) throw new Refusal('paths must be a list of at most 50 files or folders. Nothing was read.')
  return value.map(inside)
}

const flag = (value, name) => {
  if (value === undefined) return false
  if (typeof value !== 'boolean') throw new Refusal(`${name} must be true or false. Nothing was read.`)
  return value
}

/** The git arguments a tool call stands for; a Refusal when one of them is not what it should be. */
export function gitArgs(name, input) {
  const args = input !== null && typeof input === 'object' && !Array.isArray(input) ? input : {}
  const known = TOOLS.find((tool) => tool.name === name)
  if (known === undefined) throw new Refusal(`There is no git tool called ${String(name)}. Nothing was read.`)
  const extra = Object.keys(args).filter((key) => !(key in known.inputSchema.properties))
  if (extra.length > 0) throw new Refusal(`git ${name} takes no ${extra.join(', ')}. Nothing was read.`)
  switch (name) {
    case 'status':
      return ['status', '--short', '--branch']
    case 'diff': {
      const against = revision(args.against, 'against')
      return ['diff', ...PLAIN, ...(flag(args.staged, 'staged') ? ['--cached'] : []), ...(flag(args.stat, 'stat') ? ['--stat'] : []), ...(against === undefined ? [] : [against]), '--', ...paths(args.paths)]
    }
    case 'log': {
      const count = args.count === undefined ? 20 : args.count
      if (!Number.isInteger(count) || count < 1 || count > 200) throw new Refusal('count must be a whole number from 1 to 200. Nothing was read.')
      const at = revision(args.revision, 'revision')
      return ['log', ...PLAIN, `--max-count=${String(count)}`, ...(flag(args.oneline, 'oneline') ? ['--oneline'] : []), ...(flag(args.patch, 'patch') ? ['--patch'] : []), ...(flag(args.stat, 'stat') ? ['--stat'] : []), ...(at === undefined ? [] : [at]), '--', ...paths(args.paths)]
    }
    case 'show': {
      const object = revision(args.object, 'object')
      if (object === undefined) throw new Refusal('show needs an object: a commit, or commit:path. Nothing was read.')
      return ['show', ...PLAIN, ...(flag(args.stat, 'stat') ? ['--stat'] : []), object, '--']
    }
    case 'blame': {
      const at = revision(args.revision, 'revision')
      if (args.path === undefined) throw new Refusal('blame needs a path: the file to read. Nothing was read.')
      return ['blame', '--no-textconv', ...(at === undefined ? [] : [at]), '--', inside(args.path)]
    }
    case 'branches':
      return ['branch', '--all', '-vv', '--no-color']
    default:
      throw new Refusal(`There is no git tool called ${String(name)}. Nothing was read.`)
  }
}

/** Git's environment: none of the run's git variables, no locks taken, no prompt, no pager. */
function gitEnv() {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^GIT_/i.test(key)))
  return { ...env, GIT_OPTIONAL_LOCKS: '0', GIT_TERMINAL_PROMPT: '0', GIT_PAGER: 'cat', PAGER: 'cat' }
}

const git = (args, env) => new Promise((done) => {
  execFile('git', [...OWN_CONFIG, ...args], { cwd: FOLDER, env, windowsHide: true, maxBuffer: 32 * 1024 * 1024, timeout: TIME_LIMIT_MS, encoding: 'utf8' }, (error, stdout, stderr) => done({ error, stdout, stderr }))
})

/**
 * `git diff` against the working tree writes the index back with what it refreshed, GIT_OPTIONAL_LOCKS or not
 * (MEASURED 2026-10-22, git 2.54: its mtime moved; `status`, `show`, `log`, `blame`, `branch` and `diff --cached`
 * left it alone). That write takes index.lock, which a commit the person makes at the same moment needs. So a diff
 * reads a copy of the index, made for it and removed after; undefined when there is no index to copy.
 */
async function indexCopy(env) {
  const found = await git(['rev-parse', '--git-path', 'index'], env)
  if (found.error) return undefined
  const copy = join(tmpdir(), `locust-git-index-${randomUUID()}`)
  try {
    await copyFile(resolve(FOLDER, String(found.stdout).trim()), copy)
    return copy
  } catch {
    return undefined
  }
}

async function runGit(name, input) {
  if (FOLDER === undefined || FOLDER.length === 0) {
    return textResult('Locust did not say which folder this run is in. Nothing was read.', true)
  }
  let args
  try {
    args = gitArgs(name, input)
  } catch (error) {
    return textResult(error instanceof Refusal ? error.message : 'That git call could not be read. Nothing was run.', true)
  }
  const env = gitEnv()
  const copy = name === 'diff' ? await indexCopy(env) : undefined
  try {
    return said(name, await git(args, copy === undefined ? env : { ...env, GIT_INDEX_FILE: copy }))
  } finally {
    if (copy !== undefined) await Promise.all([rm(copy, { force: true }), rm(`${copy}.lock`, { force: true })]).catch(() => undefined)
  }
}

/** What a git call printed, as the tool's answer. */
function said(name, { error, stdout, stderr }) {
  const why = String(stderr ?? '').trim()
  if (error) {
    if (error.killed || error.signal === 'SIGTERM') return textResult(`git ${name} took longer than ${String(TIME_LIMIT_MS / 1000)} seconds and was stopped. Ask for less, with paths or stat.`, true)
    if (error.code === 'ENOENT') return textResult('git is not installed on this computer, so there is no history to read.', true)
    if (/not a git repository/i.test(why)) return textResult('This folder is not a git repository, so there is no history to read. Your read, grep, glob and list tools still work.', true)
    return textResult(`git ${name} failed: ${why.slice(0, 2000) || 'it printed nothing to say why'}`, true)
  }
  const text = String(stdout ?? '')
  if (text.trim().length === 0) return textResult(name === 'diff' ? 'No difference.' : `git ${name} printed nothing.`)
  return textResult(text.length <= MAX_CHARS ? text : `${text.slice(0, MAX_CHARS)}\n\n[Cut at ${MAX_CHARS.toLocaleString('en-US')} characters. Ask for fewer files with paths, or stat: true for the list of files.]`)
}

const send = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n')
const sendError = (id, code, message) =>
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } }) + '\n')

// Imported by a test for gitArgs alone; started as a server, by OpenCode, otherwise.
if (process.env.LOCUST_GIT_READS_IMPORT !== '1') {
  // A call still running when the input ends is answered before the server goes.
  let pending = 0
  let ended = false
  const settle = () => {
    pending -= 1
    if (ended && pending === 0) process.exit(0)
  }
  let buffer = ''
  process.stdin.setEncoding('utf8')
  process.stdin.on('data', (chunk) => {
    buffer += String(chunk)
    if (Buffer.byteLength(buffer) > 512 * 1024) {
      sendError(null, -32600, 'MCP request is too large. Nothing was run.')
      process.exitCode = 1
      process.stdin.destroy()
      return
    }
    let cut
    while ((cut = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, cut).trim()
      buffer = buffer.slice(cut + 1)
      if (!line) continue
      let rpc
      try {
        rpc = JSON.parse(line)
      } catch {
        sendError(null, -32700, 'Invalid JSON')
        continue
      }
      if (typeof rpc !== 'object' || rpc === null || rpc.jsonrpc !== '2.0') {
        sendError(null, -32600, 'Invalid request')
        continue
      }
      if (rpc.id === undefined) continue
      if (rpc.method === 'initialize') {
        const requested = rpc.params?.protocolVersion
        const protocolVersion = ['2024-11-05', '2025-03-26', '2025-06-18'].includes(requested) ? requested : '2025-06-18'
        send(rpc.id, { protocolVersion, capabilities: { tools: {} }, serverInfo: { name: 'locust_git', version: '1.0.0' } })
        continue
      }
      if (rpc.method === 'ping') {
        send(rpc.id, {})
        continue
      }
      if (rpc.method === 'tools/list') {
        send(rpc.id, { tools: TOOLS })
        continue
      }
      if (rpc.method === 'tools/call') {
        const toolName = rpc.params?.name
        if (!TOOLS.some((tool) => tool.name === toolName)) {
          sendError(rpc.id, -32602, `Unknown git tool: ${String(toolName)}`)
          continue
        }
        pending += 1
        void runGit(toolName, rpc.params?.arguments ?? {})
          .then((answer) => send(rpc.id, answer))
          .catch(() => send(rpc.id, textResult('git could not be run. Nothing was read.', true)))
          .finally(settle)
        continue
      }
      sendError(rpc.id, -32601, 'Method not found')
    }
  })
  process.stdin.on('end', () => {
    ended = true
    if (pending === 0) process.exit(0)
  })
}
