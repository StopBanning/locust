import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createRequire } from 'node:module'
import { createServer, type Server } from 'node:http'
import { fileURLToPath } from 'node:url'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createLocustMcpHost, MCP_CONNECTION_FILE, mcpText } from './locust-mcp-host.js'

const bridge = fileURLToPath(new URL('../../resources/locust-mcp-bridge.mjs', import.meta.url))
const roots: string[] = []
const children: ChildProcessWithoutNullStreams[] = []
const hosts: ReturnType<typeof createLocustMcpHost>[] = []
const servers: Server[] = []
afterEach(async () => {
  for (const child of children.splice(0)) {
    if (child.exitCode === null) { const closed = new Promise<void>(resolve => child.once('close', () => resolve())); child.kill(); await closed }
  }
  for (const host of hosts.splice(0)) await host.dispose()
  for (const server of servers.splice(0)) { server.closeAllConnections(); await new Promise<void>(resolve => server.close(() => resolve())) }
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true })
})
async function fixture(executable = process.execPath) {
  const directory = await mkdtemp(join(tmpdir(), 'locust-stdio-mcp-'))
  roots.push(directory)
  const call = vi.fn(async (name: string) => mcpText(`answered ${name}`))
  const host = createLocustMcpHost({ directory, node: executable, bridge, call })
  hosts.push(host)
  const connection = join(directory, MCP_CONNECTION_FILE)
  const child = spawn(executable, [bridge, connection], { stdio: 'pipe', windowsHide: true, env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' } })
  children.push(child)
  let at = 0, output = ''
  const waiting = new Map<number, (value: any) => void>()
  child.stdout.setEncoding('utf8')
  child.stdout.on('data', chunk => {
    output += chunk
    let cut
    while ((cut = output.indexOf('\n')) >= 0) {
      const rpc = JSON.parse(output.slice(0, cut)); output = output.slice(cut + 1)
      waiting.get(rpc.id)?.(rpc); waiting.delete(rpc.id)
    }
  })
  const rpc = (method: string, params?: unknown): Promise<any> => new Promise((resolve, reject) => {
    const id = ++at
    const timer = setTimeout(() => { waiting.delete(id); reject(new Error('bridge did not answer')) }, 4000)
    waiting.set(id, value => { clearTimeout(timer); resolve(value) })
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
  await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'Locust test', version: '1' } })
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n')
  return { directory, connection, host, call, rpc }
}
describe('other apps reach Locust through stdio', () => {
  it('advertises exactly the nine tools, with Ask mode and spend made explicit', async () => {
    const f = await fixture()
    const answer = await f.rpc('tools/list')
    expect(answer.result.tools.map((tool: { name: string }) => tool.name)).toEqual(['list_teammates', 'start_conversation', 'send_message', 'read_reply', 'list_models', 'list_background_runs', 'list_routines', 'run_routine', 'routine_status'])
    const described = (name: string): string => answer.result.tools.find((tool: { name: string }) => tool.name === name).description
    expect(described('start_conversation')).toContain('Ask mode (read only), unless the person has turned on')
    expect(described('start_conversation')).toContain("answered by the person in Locust's window, never from here")
    for (const name of ['start_conversation', 'send_message']) expect(described(name)).toMatch(/no automatic teammate handoffs/i)
    expect(answer.result.tools[1].description).toContain('monthly limits apply')
    // Each that only reads says so (0.754), so an app may run it without asking -- OpenClaw's doctor said of the
    // unmarked list "calls require approval"; what starts work, or continues it, is unmarked and still asks.
    const readOnly = answer.result.tools.filter((tool: { annotations?: { readOnlyHint?: boolean } }) => tool.annotations?.readOnlyHint === true).map((tool: { name: string }) => tool.name)
    expect(readOnly).toEqual(['list_teammates', 'read_reply', 'list_models', 'list_background_runs', 'list_routines', 'routine_status'])
    expect(f.call).not.toHaveBeenCalled()
  })
  it('answers every tool call with the required plain sentence when Locust is not running', async () => {
    const f = await fixture()
    for (const name of ['list_teammates', 'start_conversation', 'send_message', 'read_reply', 'list_background_runs', 'list_routines', 'run_routine', 'routine_status']) {
      expect((await f.rpc('tools/call', { name, arguments: {} })).result).toEqual(mcpText('Locust is not running. Open it and try again.', true))
    }
    expect(f.call).not.toHaveBeenCalled()
  })
  it('also runs through Electron in Node mode, without launching the desktop app', { timeout: 20_000 }, async () => {
    const electron = createRequire(import.meta.url)('electron') as string
    const f = await fixture(electron)
    expect((await f.rpc('tools/call', { name: 'list_teammates' })).result).toEqual(mcpText('Locust is not running. Open it and try again.', true))
    await f.host.setEnabled(true)
    expect((await f.rpc('tools/call', { name: 'list_teammates' })).result).toEqual(mcpText('answered list_teammates'))
    expect(f.call).toHaveBeenCalledOnce()
  })
  it('gives the required not-running sentence for a stale file whose listener has closed', async () => {
    const f = await fixture()
    await f.host.setEnabled(true)
    const connection = await readFile(f.connection, 'utf8')
    await f.host.setEnabled(false)
    await writeFile(f.connection, connection)
    expect((await f.rpc('tools/call', { name: 'list_teammates' })).result).toEqual(mcpText('Locust is not running. Open it and try again.', true))
    expect(f.call).not.toHaveBeenCalled()
  })
  it('does not retry or claim nothing ran when a connection drops after the app received the request', async () => {
    const f = await fixture()
    let received = 0
    const server = createServer(req => { received += 1; req.socket.destroy() })
    servers.push(server)
    await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
    const address = server.address()
    if (typeof address !== 'object' || address === null) throw new Error('no port')
    await writeFile(f.connection, JSON.stringify({ url: `http://127.0.0.1:${address.port}/tools/call`, token: 'a'.repeat(64) }))
    expect((await f.rpc('tools/call', { name: 'start_conversation', arguments: { teammate: 'Wren', message: 'Review this' } })).result).toEqual(mcpText('The connection to Locust ended without an answer. Nothing was retried; check its history before starting again.', true))
    expect(received).toBe(1)
  })
  it('forwards an authenticated tool call, and sees a rotated connection without restarting the bridge', { timeout: 20_000 }, async () => {
    const f = await fixture()
    await f.host.setEnabled(true)
    expect((await f.rpc('tools/call', { name: 'list_teammates', arguments: {} })).result).toEqual(mcpText('answered list_teammates'))
    await f.host.setEnabled(false)
    expect((await f.rpc('tools/call', { name: 'read_reply', arguments: { conversation_id: 'mission_1' } })).result.content[0].text).toBe('Locust is not running. Open it and try again.')
    await f.host.setEnabled(true)
    expect((await f.rpc('tools/call', { name: 'list_background_runs', arguments: {} })).result).toEqual(mcpText('answered list_background_runs'))
    expect(f.call.mock.calls.map(([name]) => name)).toEqual(['list_teammates', 'list_background_runs'])
  })
  it('never sends a token to a non-loopback address from a modified profile file', async () => {
    const f = await fixture()
    await writeFile(f.connection, JSON.stringify({ url: 'https://example.test/tools/call', token: 'a'.repeat(64) }))
    expect((await f.rpc('tools/call', { name: 'list_teammates' })).result).toEqual(mcpText('Locust is not running. Open it and try again.', true))
    expect(f.call).not.toHaveBeenCalled()
  })
  it('refuses unknown tools and methods without forwarding them', async () => {
    const f = await fixture()
    expect((await f.rpc('tools/call', { name: 'write_files' })).error.code).toBe(-32602)
    expect((await f.rpc('exec', {})).error.code).toBe(-32601)
    expect((await f.rpc('ping')).result).toEqual({})
    expect(f.call).not.toHaveBeenCalled()
  })
  it('does not return a bearer token or connection metadata in the window settings', async () => {
    const f = await fixture()
    const state = await f.host.setEnabled(true)
    const secret = JSON.parse(await readFile(f.connection, 'utf8')) as { token: string; url: string }
    expect(JSON.stringify(state).includes(secret.token)).toBe(false)
    expect(JSON.stringify(state).includes(secret.url)).toBe(false)
    expect(state.claudeCommand).toContain(f.connection)
  })
})
