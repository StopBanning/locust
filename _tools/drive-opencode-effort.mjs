// Can a person choose an OpenCode model's effort, and does it reach the run?
//
//   node _tools/drive-opencode-effort.mjs [--packaged <exe>] [--tag <label>]
//
// A6.5. OpenCode's run takes `--variant`, "provider-specific reasoning
// effort", and `opencode models --verbose` lists each model's variants --
// the free DeepSeek V4.1 Flash lists low, high and max (Ling, this drive's
// first model, left OpenCode by 2026-10-22). Locust refused
// every OpenCode effort and the composer said "Fixed". Measured from a shell
// on 2026-09-25, the variant reaches the provider: `high` reasoned more than
// `low` in four runs of four.
//
// So: what does the picker offer on it, can the top stop be chosen, and does
// OpenCode run the turn at it? Read back from OpenCode's own record of the
// session (`opencode export`), not inferred from Locust's.
//
// Free model only.

import { execFile } from 'node:child_process'
import { mkdir, readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { promisify } from 'node:util'

import { pickRouteScript, say, scratchRepository, sendAndWaitScript, startDrive, teammateFace, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `opencode-effort-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })


const run = promisify(execFile)
const workspace = await scratchRepository('locust-drive-oc-effort-ws-')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  name: 'opencode-effort',
  port: 9313,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

// The effort panel opens from the composer's own effort control, which reads
// the chosen level ("Medium") or, on a model with none, is the label "Fixed".
const EFFORT = `(async () => {
  const flat = (el) => el.innerText.split(/\\s+/).join(' ').trim()
  const button = [...document.querySelectorAll('button.lc-control')].find((b) => b.querySelector('.lc-control__effort'))
  if (!button) return { chip: 'no effort control', notches: [] }
  if (!document.querySelector('.lc-effortpanel')) { button.click(); await new Promise((r) => setTimeout(r, 700)) }
  // The level is a range input: its stops are the model's efforts.
  const slider = document.querySelector('.lc-effortpanel__slider')
  const notches = slider === null ? [] : [String(Number(slider.max) + 1) + ' stops, now ' + slider.getAttribute('aria-valuetext')]
  return { chip: flat(button), notches }
})()`

const CLOSE = `(async () => {
  if (document.querySelector('.lc-effortpanel')) {
    const button = [...document.querySelectorAll('button.lc-control')].find((b) => b.querySelector('.lc-control__effort'))
    button?.click()
    await new Promise((r) => setTimeout(r, 400))
  }
})()`

const fixedLabel = () => drive.evaluate(`(document.querySelector('.lc-control__effort')?.innerText ?? 'no Fixed label')`)

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

/**
 * The efforts OpenCode itself recorded for this drive's runs: its own record of the session, `opencode export`.
 * Locust talks to OpenCode's server, so the effort travels in the request and never on a command line -- the argv
 * this drive used to read off the live process said "NO OpenCode process seen" on the 0.818 build. MEASURED
 * 2026-10-22 (1.18.27): a turn sent with variant max is exported with `"variant": "max"` on both of its messages.
 */
async function recordedVariants() {
  const ledger = join(drive.profile, 'mission-ledger')
  const texts = await Promise.all((await readdir(ledger).catch(() => [])).filter((name) => name.endsWith('.jsonl')).map((name) => readFile(join(ledger, name), 'utf8').catch(() => '')))
  const session = texts.join('\n').match(/ses_[A-Za-z0-9]{20,}/)?.[0]
  if (session === undefined) return { session: undefined, variants: [] }
  const { stdout } = await run('powershell', ['-NoProfile', '-NonInteractive', '-Command', `opencode export ${session}`], { maxBuffer: 32 * 1024 * 1024 }).catch((error) => ({ stdout: String(error?.stdout ?? '') }))
  return { session, variants: [...new Set([...stdout.matchAll(/"variant":\s*"([^"]+)"/g)].map((match) => match[1]))] }
}

try {
  const offered = await drive.capture('DeepSeek picked: what the effort control offers', async () => {
    await drive.ready()
    await drive.evaluate(`(async () => { ${teammateFace('Wren')}.click(); await new Promise((r) => setTimeout(r, 900)) })()`)
    const route = await drive.evaluate(pickRouteScript({ group: '/opencode/i', search: 'deepseek v4.1 flash', row: '/deepseek.*v4.1.*flash/i' }))
    const picker = await drive.evaluate(EFFORT)
    await drive.evaluate(CLOSE)
    return `${route} || effort control: ${picker.chip} || effort notches: ${picker.notches.length === 0 ? 'NONE' : picker.notches.join(' ')} || composer label: ${await fixedLabel()}`
  })
  check('the free model offers its efforts, three stops', /effort notches: 3 stops/.test(String(offered)), offered)

  const chosen = await drive.capture('the top stop chosen', async () => {
    await drive.evaluate(EFFORT)
    const said = await drive.evaluate(`(async () => {
      const slider = document.querySelector('.lc-effortpanel__slider')
      if (!slider) return 'no effort slider'
      // The top stop, set the way React hears it.
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(slider, slider.max)
      slider.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise((r) => setTimeout(r, 700))
      return 'slider now: ' + slider.getAttribute('aria-valuetext')
    })()`)
    await drive.evaluate(CLOSE)
    const chip = (await drive.evaluate(EFFORT)).chip
    await drive.evaluate(CLOSE)
    return `${said} || effort control now: ${chip}`
  })
  check('its top stop, Max, can be chosen', /slider now: Max \|\| effort control now: Max/.test(String(chosen)), chosen)

  const ran = await drive.capture('the run, and the effort OpenCode recorded for it', async () => {
    const sent = await drive.evaluate(sendAndWaitScript('Reply with exactly the word PEBBLE.', { waitSeconds: 180 }))
    const recorded = await recordedVariants()
    return `${sent} || session ${recorded.session ?? 'none in the record'} || OpenCode recorded variant: ${recorded.variants.length === 0 ? 'NONE' : recorded.variants.join(', ')}`
  })
  check('the run answered, and OpenCode recorded it at max', /PEBBLE/.test(String(ran)) && /OpenCode recorded variant: max$/.test(String(ran)), ran)
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: 'OpenCode / DeepSeek V4.1 Flash Free, which lists low, high and max as its variants. The effort is read back from OpenCode\'s own record of the session (opencode export).'
  })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
