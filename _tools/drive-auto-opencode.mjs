// Auto mode on OpenCode: whether it really reaches outside the folder.
//
//   node _tools/drive-auto-opencode.mjs [--packaged <exe>] [--tag <label>]
//
// A B4 lead from the code review: OpenCode's Auto run carried
// `external_directory: "deny"` beside `--auto`, and `--auto` approves only
// what is NOT explicitly denied -- so Auto stayed in the folder, which is the
// one thing Auto is for. Measured from a shell on 2026-09-25 with the free
// model: "deny" refused an outside read, "allow" returned the file.
//
// This drive settles it through the app. A marker file sits in a directory the
// mission is never given; the control asks for it under Edit, the
// test asks for it under Auto, each from a teammate with no history so neither
// answers from a remembered refusal. The proof is the marker's text in the
// thread -- it exists nowhere else.
//
// Free model only: the drives' free route on OpenCode (drive-lib FREE_ROUTE).

import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, FREE_ROW, FREE_SEARCH, pickRouteScript, say, scratchRepository, sendAndWaitScript, startDrive, teammateFace, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `auto-opencode-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })


const workspace = await scratchRepository('locust-drive-auto-oc-ws-')
const outside = await mkdtemp(join(tmpdir(), 'locust-drive-auto-oc-outside-'))
const MARKER = 'OUTSIDE-MARKER-4410'
await writeFile(join(outside, 'secret.txt'), `${MARKER}\n`, 'utf8')
const target = join(outside, 'secret.txt')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  name: 'auto-opencode',
  port: 9312,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' },
      { teammateId: 'tm_vale', name: 'Vale', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const pickMode = (pattern) => `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find(b => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(b.innerText))
  if (!control) return 'no mode control'
  control.click(); await new Promise(r => setTimeout(r, 400))
  const item = [...document.querySelectorAll('[role=menuitemradio]')].find(b => ${pattern}.test(b.innerText.trim()))
  if (!item) { control.click(); return 'mode not offered' }
  item.click(); await new Promise(r => setTimeout(r, 400))
  return 'mode: ' + control.innerText.replace(/[ \\t\\n]+/g, ' ').trim()
})()`

const ask = `Use your read tool on the file ${target} and reply with exactly what it contains, or say plainly that you could not read it.`
const threadTail = () => drive.evaluate(`(document.querySelector('.lc-thread')?.innerText.replace(/[ \\t\\n]+/g, ' ').slice(-260) ?? '')`)
const verdict = (tail) => (tail.includes(MARKER) ? `REACHED: the marker is in the thread` : 'HELD: no marker in the thread')

const runAs = async (name, modePattern) => {
  await drive.evaluate(`(async () => { ${teammateFace(name)}.click(); await new Promise(r => setTimeout(r, 900)) })()`)
  const route = await drive.evaluate(pickRouteScript({ group: '/opencode/i', search: FREE_SEARCH, row: FREE_ROW }))
  const mode = await drive.evaluate(pickMode(modePattern))
  await drive.evaluate(sendAndWaitScript(ask, { waitSeconds: 240 }))
  const tail = await threadTail()
  return `${route} || ${mode} || ${verdict(tail)} || ${tail}`
}

try {
  await drive.capture('the control: Wren, Edit, asked to read a file outside the folder', async () => {
    await drive.ready()
    return runAs('Wren', '/^Edit\\b/')
  })
  await drive.capture('the test: Vale, Auto, the same ask', () => runAs('Vale', '/^Auto\\b/'))
  await drive.capture('the header records what the Auto run was allowed', () => drive.evaluate(`(document.querySelector('.lc-workroom__header')?.innerText.replace(/[ \\t\\n]+/g, ' ').slice(0, 200) ?? '')`))
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Wren runs the control under Edit and Vale runs Auto, both on OpenCode's free ${FREE_ROUTE.model}, so the two never share a session. The marker ${MARKER} sits in ${outside.replace(/\\/g, '/')}, a directory the mission was never given; it reaches the thread only if the run left its folder.`
  })
  await rm(outside, { recursive: true, force: true })
}
