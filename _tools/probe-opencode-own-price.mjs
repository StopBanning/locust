// Whether a run of a model of your own reads its listed price.
//
//   node _tools/probe-opencode-own-price.mjs [--opencode-only]
//
// A model the person adds at its provider's own address is priced from the
// list models.dev keeps for it (listed-prices.ts, 0.810). This starts a local
// OpenAI-compatible endpoint that reports fixed token counts, declares it
// through Locust's OWN builder with Claude Haiku 5.5's listed price -- a tier
// included, dearer above 100k of context -- and runs Locust's OpenCode
// command, process runner and normalizer (given the price, as a run of your
// own model is) against it twice: once under the tier, once over it. Each
// run's dollars must be the price times the tokens the endpoint reported.
// With --opencode-only the normalizer is not given the price, and OpenCode's
// own figure is read: MEASURED 2026-10-21 on 1.18.27, it keeps the base of a
// price its config gives and drops the tier, so the run over it reads $0.01505
// for $0.07525. Build the adapters first. Spends nothing.

import { mkdtempSync, rmSync } from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { execFileSync } from 'node:child_process'

const openCodeOnly = process.argv.includes('--opencode-only')
const PRICE = { input: 0.1, output: 0.5, cache_read: 0.01, cache_write: 0.125, tiers: [{ input: 0.5, output: 2.5, cache_read: 0.05, cache_write: 0.625, tier: { type: 'context', size: 100000 } }] }
let reported = { prompt: 1000, completion: 100 }
const server = createServer((request, response) => {
  request.resume()
  request.on('end', () => {
    response.writeHead(200, { 'content-type': 'text/event-stream' })
    const chunk = (data) => response.write(`data: ${JSON.stringify(data)}\n\n`)
    const base = { id: 'c1', object: 'chat.completion.chunk', created: 1, model: 'claude-haiku-5-5' }
    chunk({ ...base, choices: [{ index: 0, delta: { role: 'assistant', content: 'pong' }, finish_reason: null }] })
    chunk({ ...base, choices: [{ index: 0, delta: {}, finish_reason: 'stop' }], usage: { prompt_tokens: reported.prompt, completion_tokens: reported.completion, total_tokens: reported.prompt + reported.completion } })
    response.end('data: [DONE]\n\n')
  })
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const port = server.address().port

const lib = await import(new URL('../packages/runtime-adapters/dist/index.js', import.meta.url).href)
// The npm install puts a shim on PATH and the real binary here.
const installed = join(process.env.APPDATA ?? '', 'npm', 'node_modules', 'opencode-ai', 'bin', 'opencode.exe')
let where = installed
try {
  where = execFileSync('where', ['opencode.exe'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).split(/\r?\n/).find((line) => line.trim().length > 0) ?? installed
} catch {
  // Not on PATH as an .exe: the npm install's copy.
}

let failures = 0
for (const [prompt, completion, price] of [[1000, 100, PRICE], [150000, 100, PRICE.tiers[0]]]) {
  reported = { prompt, completion }
  const workspace = mkdtempSync(join(tmpdir(), 'locust-own-price-ws-'))
  const spec = lib.createOpenCodeRunCommand(
    { executablePath: where.trim(), prefixArgs: [] },
    {
      workspacePath: workspace,
      prompt: 'say pong',
      model: 'own-0a1b2c3d/claude-haiku-5-5',
      sandbox: 'read-only',
      providers: { 'own-0a1b2c3d': { name: 'claude-haiku-5-5', baseUrl: `http://127.0.0.1:${String(port)}/v1`, apiKey: 'x', models: ['claude-haiku-5-5'], prices: { 'claude-haiku-5-5': PRICE } } }
    }
  )
  const run = lib.createNodeRuntimeProcessRunner().start(spec, 'say pong')
  const normalizer = lib.createOpenCodeEventNormalizer({ runId: 'run_1', missionId: 'mission_1', cliVersion: 'probe', ...(openCodeOnly ? {} : { price: PRICE }) })
  const events = []
  for await (const record of run.records) events.push(...normalizer.accept(record))
  events.push(...normalizer.finish(await run.completion))
  const done = events.find((event) => event.type === 'run.completed')
  const usage = done?.payload?.usage
  const expected = (prompt * price.input + completion * price.output) / 1_000_000
  const ok = typeof usage?.usd === 'number' && Math.abs(usage.usd - expected) < 1e-9
  if (!ok) failures += 1
  console.log(`[${ok ? 'PASS' : 'FAIL'}] ${String(prompt)} in, ${String(completion)} out: the run read ${JSON.stringify(usage)}; the listed price makes it $${expected.toFixed(6)}${done === undefined ? ` (no run.completed: ${events.map((event) => event.type).join(', ')})` : ''}`)
  rmSync(workspace, { recursive: true, force: true })
}
server.close()
process.exit(failures === 0 ? 0 : 1)
