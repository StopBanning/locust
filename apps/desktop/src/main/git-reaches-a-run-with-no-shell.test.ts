import { execFileSync, spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { existsSync, statSync } from 'node:fs'
import { mkdtemp, readdir, rm, utimes, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, describe, expect, it } from 'vitest'

/*
 * GIT REACHES A RUN WITH NO SHELL (0.818). Ask and Plan on OpenCode refuse every shell command again: OpenCode
 * matched 0.744's `git status*` allow with the redirect taken off, so `git status > status.txt` wrote the file and
 * `git log -1 > ../outside.txt` wrote one outside the folder (measured 2026-10-22). Git reaches the run through this
 * server instead, which runs git with no shell and checks each argument.
 */
const server = fileURLToPath(new URL('../../resources/locust-git-reads.mjs', import.meta.url))
const roots: string[] = []
const children: ChildProcessWithoutNullStreams[] = []
afterEach(async () => {
  for (const child of children.splice(0)) {
    if (child.exitCode === null) { const closed = new Promise<void>((resolve) => child.once('close', () => resolve())); child.kill(); await closed }
  }
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true })
})

const git = (cwd: string, ...args: string[]): string => execFileSync('git', ['-c', 'user.name=Locust test', '-c', 'user.email=test@locust.test', ...args], { cwd, encoding: 'utf8', windowsHide: true })

/** A repository with one commit and an uncommitted change that breaks it, inside a folder that has a sibling. */
async function repository(): Promise<{ readonly root: string; readonly folder: string }> {
  const root = await mkdtemp(join(tmpdir(), 'locust-git-reads-'))
  roots.push(root)
  const folder = join(root, 'project')
  git(root, 'init', '-q', 'project')
  git(folder, 'config', 'core.autocrlf', 'false')
  await writeFile(join(folder, 'calc.py'), 'def add(a, b):\n    return a + b\n')
  git(folder, 'add', 'calc.py')
  git(folder, 'commit', '-q', '-m', 'add calc')
  await writeFile(join(folder, 'calc.py'), 'def add(a, b):\n    return a - b\n')
  return { root, folder }
}

function start(folder: string, env: NodeJS.ProcessEnv = {}) {
  const child = spawn(process.execPath, [server], { stdio: 'pipe', windowsHide: true, env: { ...process.env, ...env, LOCUST_GIT_FOLDER: folder } })
  children.push(child)
  let at = 0, output = ''
  const waiting = new Map<number, (value: any) => void>()
  child.stdout.setEncoding('utf8')
  child.stdout.on('data', (chunk: string) => {
    output += chunk
    let cut
    while ((cut = output.indexOf('\n')) >= 0) {
      const rpc = JSON.parse(output.slice(0, cut)); output = output.slice(cut + 1)
      waiting.get(rpc.id)?.(rpc); waiting.delete(rpc.id)
    }
  })
  const rpc = (method: string, params?: unknown): Promise<any> => new Promise((resolve, reject) => {
    const id = ++at
    const timer = setTimeout(() => { waiting.delete(id); reject(new Error('the git tool did not answer')) }, 15_000)
    waiting.set(id, (value) => { clearTimeout(timer); resolve(value) })
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
  const call = async (name: string, args: Record<string, unknown> = {}): Promise<{ readonly text: string; readonly error: boolean }> => {
    const answer = await rpc('tools/call', { name, arguments: args })
    return { text: answer.result.content.map((part: { text: string }) => part.text).join(''), error: answer.result.isError === true }
  }
  return { rpc, call }
}

describe('Locust’s git tool, for a run with no shell', () => {
  it('lists six tools, every one marked read only', async () => {
    const { folder } = await repository()
    const tool = start(folder)
    expect((await tool.rpc('initialize', { protocolVersion: '2025-06-18' })).result.serverInfo.name).toBe('locust_git')
    const tools = (await tool.rpc('tools/list')).result.tools as { name: string; annotations: { readOnlyHint: boolean } }[]
    expect(tools.map((one) => one.name)).toEqual(['status', 'diff', 'log', 'show', 'blame', 'branches'])
    for (const one of tools) expect(one.annotations.readOnlyHint, one.name).toBe(true)
  })

  it('reads what a review needs: the change, the history, a file as it was', async () => {
    const { folder } = await repository()
    const tool = start(folder)
    expect((await tool.call('status')).text).toMatch(/^## .+\n M calc\.py/)
    const diff = await tool.call('diff')
    expect(diff.error).toBe(false)
    expect(diff.text).toContain('-    return a + b\n+    return a - b')
    expect((await tool.call('diff', { stat: true, paths: ['calc.py'] })).text).toContain('1 file changed')
    expect((await tool.call('log', { oneline: true, count: 5 })).text).toMatch(/^[0-9a-f]+ add calc\n$/)
    expect((await tool.call('show', { object: 'HEAD:calc.py' })).text).toBe('def add(a, b):\n    return a + b\n')
    expect((await tool.call('blame', { path: 'calc.py' })).text).toContain('Not Committed Yet')
    expect((await tool.call('branches')).text).toContain('add calc')
    expect((await tool.call('diff', { staged: true })).text).toBe('No difference.')
  })

  it('takes no option, no path outside the folder, and nothing it does not name', async () => {
    const { root, folder } = await repository()
    const tool = start(folder)
    for (const [name, args] of [
      ['diff', { against: '--output=../written.txt' }],
      ['log', { revision: '--output=written.txt' }],
      ['show', { object: '--ext-diff' }],
      ['log', { paths: ['../outside'] }],
      ['diff', { paths: [join(root, 'elsewhere.txt')] }],
      ['diff', { paths: [':(top)'] }],
      ['blame', { path: '../calc.py' }],
      ['status', { porcelain: '--output=written.txt' }],
      ['log', { count: 0 }],
      ['diff', { staged: 'yes' }]
    ] as const) {
      const said = await tool.call(name, args)
      expect(said.error, `${name} ${JSON.stringify(args)}`).toBe(true)
      expect(said.text, `${name} ${JSON.stringify(args)}`).toMatch(/Nothing was read\.$/)
    }
    expect(existsSync(join(folder, 'written.txt')) || existsSync(join(root, 'written.txt'))).toBe(false)
    // An unknown tool is the protocol's error, not a git call.
    expect((await tool.rpc('tools/call', { name: 'push', arguments: {} })).error.code).toBe(-32602)
  })

  it('writes nothing: no index refresh, no lock, and no program a variable names', async () => {
    const { folder } = await repository()
    // A committed file touched but not changed: `git status` would refresh the index for it, and write the index.
    await writeFile(join(folder, 'notes.txt'), 'Status: draft\n')
    git(folder, 'add', 'notes.txt')
    git(folder, 'commit', '-q', '-m', 'add notes')
    const later = new Date(Date.now() + 60_000)
    await utimes(join(folder, 'notes.txt'), later, later)
    const index = join(folder, '.git', 'index')
    const before = statSync(index).mtimeMs
    const listed = (await readdir(folder)).sort()
    // `touch` as git's external diff would make files named after its arguments, were it run.
    const tool = start(folder, { GIT_EXTERNAL_DIFF: 'touch', GIT_DIR: join(folder, 'elsewhere') })
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect((await tool.call('status')).error).toBe(false)
    expect((await tool.call('diff')).text).toContain('+    return a - b')
    expect((await tool.call('log', { patch: true })).error).toBe(false)
    expect(statSync(index).mtimeMs).toBe(before)
    expect((await readdir(folder)).sort()).toEqual(listed)
    expect((await readdir(join(folder, '.git'))).filter((name) => name.endsWith('.lock'))).toEqual([])
    // The copy of the index a diff read is gone with it.
    expect((await readdir(tmpdir())).filter((name) => name.startsWith('locust-git-index-'))).toEqual([])
  })

  it('says so when the folder is not a repository', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-git-reads-none-'))
    roots.push(root)
    const said = await start(root, { GIT_CEILING_DIRECTORIES: root }).call('status')
    expect(said).toEqual({ text: 'This folder is not a git repository, so there is no history to read. Your read, grep, glob and list tools still work.', error: true })
  })
})
