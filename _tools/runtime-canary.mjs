// The runtime canary: a new CLI release, through one real turn, before Locust installs it.
//
//   LOCUST_SPEND=1 node _tools/runtime-canary.mjs --packaged <Locust.exe> [--runtime codex|copilot] [--version <v>] [--force]
//
// Locust updates the coding agents npm installed (runtime-updates.ts: Codex
// and Copilot) on its own, twelve hours after a release goes out. Twelve hours
// keeps a pulled release away; it does not keep away one that changed its
// event stream, and every Locust reads that stream. A release that renamed an
// event would reach every tester the same morning, and the first word of it
// would be theirs.
//
// So, for each agent: ask npm for its newest release. If this machine does not
// have it yet (or --force), install that release into a folder of its own
// (never the global one; the person's agent stays what it was), and run the
// packaged app with that folder first on its PATH through one real turn --
// drive-a-turn-reads-as-it-happens, which checks the steps read as lines
// while it works and the finished turn reads the same. The mission's own
// record must name the new version, or the turn proved nothing about it.
//
// Writes docs/runtime-canary/<runtime>-<version>.json: the verdict and why.
// Spends one turn on the agent's account default. Nothing is published and
// nothing on the machine is updated: that is still runtime-updates.ts's job.

import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { execFileSync as run } from 'node:child_process'
import { localPath } from './local-path.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const runtime = arg('--runtime') ?? 'codex'
const force = process.argv.includes('--force')
const ROOT = localPath(new URL('..', import.meta.url))
// Beside the checkout, never inside it: the scratch folder the drives already use.
const SCRATCH = process.env.LOCUST_SCRATCH ?? join(ROOT, '..', '.scratch')
// The agents Locust keeps current itself, by their npm package and command.
const AGENTS = { codex: { pkg: '@openai/codex', command: 'codex' }, copilot: { pkg: '@github/copilot', command: 'copilot' } }
const agent = AGENTS[runtime]
if (packaged === undefined || agent === undefined) {
  console.log('usage: LOCUST_SPEND=1 node _tools/runtime-canary.mjs --packaged <Locust.exe> [--runtime codex|copilot] [--version <v>] [--force]')
  process.exit(2)
}
if (process.env.LOCUST_SPEND !== '1') {
  console.log('The canary runs one real turn on the agent\'s account: set LOCUST_SPEND=1.')
  process.exit(2)
}
const npm = (args) => spawnSync('npm', args, { encoding: 'utf8', shell: true })
const versionOf = (text) => /(\d+\.\d+\.\d+(?:-[\w.]+)?)/.exec(text ?? '')?.[1]

const newest = arg('--version') ?? versionOf(npm(['view', agent.pkg, 'version']).stdout)
let installed
try { installed = versionOf(execFileSync(agent.command, ['--version'], { encoding: 'utf8', shell: true })) } catch { installed = undefined }
console.log(`${runtime}: this machine has ${installed ?? 'none'}; npm's newest is ${newest ?? 'unknown'}`)
if (newest === undefined) { console.log('npm did not answer; nothing checked.'); process.exit(1) }
if (newest === installed && !force) { console.log('Nothing new to check.'); process.exit(0) }

const prefix = join(SCRATCH, 'runtime-canary', `${runtime}-${newest}`)
const bin = join(prefix, 'node_modules', '.bin')
if (!existsSync(join(bin, `${agent.command}.cmd`)) && !existsSync(join(bin, agent.command))) {
  console.log(`installing ${agent.pkg}@${newest} into ${prefix} (not the global one)`)
  await mkdir(prefix, { recursive: true })
  const put = npm(['install', '--prefix', `"${prefix}"`, '--no-audit', '--no-fund', `${agent.pkg}@${newest}`])
  if (put.status !== 0) { console.log(`install failed: ${(put.stderr ?? '').slice(-400)}`); process.exit(1) }
}
const canaryVersion = versionOf(spawnSync(join(bin, agent.command), ['--version'], { encoding: 'utf8', shell: true }).stdout)
console.log(`the canary's ${agent.command}: ${canaryVersion ?? 'did not answer'}`)

const tag = `canary-${newest}`
const turn = spawnSync(process.execPath, [join(ROOT, '_tools', 'drive-a-turn-reads-as-it-happens.mjs'), '--packaged', packaged, '--runtime', runtime, '--tag', tag], {
  encoding: 'utf8',
  env: { ...process.env, LOCUST_DRIVE_PATH_FIRST: bin, LOCUST_DRIVE_KEEP: '1' },
  timeout: 20 * 60 * 1000
})
const said = `${turn.stdout ?? ''}${turn.stderr ?? ''}`
const checks = said.split('\n').filter((line) => /\[(PASS|FAIL)\]/.test(line)).map((line) => line.trim())
const failed = checks.filter((line) => line.startsWith('[FAIL]'))

// The turn must have run on the release being checked, as its own record says.
let ranOn
const profile = /profile kept: (.+)/.exec(said)?.[1]?.trim()
if (profile !== undefined) {
  const ledger = join(profile, 'mission-ledger')
  for (const file of (await readdir(ledger).catch(() => [])).filter((name) => name.endsWith('.jsonl'))) {
    const text = await readFile(join(ledger, file), 'utf8').catch(() => '')
    ranOn ??= /"cliVersion":"([^"]+)"/.exec(text)?.[1]
  }
}
const ranOnIt = ranOn !== undefined && versionOf(ranOn) === newest
const ok = turn.status === 0 && failed.length === 0 && checks.length > 0 && ranOnIt
const verdict = {
  runtime,
  version: newest,
  installedHere: installed ?? null,
  checkedAt: new Date().toISOString(),
  ok,
  ranOn: ranOn ?? null,
  checks,
  why: ok ? 'One real turn read as it should on this release.'
    : !ranOnIt ? `The turn did not run on ${newest} (its record says ${ranOn ?? 'nothing'}), so this release was not checked.`
      : failed.length > 0 ? `Locust's reading of this release failed: ${failed.join(' / ')}`
        : `The drive ended ${String(turn.status)} without its checks: ${said.slice(-300)}`
}
const out = join(ROOT, 'docs', 'runtime-canary')
await mkdir(out, { recursive: true })
await writeFile(join(out, `${runtime}-${newest}.json`), `${JSON.stringify(verdict, null, 2)}\n`, 'utf8')
for (const line of checks) console.log(`  ${line.slice(0, 160)}`)
// --publish: beside the installers, where installed copies read it before updating (0.501).
if (process.argv.includes('--publish') && (ok || ranOnIt)) {
  try {
    console.log(run(process.execPath, [join(ROOT, '_tools', 'publish-canary-verdict.mjs'), join(out, `${runtime}-${newest}.json`)], { encoding: 'utf8' }).trim())
  } catch (error) {
    console.log(`the verdict could not be published: ${error instanceof Error ? error.message : String(error)}`)
  }
}
console.log(ok ? `CANARY PASSED: ${runtime} ${newest}` : `CANARY FAILED: ${runtime} ${newest} -- ${verdict.why.slice(0, 300)}`)
process.exit(ok ? 0 : 1)
