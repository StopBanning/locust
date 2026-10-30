// Does Settings > Appearance put reading comfort before ornament?
//
//   node _tools/probe-appearance-order.mjs [--packaged <exe>] [--tag <name>]
//
// The design review: "Appearance: reading comfort before ornament". Reply
// text size -- the one setting that changes how a reply reads -- sat under
// the sidebar and a long block of send-button effects. Opens the page and
// reads its headings in order. Sends nothing.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `appearance-order-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `appearance-order-${tag}`,
  port: 9428,
  workspace: await scratchRepository('locust-appearance-ws-'),
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const headings = JSON.parse(await drive.capture('Settings > Appearance', () => drive.evaluate(`(async () => {
    const settings = [...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Settings')
    if (!settings) return JSON.stringify({ error: 'no Settings button' })
    settings.click()
    await new Promise((r) => setTimeout(r, 800))
    const page = [...document.querySelectorAll('button, a')].find((b) => b.innerText.trim() === 'Appearance')
    if (!page) return JSON.stringify({ error: 'no Appearance page' })
    page.click()
    await new Promise((r) => setTimeout(r, 800))
    return JSON.stringify({ headings: [...document.querySelectorAll('.lc-settings__heading')].map((h) => h.textContent.trim()) })
  })()`)))
  say(`headings: ${JSON.stringify(headings)}`)
  const order = headings.headings ?? []
  check('Reply text size comes first', order[0] === 'Reply text size', order.join(' | '))
  check('and ahead of the send button and the boot screen', order.indexOf('Reply text size') < order.indexOf('Send button') && order.indexOf('Reply text size') < order.indexOf('Boot screen'), order.join(' | '))
  say(failures === 0 ? '\nAPPEARANCE ORDER PASSED' : `\nAPPEARANCE ORDER: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Settings > Appearance, headings in order. Nothing was sent.' })
}
