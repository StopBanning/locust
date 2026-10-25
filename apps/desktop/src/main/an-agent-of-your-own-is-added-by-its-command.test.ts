import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { acpCatalog, acpRouteModel, commandWords, modelOptionOf, parsedAcpAgents, parsedAcpRouteModel } from '../shared/acp-agents.js'
import { whatItMayDo } from '../shared/may-do.js'
import { createAcpAgents, testAcpAgent } from './acp-agents.js'
import { startAppServerProcess } from './app-server-process.js'
import { isFreeRoute } from './free-routes.js'

/*
 * YOUR OWN AGENTS (0.777). An agent is a command line; Test opens a session with it and keeps what it answered; a
 * route names it and the model picked on it. The agent here is a stand-in speaking the protocol, run under Node.
 */
const STUB = `
let buffer = ''
process.stdin.on('data', (chunk) => {
  buffer += chunk
  let at
  while ((at = buffer.indexOf('\\n')) >= 0) {
    const line = buffer.slice(0, at); buffer = buffer.slice(at + 1)
    if (!line.trim()) continue
    const message = JSON.parse(line)
    const reply = (result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, result }) + '\\n')
    if (message.method === 'initialize') reply({ protocolVersion: 1, agentInfo: { name: 'Stub Agent', version: '2.1.0' }, authMethods: [{ id: 'login', name: 'Login', description: 'Run stub login in a terminal' }] })
    else if (message.method === 'session/new') {
      if (process.argv.includes('--locked')) process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, error: { code: -32000, message: 'auth required' } }) + '\\n')
      else reply({ sessionId: 's1', configOptions: [
        { id: 'model', category: 'model', currentValue: 'stub/fast', options: [{ group: 'Stub', options: [{ value: 'stub/fast', name: 'Fast' }, { value: 'stub/deep', name: 'Deep' }] }] },
        { id: 'mode', category: 'mode', currentValue: 'build', options: [{ value: 'build', name: 'Build' }] }
      ] })
    }
  }
})
`
async function stub(): Promise<string> {
  const folder = await mkdtemp(join(tmpdir(), 'locust-acp-stub-'))
  const file = join(folder, 'stub-agent.mjs')
  await writeFile(file, STUB, 'utf8')
  return file
}
const spawn = (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string) => startAppServerProcess(executablePath, args, undefined, env, cwd)
const scratch = () => mkdtemp(join(tmpdir(), 'locust-acp-test-'))
const node = { commandName: 'node', discoveredPath: process.execPath, executablePath: process.execPath, prefixArgs: [], kind: 'native' as const }

describe('an agent of your own', () => {
  it('is a program and its arguments, never a shell line', () => {
    expect(commandWords('goose acp')).toEqual(['goose', 'acp'])
    expect(commandWords('"C:\\Program Files\\Goose\\goose.exe" acp --debug')).toEqual(['C:\\Program Files\\Goose\\goose.exe', 'acp', '--debug'])
    expect(commandWords('npx -y @zed-industries/claude-code-acp')).toEqual(['npx', '-y', '@zed-industries/claude-code-acp'])
    for (const refused of ['', '   ', 'goose acp && rm -rf ~', 'goose acp | tee x', 'cmd /c goose', 'powershell -c goose', '"unclosed acp']) {
      expect(commandWords(refused), refused).toBeUndefined()
    }
  })

  it('is named on a route with the model picked on it', () => {
    expect(acpRouteModel('acp_goose')).toBe('acp_goose')
    expect(acpRouteModel('acp_goose', 'gpt-x')).toBe('acp_goose::gpt-x')
    expect(parsedAcpRouteModel('acp_goose::opencode/muse-free')).toEqual({ agentId: 'acp_goose', model: 'opencode/muse-free' })
    expect(parsedAcpRouteModel('acp_goose')).toEqual({ agentId: 'acp_goose' })
    expect(parsedAcpRouteModel('opencode/muse-free')).toBeUndefined()
  })

  it('reads the models an agent offers from its session, grouped or not', () => {
    // All of them (0.835): Kilo offers 336, and the first 200 were all that was kept.
    const kilo = Array.from({ length: 336 }, (_, at) => ({ value: `kilo/model-${String(at)}`, name: `Model ${String(at)}` }))
    expect(modelOptionOf({ configOptions: [{ id: 'model', category: 'model', currentValue: 'kilo/model-0', options: kilo }] })?.models).toHaveLength(336)
    expect(modelOptionOf({ configOptions: [{ id: 'model', category: 'model', currentValue: 'a', options: [{ options: [{ value: 'a', name: 'A' }] }, { value: 'b' }] }] }))
      .toEqual({ id: 'model', current: 'a', models: [{ value: 'a', name: 'A' }, { value: 'b', name: 'b' }] })
    expect(modelOptionOf({ configOptions: [{ id: 'mode', category: 'mode', options: [] }] })).toBeUndefined()
    expect(modelOptionOf({})).toBeUndefined()
  })

  it('lists one row per model in the picker, or its own default when it offers none', () => {
    const rows = acpCatalog([
      { id: 'acp_stub', name: 'Stub', commandLine: 'stub', answer: { agentName: 'Stub Agent', modelOption: { id: 'model', models: [{ value: 'stub/fast', name: 'Fast' }] } } },
      { id: 'acp_bare', name: 'Bare', commandLine: 'bare' }
    ])
    expect(rows.map((row) => [row.id, row.displayName, row.agentName, row.runtime])).toEqual([
      ['acp_stub::stub/fast', 'Fast', 'Stub', 'acp'],
      ['acp_bare', 'Its own default model', 'Bare', 'acp']
    ])
  })

  it('keeps only well-formed agents from a file someone edited by hand', () => {
    expect(parsedAcpAgents({ agents: [
      { id: 'acp_ok', name: 'Ok', commandLine: 'ok acp' },
      { id: 'acp_ok', name: 'Twice', commandLine: 'ok acp' },
      { id: 'not-acp', name: 'Bad id', commandLine: 'x' },
      { id: 'acp_shell', name: 'Shell', commandLine: 'x && y' }
    ] }).map((agent) => agent.id)).toEqual(['acp_ok'])
  })

  it('is tested by opening a session and closing it: its name, version and models, no prompt', async () => {
    const tested = await testAcpAgent({ launch: node, args: [await stub()] }, { spawn, scratch })
    expect(tested).toEqual({ ok: true, answer: {
      agentName: 'Stub Agent', version: '2.1.0',
      modelOption: { id: 'model', current: 'stub/fast', models: [{ value: 'stub/fast', name: 'Fast' }, { value: 'stub/deep', name: 'Deep' }] },
      signIn: ['Run stub login in a terminal']
    } })
  })

  it('says how to sign in when the agent will not open a session', async () => {
    const tested = await testAcpAgent({ launch: node, args: [await stub(), '--locked'] }, { spawn, scratch })
    expect(tested).toEqual({ ok: false, message: 'It answered, but would not open a session. It says to sign in: Run stub login in a terminal.' })
  })

  it('says so when the program is not an agent at all', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-acp-quiet-'))
    const quiet = join(folder, 'quiet.mjs')
    await writeFile(quiet, 'process.exit(3)\n', 'utf8')
    const tested = await testAcpAgent({ launch: node, args: [quiet] }, { spawn, scratch })
    expect(tested.ok).toBe(false)
    expect(!tested.ok && tested.message).toMatch(/^It stopped without answering\./)
  })

  it('is kept only when it answered, under a name of its own, and found again for a run', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'locust-acp-store-'))
    const file = await stub()
    const agents = createAcpAgents({ directory, locate: async (command) => (command === 'node' ? node : undefined), spawn, scratch })
    expect(await agents.save('Stub', 'nowhere-agent acp')).toEqual({ ok: false, message: 'Locust could not find nowhere-agent. Install it, or check it opens in a new terminal, then test again.' })
    const saved = await agents.save('Stub', `node "${file}"`)
    expect(saved.ok && saved.agents.map((agent) => [agent.id, agent.answer?.agentName])).toEqual([['acp_stub', 'Stub Agent']])
    expect(await agents.save('stub', `node "${file}"`)).toEqual({ ok: false, message: 'You already have an agent called stub.' })
    const found = await agents.resolve('acp_stub')
    expect('found' in found && found.found.args).toEqual([file])
    expect(await agents.resolve('acp_gone')).toEqual({ message: 'That agent is not in Settings > AI agents > Your own agents any more.' })
    expect((await agents.remove('acp_stub')).ok && (await agents.list())).toEqual({ ok: true, agents: [] })
  })

  it('may do only what it asks and the person approves, and costs something unless it is OpenCode’s own free model', () => {
    expect(whatItMayDo('acp', 'workspace-write', 'approve-each').rows.map((row) => row.text)).toEqual([
      'read the files in this folder',
      "change files and run commands when it asks and you approve; which actions it asks about is the agent's own choice"
    ])
    expect(isFreeRoute('acp', 'acp_opencode::opencode/muse-spark-1.3-contributor-free')).toBe(true)
    expect(isFreeRoute('acp', 'acp_goose::gpt-5')).toBe(false)
    expect(isFreeRoute('acp', 'acp_goose')).toBe(false)
  })
})
