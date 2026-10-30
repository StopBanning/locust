import { request, Server, type IncomingMessage, type ServerResponse } from 'node:http'
import { mkdtemp, readFile, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createLocustMcpHost, MCP_CONNECTION_FILE, MCP_SETTINGS_FILE, mcpRequestHandler, mcpSetup, mcpText, windowsTool } from './locust-mcp-host.js'

const hosts: ReturnType<typeof createLocustMcpHost>[] = []
const roots: string[] = []
afterEach(async () => {
  for (const host of hosts.splice(0)) await host.dispose()
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true })
  vi.restoreAllMocks()
})
async function fixture(protectFile?: (path: string) => Promise<void>) {
  const directory = await mkdtemp(join(tmpdir(), 'locust-mcp-'))
  roots.push(directory)
  const call = vi.fn(async () => mcpText('hello'))
  const host = createLocustMcpHost({ directory, node: '/Locust.exe', bridge: '/bridge.mjs', call, ...(protectFile ? { protectFile } : {}) })
  hosts.push(host)
  return { directory, host, call, connection: async () => JSON.parse(await readFile(join(directory, MCP_CONNECTION_FILE), 'utf8')) as { url: string; token: string } }
}
// Headers only: a rejection MUST arrive before even an unfinished body.
const headersOnly = (url: string, token?: string): Promise<number | undefined> => new Promise((resolve, reject) => {
  const req = request(url, { method: 'POST', headers: { 'content-type': 'application/json', 'content-length': '9999', ...(token ? { authorization: `Bearer ${token}` } : {}) } }, res => {
    resolve(res.statusCode); res.resume(); req.destroy()
  })
  req.on('error', reject)
  req.setTimeout(2000, () => { req.destroy(); reject(new Error('did not refuse headers before the body')) })
  req.flushHeaders()
})

describe('other apps need a current local token', () => {
  it("keeps each teammate's own mode off by default, saves the switch, and keeps it across a restart and the server switch", async () => {
    const f = await fixture(async () => undefined)
    await f.host.load()
    expect(f.host.ownMode()).toBe(false)
    for (const odd of ['true', 1, null]) expect((await f.host.setOwnMode(odd)).message).toBe('That mode setting was not changed.')
    expect(f.host.ownMode()).toBe(false)
    expect((await f.host.setOwnMode(true)).ownMode).toBe(true)
    await f.host.setEnabled(true)
    await f.host.setEnabled(false)
    expect(JSON.parse(await readFile(join(f.directory, MCP_SETTINGS_FILE), 'utf8'))).toEqual({ enabled: false, ownMode: true })
    const again = createLocustMcpHost({ directory: f.directory, node: '/Locust.exe', bridge: '/bridge.mjs', call: f.call, protectFile: async () => undefined })
    hosts.push(again)
    expect((await again.load()).ownMode).toBe(true)
    expect((await again.setOwnMode(false)).ownMode).toBe(false)
    expect(again.ownMode()).toBe(false)
  })
  it('lets an editor answer cards only once the person turns it on, and keeps that and the editor across a restart', async () => {
    const f = await fixture(async () => undefined)
    await f.host.load()
    expect(f.host.editorAnswers()).toBe(false)
    for (const odd of ['true', 1, null]) expect((await f.host.setEditorAnswers(odd)).message).toBe('That setting was not changed.')
    expect(f.host.editorAnswers()).toBe(false)
    expect((await f.host.noteEditor()).editorSeen).toBe(true)
    expect((await f.host.setEditorAnswers(true)).editorAnswers).toBe(true)
    expect(JSON.parse(await readFile(join(f.directory, MCP_SETTINGS_FILE), 'utf8'))).toEqual({ enabled: false, ownMode: false, editorAnswers: true, editorSeen: true })
    const again = createLocustMcpHost({ directory: f.directory, node: '/Locust.exe', bridge: '/bridge.mjs', call: f.call, protectFile: async () => undefined })
    hosts.push(again)
    expect(await again.load()).toEqual({ enabled: false, ownMode: false, editorAnswers: true, editorSeen: true })
    expect(again.editorAnswers()).toBe(true)
    expect((await again.setEditorAnswers(false)).editorAnswers).toBeUndefined()
    expect(again.editorAnswers()).toBe(false)
  })
  it('is off by default, with no connection file or listening socket', async () => {
    const f = await fixture()
    expect(await f.host.load()).toEqual({ enabled: false, ownMode: false })
    expect(await stat(join(f.directory, MCP_CONNECTION_FILE)).catch(() => undefined)).toBeUndefined()
    expect(f.call).not.toHaveBeenCalled()
  })
  it.each([undefined, 'b'.repeat(64), 'old-token'])('rejects unauthorized headers without invoking any body reader (%s)', async auth => {
    const body = vi.fn(async () => ({ name: 'list_teammates' }))
    const call = vi.fn(async () => mcpText('never'))
    const end = vi.fn()
    const writeHead = vi.fn(() => ({ end }))
    mcpRequestHandler(() => 'a'.repeat(64), call, body)({ headers: { authorization: auth === undefined ? undefined : `Bearer ${auth}` } } as IncomingMessage, { writeHead } as unknown as ServerResponse)
    expect(writeHead).toHaveBeenCalledWith(401, { connection: 'close' })
    expect(body).not.toHaveBeenCalled()
    expect(call).not.toHaveBeenCalled()
  })
  it('binds only loopback, rejects no/wrong/stale tokens before an unfinished body, and closes while off', { timeout: 20_000 }, async () => {
    const listen = vi.spyOn(Server.prototype, 'listen')
    const f = await fixture()
    expect((await f.host.setEnabled(true)).enabled).toBe(true)
    expect(listen).toHaveBeenCalledWith(0, '127.0.0.1', expect.any(Function))
    const first = await f.connection()
    expect(new URL(first.url).hostname).toBe('127.0.0.1')
    expect(await headersOnly(first.url)).toBe(401)
    expect(await headersOnly(first.url, 'f'.repeat(64))).toBe(401)
    expect(f.call).not.toHaveBeenCalled()
    expect(await f.host.setEnabled(false)).toEqual({ enabled: false, ownMode: false })
    await expect(fetch(first.url)).rejects.toThrow()
    expect(await stat(join(f.directory, MCP_CONNECTION_FILE)).catch(() => undefined)).toBeUndefined()
    await f.host.setEnabled(true)
    const second = await f.connection()
    expect(second.token === first.token).toBe(false)
    expect(await headersOnly(second.url, first.token)).toBe(401)
    expect(f.call).not.toHaveBeenCalled()
    const answer = await fetch(second.url, { method: 'POST', headers: { authorization: `Bearer ${second.token}` }, body: JSON.stringify({ name: 'list_teammates', arguments: {} }) })
    expect(answer.status).toBe(200)
    expect(await answer.json()).toEqual(mcpText('hello'))
    expect(f.call).toHaveBeenCalledWith('list_teammates', {})
  })
  it('writes no bearer bytes until account-only file permissions succeed, and fails closed if they cannot', async () => {
    const protect = vi.fn(async (path: string) => { expect(await readFile(path, 'utf8')).toBe(''); throw new Error('ACL refused') })
    const f = await fixture(protect)
    expect((await f.host.setEnabled(true)).enabled).toBe(false)
    expect(protect).toHaveBeenCalledOnce()
    expect(await stat(join(f.directory, MCP_CONNECTION_FILE)).catch(() => undefined)).toBeUndefined()
    expect(f.call).not.toHaveBeenCalled()
  })
  it('revokes a valid request while its body is still arriving', async () => {
    let token: string | undefined = 'a'.repeat(64)
    const call = vi.fn(async () => mcpText('never'))
    let finish!: (value: unknown) => void
    const body = () => new Promise<unknown>(resolve => { finish = resolve })
    const end = vi.fn()
    const writeHead = vi.fn(() => ({ end }))
    mcpRequestHandler(() => token, call, body)({ headers: { authorization: `Bearer ${token}` }, method: 'POST', url: '/tools/call' } as IncomingMessage, { writeHead } as unknown as ServerResponse)
    token = undefined; finish({ name: 'start_conversation' })
    await vi.waitFor(() => expect(writeHead).toHaveBeenCalledWith(401))
    expect(call).not.toHaveBeenCalled()
  })
  it('refuses browser origins and non-tool routes before reading their bodies', () => {
    const body = vi.fn(async () => ({}))
    const call = vi.fn(async () => mcpText('never'))
    const writeHead = vi.fn(() => ({ end: vi.fn() }))
    for (const fields of [{ headers: { authorization: `Bearer ${'a'.repeat(64)}`, origin: 'https://example.test' }, method: 'POST', url: '/tools/call' }, { headers: { authorization: `Bearer ${'a'.repeat(64)}` }, method: 'GET', url: '/tools/call' }]) {
      mcpRequestHandler(() => 'a'.repeat(64), call, body)(fields as IncomingMessage, { writeHead } as unknown as ServerResponse)
    }
    expect(writeHead).toHaveBeenCalledTimes(2)
    expect(writeHead).toHaveBeenCalledWith(403, { connection: 'close' })
    expect(body).not.toHaveBeenCalled()
  })
  it('keeps setup text token-free and safely quotes spaces, backslashes and PowerShell expansion', () => {
    const setup = mcpSetup('C:\\Program Files\\Locust\\Locust.exe', 'C:\\Locust $name\\bridge.mjs', 'C:\\Profile\\locust-mcp-connection.json')
    expect(setup.claudeCommand).toBe('claude mcp add --env ELECTRON_RUN_AS_NODE=1 --transport stdio --scope user locust -- "C:\\Program Files\\Locust\\Locust.exe" "C:\\Locust `$name\\bridge.mjs" "C:\\Profile\\locust-mcp-connection.json"')
    expect(setup.codexConfig).toBe('[mcp_servers.locust]\ncommand = "C:\\\\Program Files\\\\Locust\\\\Locust.exe"\nargs = ["C:\\\\Locust $name\\\\bridge.mjs", "C:\\\\Profile\\\\locust-mcp-connection.json"]\nenv = { ELECTRON_RUN_AS_NODE = "1" }')
    // OpenClaw (0.754): its `mcp add` flags, as its own --help gives them, quoted for PowerShell like Claude Code's.
    expect(setup.openclawCommand).toBe('openclaw mcp add locust --command "C:\\Program Files\\Locust\\Locust.exe" --arg "C:\\Locust `$name\\bridge.mjs" --arg "C:\\Profile\\locust-mcp-connection.json" --env ELECTRON_RUN_AS_NODE=1')
    expect(JSON.stringify(setup)).not.toContain('token')
  })
  it('remembers enabling across app restarts but never reuses the old token, and disposes without reopening', { timeout: 20_000 }, async () => {
    const f = await fixture()
    await f.host.setEnabled(true)
    const old = await f.connection()
    await f.host.dispose()
    const next = createLocustMcpHost({ directory: f.directory, node: '/Locust.exe', bridge: '/bridge.mjs', call: f.call })
    hosts.push(next)
    expect((await next.load()).enabled).toBe(true)
    expect((await f.connection()).token === old.token).toBe(false)
    await next.dispose()
    expect((await next.setEnabled(true)).enabled).toBe(false)
  })
})

describe("the account-only lock uses Windows' own tools", () => {
  // The lock runs on a Windows host only (privateMcpFile returns first elsewhere), which joins System32's path this way.
  it.skipIf(process.platform !== 'win32')('names whoami and icacls by their System32 path, never by a PATH lookup that can find Git\'s coreutils', () => {
    expect(windowsTool('whoami.exe', { SystemRoot: 'C:\\Windows' })).toBe('C:\\Windows\\System32\\whoami.exe')
    expect(windowsTool('icacls.exe', { windir: 'D:\\Win' })).toBe('D:\\Win\\System32\\icacls.exe')
    expect(windowsTool('whoami.exe', {})).toBe('C:\\Windows\\System32\\whoami.exe')
  })
})
