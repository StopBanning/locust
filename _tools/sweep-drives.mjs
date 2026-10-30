// Every drive that runs on a packaged build for free, one after another (C6).
//
//   node _tools/sweep-drives.mjs --packaged <exe> --out <dir> [--only a,b] [--from name]
//
// The plan's pre-launch check: "Re-run the whole drive set on a packaged
// build before wave 1". A drive is in the sweep when it takes `--packaged`
// and never asks for a paid account (`spends: true`) -- Colin: free or cheap
// models for my own testing. Each runs alone (ports and the machine's
// attention are shared), on the free model, with a time limit; its record
// goes under <out>/captures (LOCUST_DRIVE_OUT), never into the repository.
//
// What a drive says is read, not judged: its exit code, every line with
// PASS or FAIL in it, and any "failed:" line. The summary is rewritten after
// every drive, so a sweep that is stopped still says what it found.

import { spawn, execFile } from 'node:child_process'
import { readdirSync, readFileSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, FREE_ROUTE } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const out = arg('--out')
if (packaged === undefined || out === undefined) {
  console.error('usage: node _tools/sweep-drives.mjs --packaged <exe> --out <dir> [--only a,b] [--from name]')
  process.exit(2)
}
const only = arg('--only')?.split(',').map((name) => name.trim())
const from = arg('--from')
/*
 * THE FREE MODELS, IN TURN (2026-10-06; list refreshed 2026-10-18, and on
 * 2026-10-22 without Space Bunny and LongCat, gone from OpenCode). One free
 * model for a whole sweep is a provider asked hundreds of times in a row:
 * Ling 3.0 went "Endpoint is unavailable" and Muse Spark "Rate limit
 * exceeded" under it, and every drive after failed for the provider's
 * reason, not Locust's. Each drive takes the next of the free models found
 * answering, so no one provider carries the sweep.
 *
 * The app takes its free models in an order kept from this list since 0.712
 * (shared/free-models.ts FREE_MODELS_BEST_FIRST: a new person's first message
 * goes to the first of them). A model dropped here comes out of that too.
 * The 2026-10-18 list is that file's list, in its order: read fresh from
 * OpenCode Zen, then one short question asked of each in a throwaway home,
 * three tries each (see that file for answers and times). jev-1.13-free is
 * listed by OpenCode but answered nothing, so it is in neither list.
 */
const FREE_MODELS = [
  'opencode/deepseek-v4.1-flash-free',
  'opencode/nemotron-3.5-lightning-free',
  'opencode/nemotron-3.5-super-vl-free',
  'opencode/glm-5.3-flash-free',
  'opencode/mimo-v2.6-flash-free',
  'opencode/beam-free'
]
let freeTurn = 0
const nextFreeModel = () => FREE_MODELS[freeTurn++ % FREE_MODELS.length]
const LIMIT_MS = Number(arg('--limit-min') ?? '20') * 60_000

/**
 * Drives a sweep cannot run as they are, and why -- said in the summary, not
 * dropped (the 0.367 sweep ran both and counted a usage error as a failure).
 */
const SKIP = {
  'drive-context-menu': 'needs --reuse <profile>: it opens a profile another drive made',
  'drive-update-lane': 'needs an OLDER build and --expect-latest / --expect-every: run by hand after a release',
  'drive-turns-read-in-order': 'needs --ledger <folder>: it copies whole conversations, never from the person’s own profile',
  'drive-a-pull-request-from-the-conversation': 'needs --repo <a clone of a throwaway GitHub repository>',
  'drive-cloud-from-a-folder-not-on-github': 'needs --repo <clone>',
  'drive-cloud-without-an-environment': 'needs --repo <a clone with an environment>'
}

/**
 * What one drive needs to reach the thing it tests. The handoff drive picks
 * Cursor while a free run goes on; a drive's window refuses every paid route
 * first unless LOCUST_SPEND=1, so without it the drive guard's refusal is all
 * it ever sees. Cursor is refused before it starts either way: nothing is spent.
 */
const ENV = {
  'drive-handoff-refused': { LOCUST_SPEND: '1' }
}

const tools = localPath(new URL('./', import.meta.url))
const drives = readdirSync(tools)
  .filter((name) => /^drive-.*\.mjs$/.test(name) && name !== 'drive-lib.mjs')
  .filter((name) => {
    const text = readFileSync(join(tools, name), 'utf8')
    return text.includes("'--packaged'") && !/spends:\s*true/.test(text)
  })
  .filter((name) => only === undefined || only.includes(name.replace(/\.mjs$/, '')))
  .sort()
const start = from === undefined ? 0 : Math.max(0, drives.findIndex((name) => name.replace(/\.mjs$/, '') === from))

await mkdir(join(out, 'captures'), { recursive: true })
await mkdir(join(out, 'logs'), { recursive: true })
const results = []

/** The whole tree this sweep started, and nothing else: a drive's own app included. */
const killTree = (pid) => new Promise((resolve) => execFile('taskkill', ['/PID', String(pid), '/T', '/F'], { windowsHide: true }, () => resolve()))

async function summarise() {
  const lines = results.map((result) => {
    const verdict = result.skipped === true ? 'SKIPPED' : result.timedOut ? 'TIMED OUT' : result.fails > 0 || result.failedLines.length > 0 ? 'FAIL' : result.code !== 0 ? `EXIT ${String(result.code)}` : result.passes > 0 ? 'PASS' : 'RAN'
    return `| ${result.name} | ${verdict} | ${String(result.passes)} | ${String(result.fails)} | ${Math.round(result.ms / 1000)}s | ${result.note.replace(/\|/g, '/').slice(0, 160)} |`
  })
  const md = [
    `# Drive sweep on ${packaged}`,
    '',
    `${String(results.length)} of ${String(drives.length - start)} run.`,
    '',
    '| drive | result | PASS lines | FAIL lines | took | note |',
    '| --- | --- | --- | --- | --- | --- |',
    ...lines,
    ''
  ].join('\n')
  await writeFile(join(out, 'SUMMARY.md'), md, 'utf8')
  await writeFile(join(out, 'summary.json'), JSON.stringify(results, null, 2), 'utf8')
}

const RETRY_FIRST = FREE_ROUTE.model
const PAID_REFUSAL = /refusing to run "[^"]+": it spends a paid account/
/** A drive that reaches a site outside GitHub only when told to (drive-pet-picks: LOCUST_NETWORK=1). */
const NETWORK_REFUSAL = /Re-run with LOCUST_NETWORK=1 if you mean it/
const PROVIDER_REFUSED = /Rate limit exceeded|rate[- ]limited|Endpoint is unavailable|provider answered "(?:Too Many Requests|Service Unavailable)/i
/*
 * A drive's own verdict: `[PASS]`/`[FAIL]` on a check, or `PASS:`/`FAIL:` starting a line. Not the word anywhere:
 * the 0.817 sweep counted 2 failures in a drive that passed every check, because the check it planted printed
 * "FAIL report total" and the drive quoted it.
 */
const PASS_LINE = /\[PASS\]|^\s*PASS:/
const FAIL_LINE = /\[FAIL\]|^\s*FAIL:/
async function runDrive(name, model, attempt = 0) {
  const child = spawn(process.execPath, [join(tools, name), '--packaged', packaged], {
    cwd: localPath(new URL('../', import.meta.url)),
    // The free model is the drives' own (drive-lib FREE_ROUTE) unless LOCUST_FREE_MODEL says otherwise. This forced
    // Ling 3.0, and on 2026-10-06 Ling 3.0's provider was down ("Endpoint is unavailable", then rate limits): 41 of
    // the first 83 drives failed on it while Muse Spark answered.
    env: { ...process.env, LOCUST_DRIVE_OUT: join(out, 'captures'), LOCUST_FREE_MODEL: model, ...(ENV[name.replace(/\.mjs$/, '')] ?? {}) },
    windowsHide: true
  })
  let text = ''
  child.stdout.on('data', (chunk) => { text += String(chunk) })
  child.stderr.on('data', (chunk) => { text += String(chunk) })
  let timedOut = false
  const timer = setTimeout(() => {
    timedOut = true
    void killTree(child.pid)
  }, LIMIT_MS)
  const code = await new Promise((resolve) => child.on('close', (value) => resolve(value ?? -1)))
  clearTimeout(timer)
  // Each run its own log: a retry wrote over the first, and the 0.709 sweep's first failures were lost behind
  // retries on a model that was no longer there.
  await writeFile(join(out, 'logs', name.replace(/\.mjs$/, attempt === 0 ? '.log' : `.again-${String(attempt)}.log`)), text, 'utf8')
  const lines = text.split(/\r?\n/)
  return {
    text,
    code,
    timedOut,
    passes: lines.filter((line) => PASS_LINE.test(line)).length,
    fails: lines.filter((line) => FAIL_LINE.test(line)).length,
    failedLines: lines.filter((line) => /(drive|probe) failed:|Error:|TypeError|ReferenceError/.test(line))
  }
}

for (const name of drives.slice(start)) {
  const began = Date.now()
  const skipped = SKIP[name.replace(/\.mjs$/, '')]
  if (skipped !== undefined) {
    results.push({ name: name.replace(/\.mjs$/, ''), code: 0, timedOut: false, skipped: true, passes: 0, fails: 0, failedLines: [], ms: 0, note: `skipped: ${skipped}` })
    await summarise()
    console.log(`${name}: skipped, ${skipped}`)
    continue
  }
  /*
   * A drive that failed because its free model's provider refused (a rate
   * limit, an endpoint down) says nothing about Locust: on 2026-10-06 four
   * drives failed that way, each on whichever model the rotation had reached.
   * Such a drive runs again on the next free model, at most twice, and the
   * summary says which models were refused. A model forced with
   * LOCUST_FREE_MODEL is never swapped.
   *
   * And ANY failure gets one more run on another model: a free model that is
   * only slow says nothing a drive can read -- Space Bunny left a routine on
   * step 1 past the drive's wait, and the same drive passed 6/6 on Fledge
   * Alpha (2026-10-06). A failure on two models is the one to look at.
   */
  const refusedBy = []
  const failedOn = []
  let run
  for (let attempt = 0; attempt < 3; attempt += 1) {
    // A first run takes the rotation; a run again takes the steadiest free model first (Fledge Alpha answered every
    // drive asked again on it, 2026-10-06; it left OpenCode 2026-10-08, so Muse Spark), so a retry tests Locust rather
    // than another model's bad hour.
    const tried = [...refusedBy, ...failedOn].map((slug) => `opencode/${slug}`)
    const model = process.env.LOCUST_FREE_MODEL
      ?? (attempt > 0 && !tried.includes(RETRY_FIRST) ? RETRY_FIRST : nextFreeModel())
    run = await runDrive(name, model, attempt)
    if (run.fails === 0 && run.code === 0) break
    if (process.env.LOCUST_FREE_MODEL !== undefined) break
    // A drive that spends only on some routes (`spends: runtime !== 'opencode'`) passes the filter above and then
    // refuses, correctly, without LOCUST_SPEND: that is a skip, not a failure to run again.
    if (PAID_REFUSAL.test(run.text) || NETWORK_REFUSAL.test(run.text)) break
    const refused = PROVIDER_REFUSED.test(run.text)
    if (!refused && failedOn.length > 0) break
    ;(refused ? refusedBy : failedOn).push(model.replace(/^opencode\//, ''))
    console.log(`${name}: failed on ${model}${refused ? ' (refused by its provider)' : ''}; again on the next free model`)
  }
  const { code, timedOut, passes, fails, failedLines } = run
  if (PAID_REFUSAL.test(run.text)) {
    results.push({ name: name.replace(/\.mjs$/, ''), code: 0, timedOut: false, skipped: true, passes: 0, fails: 0, failedLines: [], ms: Date.now() - began, note: 'skipped: it spends a paid account on its default route' })
    await summarise()
    console.log(`${name}: skipped, it spends a paid account on its default route`)
    continue
  }
  if (NETWORK_REFUSAL.test(run.text)) {
    results.push({ name: name.replace(/\.mjs$/, ''), code: 0, timedOut: false, skipped: true, passes: 0, fails: 0, failedLines: [], ms: Date.now() - began, note: 'skipped: it reaches another site only with LOCUST_NETWORK=1' })
    await summarise()
    console.log(`${name}: skipped, it reaches another site only with LOCUST_NETWORK=1`)
    continue
  }
  const passedLater = fails === 0 && code === 0 && (refusedBy.length > 0 || failedOn.length > 0)
  const note = (refusedBy.length > 0 ? `refused by ${refusedBy.join(', ')}; ` : '')
    + (failedOn.length > 0 ? `${passedLater ? 'passed after failing' : 'also failed'} on ${failedOn.join(', ')}; ` : '')
    + (failedLines[0] ?? run.text.split(/\r?\n/).filter((line) => FAIL_LINE.test(line))[0] ?? '')
  results.push({ name: name.replace(/\.mjs$/, ''), code, timedOut, passes, fails, failedLines: failedLines.slice(0, 5), ms: Date.now() - began, note })
  await summarise()
  console.log(`${name}: code ${String(code)}${timedOut ? ' TIMED OUT' : ''}, ${String(passes)} PASS, ${String(fails)} FAIL ${note.slice(0, 120)}`)
}
console.log('SWEEP DONE')
