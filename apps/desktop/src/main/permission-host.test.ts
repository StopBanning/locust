import { existsSync, readFileSync } from 'node:fs'
import { request } from 'node:http'
import { dirname } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import type { MissionApprovalRequest } from '../shared/ipc.js'
import { bridgeConfig, connectorSummary, createPermissionHost, serverPrefixOf } from './permission-host.js'
import type { BridgeAnswer, PermissionHost } from './permission-host.js'

/**
 * Locust answers Claude Code's permission questions, and only Locust's own
 * runs can ask.
 *
 * These go over a real loopback socket rather than calling the handler,
 * because the thing that can go wrong is the wire: a field misnamed in the
 * config, a token that is not checked before the body is read, a request
 * that never resolves when the run ends. Each of those is a connector call
 * silently refused, which is exactly how 0.60.1 failed.
 */

const hosts: PermissionHost[] = []
afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.dispose()))
})

function post(port: number, body: unknown): Promise<BridgeAnswer> {
  return new Promise((resolve, reject) => {
    const text = JSON.stringify(body)
    const req = request(
      { host: '127.0.0.1', port, path: '/approve', method: 'POST', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(text) } },
      (res) => {
        const chunks: Buffer[] = []
        res.on('data', (chunk: Buffer) => chunks.push(chunk))
        res.on('end', () => resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')) as BridgeAnswer))
      }
    )
    req.on('error', reject)
    req.end(text)
  })
}

async function hostWithCards(): Promise<{ host: PermissionHost; cards: MissionApprovalRequest[] }> {
  const cards: MissionApprovalRequest[] = []
  let n = 0
  const host = createPermissionHost({
    bridgePath: 'C:/locust/resources/locust-permission-bridge.mjs',
    node: 'node',
    emitApproval: (card) => cards.push(card),
    createId: () => `id_${String(++n)}`
  })
  hosts.push(host)
  await host.start()
  return { host, cards }
}

/** The token a registered run was given, read back out of its own config. */
function tokenOf(configPath: string): string {
  const parsed = JSON.parse(readFileSync(configPath, 'utf8')) as { mcpServers: { locust: { env: { LOCUST_PERMISSION_TOKEN: string } } } }
  return parsed.mcpServers.locust.env.LOCUST_PERMISSION_TOKEN
}

describe('the permission host', () => {
  it('refuses a Claude approval when its pending text cannot be written', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run_stream', missionId: 'mission_stream', cwd: 'C:/work',
      beforeApproval: async () => { throw new Error('disk full') } })
    const answer = await post(host.port, { token: tokenOf(configPath), toolName: 'Read', input: {} })
    expect(answer.behavior).toBe('deny')
    expect(cards).toEqual([])
  })

  it('waits for pending text before raising a Claude approval and refuses one whose run ended while waiting', async () => {
    const { host, cards } = await hostWithCards()
    let release!: () => void
    let entered = false
    const flushing = new Promise<void>((resolve) => { release = resolve })
    const { configPath } = await host.register({ runId: 'run_stream', missionId: 'mission_stream', cwd: 'C:/work', beforeApproval: async () => {
      entered = true
      await flushing
    } })
    const asked = post(host.port, { token: tokenOf(configPath), toolName: 'Read', input: { file_path: 'notes.md' } })
    await vi.waitFor(() => { expect(entered).toBe(true) })
    expect(cards).toEqual([])
    await host.release('run_stream')
    release()
    expect((await asked).behavior).toBe('deny')
    expect(cards).toEqual([])
  })

  it('takes a card down when the bridge stops waiting for it, so the question asked again is the one card (0.837)', async () => {
    const withdrawn: string[] = []
    const cards: MissionApprovalRequest[] = []
    const host = createPermissionHost({ bridgePath: 'C:/locust/resources/locust-permission-bridge.mjs', node: 'node', emitApproval: (card) => cards.push(card), withdrawApproval: (id) => withdrawn.push(id), createId: () => `gone_${String(cards.length + withdrawn.length)}` })
    hosts.push(host)
    await host.start()
    const { configPath } = await host.register({ runId: 'run_late', missionId: 'm_late', cwd: 'C:/work' })
    const text = JSON.stringify({ token: tokenOf(configPath), toolName: 'Write', input: { file_path: 'C:/work/a.txt', content: 'OK' } })
    // The bridge's own patience runs out: it lets go of the request without an answer.
    const req = request({ host: '127.0.0.1', port: host.port, path: '/approve', method: 'POST', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(text) } })
    req.on('error', () => undefined)
    req.end(text)
    await vi.waitFor(() => { expect(cards).toHaveLength(1) })
    req.destroy()
    await vi.waitFor(() => { expect(withdrawn).toEqual([cards[0]!.approvalId]) })
    // Pressing the old card now answers nothing.
    expect(host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-once' })).toBe(false)
  })

  it('leaves an answered card alone when its connection closes after the answer', async () => {
    const withdrawn: string[] = []
    const cards: MissionApprovalRequest[] = []
    const host = createPermissionHost({ bridgePath: 'C:/locust/resources/locust-permission-bridge.mjs', node: 'node', emitApproval: (card) => cards.push(card), withdrawApproval: (id) => withdrawn.push(id) })
    hosts.push(host)
    await host.start()
    const { configPath } = await host.register({ runId: 'run_ok', missionId: 'm_ok', cwd: 'C:/work' })
    const asked = post(host.port, { token: tokenOf(configPath), toolName: 'Bash', input: { command: 'npm test' } })
    await vi.waitFor(() => { expect(cards).toHaveLength(1) })
    expect(host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-once' })).toBe(true)
    expect((await asked).behavior).toBe('allow')
    await new Promise((r) => setTimeout(r, 50))
    expect(withdrawn).toEqual([])
  })

  it('refuses a request that carries no token of ours, before reading anything else', async () => {
    const { host, cards } = await hostWithCards()
    const answer = await post(host.port, { token: 'not-ours', toolName: 'mcp__claude_ai_Robinhood__place_equity_order' })
    expect(answer.behavior).toBe('deny')
    // And no card was raised: a stranger cannot make the window ask.
    expect(cards).toEqual([])
  })

  it('turns a question into the card the app already has, and answers with the decision', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath, toolName } = await host.register({ runId: 'run1', missionId: 'm1', cwd: 'C:/work' })
    expect(toolName).toBe('mcp__locust__approve')
    const token = tokenOf(configPath)

    const asked = post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_watchlists', input: { limit: 5 } })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards).toHaveLength(1)
    expect(cards[0]).toMatchObject({ runId: 'run1', missionId: 'm1', kind: 'connector', summary: 'Use get_watchlists on Robinhood', cwd: 'C:/work' })
    expect(cards[0]!.detail).toContain('"limit": 5')

    expect(host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-once' })).toBe(true)
    const answer = await asked
    expect(answer).toEqual({ behavior: 'allow', updatedInput: { limit: 5 } })
  })

  it("names the call that asks, so a helper's card can name the helper (helper visibility)", async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: 'C:/work' })
    const token = tokenOf(configPath)
    const asked = post(host.port, { token, toolName: 'Bash', input: { command: 'ls' }, toolUseId: 'toolu_0134dcHNk1GH8kL57hFJ7THL' })
    const odd = post(host.port, { token, toolName: 'Bash', input: { command: 'pwd' }, toolUseId: 'not an id; rm -rf' })
    await new Promise((r) => setTimeout(r, 30))
    const card = (command: string): MissionApprovalRequest | undefined => cards.find((each) => each.detail.includes(command))
    expect(card('ls')?.toolUseId).toBe('toolu_0134dcHNk1GH8kL57hFJ7THL')
    // Anything that is not an id is not carried.
    expect(card('pwd')).toBeDefined()
    expect('toolUseId' in card('pwd')!).toBe(false)
    cards.forEach((card) => host.decide({ approvalId: card.approvalId, decision: 'deny' }))
    await Promise.all([asked, odd])
  })

  it('carries a denial to the model with a sentence, not a bare refusal', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: null })
    const asked = post(host.port, { token: tokenOf(configPath), toolName: 'mcp__claude_ai_Gmail__send_message' })
    await new Promise((r) => setTimeout(r, 30))
    host.decide({ approvalId: cards[0]!.approvalId, decision: 'deny' })
    const answer = await asked
    expect(answer.behavior).toBe('deny')
    expect(answer.behavior === 'deny' && answer.message.length > 0).toBe(true)
  })

  it('carries the person’s reason for a denial, when they gave one (0.374)', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: null })
    const asked = post(host.port, { token: tokenOf(configPath), toolName: 'mcp__claude_ai_Gmail__send_message' })
    await new Promise((r) => setTimeout(r, 30))
    host.decide({ approvalId: cards[0]!.approvalId, decision: 'deny', reason: 'draft it instead of sending' })
    const answer = await asked
    expect(answer).toEqual({ behavior: 'deny', message: 'Denied in Locust. The person declined this, and said: draft it instead of sending' })
  })

  // QA-2026-09-29 round 2, R35: Always is the TOOL, not the connector. It was
  // the connector, and "list_issues" let "delete_file" through with no card.
  // Since 0.616 the host remembers nothing (shared/who-decides.ts): it used to
  // answer a tool it had been told "always" about before the saved rules were
  // read. Every request is raised, with the tool as the key an Always on it
  // covers, and the main process decides the later ones after the rules.
  it('raises every request, even after Always, each with its own tool as the key an Always would cover', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: null })
    const token = tokenOf(configPath)

    const first = post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards[0]).toMatchObject({ alwaysCovers: 'get_watchlists on Robinhood again, and no other tool', alwaysKey: 'claude:mcp__claude_ai_Robinhood__get_watchlists' })
    host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-always' })
    expect((await first).behavior).toBe('allow')

    // The same tool again: raised, never answered by the host itself.
    const again = post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards).toHaveLength(2)
    expect(cards[1]!.alwaysKey).toBe(cards[0]!.alwaysKey)
    host.decide({ approvalId: cards[1]!.approvalId, decision: 'deny' })
    expect((await again).behavior).toBe('deny')

    // Same connector, a different tool: a key of its own.
    const other = post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_accounts' })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards[2]!.alwaysKey).toBe('claude:mcp__claude_ai_Robinhood__get_accounts')
    host.decide({ approvalId: cards[2]!.approvalId, decision: 'deny' })
    expect((await other).behavior).toBe('deny')

    // Bash: one key for every command, as its card says.
    const command = post(host.port, { token, toolName: 'Bash', input: { command: 'npm test' } })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards[3]).toMatchObject({ alwaysKey: 'claude:Bash', alwaysCovers: 'every command it runs' })
    host.decide({ approvalId: cards[3]!.approvalId, decision: 'deny' })
    expect((await command).behavior).toBe('deny')
  })

  it('does not let one run answer for another', async () => {
    const { host, cards } = await hostWithCards()
    const a = await host.register({ runId: 'runA', missionId: 'mA', cwd: null })
    const b = await host.register({ runId: 'runB', missionId: 'mB', cwd: null })
    const askedA = post(host.port, { token: tokenOf(a.configPath), toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    await new Promise((r) => setTimeout(r, 30))
    host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-always' })
    await askedA
    // B asks the same connector: its own run never said always.
    const askedB = post(host.port, { token: tokenOf(b.configPath), toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards).toHaveLength(2)
    host.decide({ approvalId: cards[1]!.approvalId, decision: 'deny' })
    expect((await askedB).behavior).toBe('deny')
  })

  it('refuses whatever is still waiting when the run ends, and removes its config', async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: null })
    const token = tokenOf(configPath)
    expect(existsSync(configPath)).toBe(true)
    const asked = post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    await new Promise((r) => setTimeout(r, 30))
    expect(cards).toHaveLength(1)
    await host.release('run1')
    const answer = await asked
    expect(answer.behavior).toBe('deny')
    expect(existsSync(dirname(configPath))).toBe(false)
    // The token died with the run.
    const after = await post(host.port, { token, toolName: 'mcp__claude_ai_Robinhood__get_watchlists' })
    expect(after.behavior).toBe('deny')
    expect(host.decide({ approvalId: cards[0]!.approvalId, decision: 'approve-once' })).toBe(false)
  })

  it("refuses a question's answer aimed at a permission, and says so", async () => {
    const { host, cards } = await hostWithCards()
    const { configPath } = await host.register({ runId: 'run1', missionId: 'm1', cwd: null })
    const asked = post(host.port, { token: tokenOf(configPath), toolName: 'mcp__x__y' })
    await new Promise((r) => setTimeout(r, 30))
    host.decide({ approvalId: cards[0]!.approvalId, answers: { q1: ['yes'] } })
    const answer = await asked
    expect(answer.behavior).toBe('deny')
    expect(answer.behavior === 'deny' && answer.message).toContain('belongs to a question')
  })
})

describe('what the CLI is handed', () => {
  it('is an mcp.json that names the bridge, the loopback URL and the token, under the name the tool flag expects', () => {
    const text = bridgeConfig({ bridgePath: 'C:/r/bridge.mjs', url: 'http://127.0.0.1:4321/approve', token: 'tok' , node: 'node' })
    const parsed = JSON.parse(text) as { mcpServers: Record<string, { command: string; args: string[]; env: Record<string, string> }> }
    // `mcp__locust__approve` is `mcp__<server>__<tool>`: the server MUST be
    // called `locust` or the flag names a tool that does not exist.
    expect(Object.keys(parsed.mcpServers)).toEqual(['locust'])
    expect(parsed.mcpServers.locust).toEqual({
      command: 'node',
      args: ['C:/r/bridge.mjs'],
      env: { ELECTRON_RUN_AS_NODE: '1', LOCUST_PERMISSION_URL: 'http://127.0.0.1:4321/approve', LOCUST_PERMISSION_TOKEN: 'tok' }
    })
  })

  it('names a connector the way the activity rows do', () => {
    expect(connectorSummary('mcp__claude_ai_Robinhood__get_watchlists')).toBe('Use get_watchlists on Robinhood')
    expect(connectorSummary('mcp__linear__create_issue')).toBe('Use create_issue on linear')
    expect(serverPrefixOf('mcp__claude_ai_Robinhood__get_watchlists')).toBe('mcp__claude_ai_Robinhood__')
    expect(serverPrefixOf('Bash')).toBe('Bash')
  })
})

/*
 * A B4 lead from the code review, settled: two Claude runs registering at
 * once both found no server and each started one; the first listener stayed
 * open on loopback, and dispose closed only the second.
 */
describe('two runs registering at once', () => {
  it('share one listener, and both are answered on it', async () => {
    let n = 0
    const host = createPermissionHost({
      bridgePath: 'C:/locust/resources/locust-permission-bridge.mjs',
      node: 'node',
      emitApproval: () => undefined,
      createId: () => `id_${String(++n)}`
    })
    hosts.push(host)
    const [a, b] = await Promise.all([
      host.register({ runId: 'run_a', missionId: 'm_a', cwd: null }),
      host.register({ runId: 'run_b', missionId: 'm_b', cwd: null })
    ])
    const urlOf = (configPath: string) =>
      (JSON.parse(readFileSync(configPath, 'utf8')) as { mcpServers: { locust: { env: { LOCUST_PERMISSION_URL: string } } } }).mcpServers.locust.env.LOCUST_PERMISSION_URL
    expect(urlOf(a.configPath)).toBe(urlOf(b.configPath))
    // And nothing is left listening once it is closed.
    const listening = () => process.getActiveResourcesInfo().filter((kind) => kind === 'TCPServerWrap').length
    await host.dispose()
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(listening()).toBe(0)
  })
})
