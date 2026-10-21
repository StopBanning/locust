// OpenCode 2 over ACP: print every tool_call and tool_call_update (kind, title, status, rawInput, locations).
//
//   node _tools/probe-opencode-2-tool-rows.mjs
//
// Uses the OpenCode 2 binary and home the handoff names. Allow-once on every permission. Free model only.

import { spawn } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { createInterface } from 'node:readline'

const bin = process.env.LOCUST_OPENCODE2_BIN
  ?? 'C:\\work\\scratch\\opencode2\\node_modules\\@opencode\\cli-windows-x64\\bin\\opencode.exe'
const home = process.env.LOCUST_OPENCODE2_HOME ?? 'C:\\work\\scratch\\opencode2\\home'
const model = 'opencode/deepseek-v4.1-flash-free'
const scratch = 'C:\\work\\scratch\\oc2-tool-rows-probe'
mkdirSync(scratch, { recursive: true })
writeFileSync(join(scratch, 'note.txt'), 'probe-note\n', 'utf8')

const child = spawn(bin, ['acp'], {
  cwd: scratch,
  stdio: ['pipe', 'pipe', 'pipe'],
  env: {
    ...process.env,
    XDG_CONFIG_HOME: join(home, 'config'),
    XDG_DATA_HOME: join(home, 'data'),
    XDG_CACHE_HOME: join(home, 'cache'),
    XDG_STATE_HOME: join(home, 'state'),
    OPENCODE_DISABLE_AUTOUPDATE: '1'
  }
})

let nextId = 1
const pending = new Map()
const tools = []

const send = (method, params) => {
  const id = nextId
  nextId += 1
  const line = JSON.stringify({ jsonrpc: '2.0', id, method, params })
  child.stdin.write(`${line}\n`)
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
  })
}

const allowOnce = (request) => {
  const options = Array.isArray(request.params?.options) ? request.params.options : []
  const pick = options.find((option) => option.kind === 'allow_once')
    ?? options.find((option) => option.kind === 'once' || option.optionId === 'once')
    ?? options.find((option) => /allow once/i.test(String(option.name ?? '')))
  const result = pick === undefined
    ? { outcome: { outcome: 'cancelled' } }
    : { outcome: { outcome: 'selected', optionId: pick.optionId } }
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result })}\n`)
  process.stdout.write(`permission ${JSON.stringify({ kinds: options.map((option) => option.kind), picked: pick?.optionId ?? null })}\n`)
}

const lines = createInterface({ input: child.stdout })
lines.on('line', (line) => {
  let message
  try {
    message = JSON.parse(line)
  } catch {
    process.stdout.write(`non-json ${line.slice(0, 200)}\n`)
    return
  }
  if (message.id !== undefined && message.method === undefined) {
    const waiter = pending.get(message.id)
    if (waiter !== undefined) {
      pending.delete(message.id)
      if (message.error !== undefined) waiter.reject(new Error(JSON.stringify(message.error)))
      else waiter.resolve(message.result)
    }
    return
  }
  if (message.method === 'session/request_permission') {
    allowOnce(message)
    return
  }
  if (message.method !== 'session/update') return
  const update = message.params?.update ?? {}
  if (update.sessionUpdate !== 'tool_call' && update.sessionUpdate !== 'tool_call_update') return
  const row = {
    sessionUpdate: update.sessionUpdate,
    kind: update.kind ?? null,
    title: update.title ?? null,
    status: update.status ?? null,
    rawInput: update.rawInput ?? null,
    locations: update.locations ?? null
  }
  tools.push(row)
  process.stdout.write(`${JSON.stringify(row)}\n`)
})

child.stderr.on('data', (chunk) => {
  process.stderr.write(chunk)
})

const fail = (error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
  child.kill()
  process.exit(1)
}

const timer = setTimeout(() => fail('probe timed out'), 180_000)

try {
  const init = await send('initialize', {
    protocolVersion: 1,
    clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false }
  })
  process.stdout.write(`initialize protocol ${String(init?.protocolVersion)}\n`)
  const session = await send('session/new', { cwd: scratch, mcpServers: [] })
  const sessionId = session?.sessionId
  if (typeof sessionId !== 'string' || sessionId.length === 0) fail('no session')
  const set = await send('session/set_config_option', { sessionId, configId: 'model', value: model })
  const modelOption = (Array.isArray(set?.configOptions) ? set.configOptions : []).find((option) => option.id === 'model')
  process.stdout.write(`model ${JSON.stringify(modelOption?.currentValue ?? null)}\n`)
  const ended = await send('session/prompt', {
    sessionId,
    prompt: [{ type: 'text', text: 'Read the file note.txt in this folder. Then run exactly this shell command: echo hi. Then reply with the single word: done.' }]
  })
  process.stdout.write(`stop ${JSON.stringify(ended?.stopReason ?? null)} tools ${String(tools.length)}\n`)
} catch (error) {
  fail(error)
} finally {
  clearTimeout(timer)
  child.stdin.end()
  child.kill()
}
