// A design question for Colin, shown rather than described (0.355).
//
//   node _tools/probe-heading-case.mjs [--packaged <exe>]
//
// Section headings inside a screen ("HOW MEMORY IS KEPT", "PROJECT FOLDER")
// are set like the wordmark: 17px, bold, wide-tracked capitals -- a rule a
// test pins. Claude sets them in sentence case, smaller and calmer. This
// photographs Memory and Settings as they are, then with ONLY that heading
// rule overridden in the page, so the two can be compared frame for frame.
// Nothing is saved; the override lives in the page until it closes.

import { join } from 'node:path'
import { mkdir } from 'node:fs/promises'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const outPath = join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), 'heading-case-question')
await mkdir(outPath, { recursive: true })

const T0 = new Date(Date.now() - 6 * 24 * 60 * 60 * 1000).toISOString()
const kept = (memoryId, text) => ({ memoryId, text, scope: 'workspace', workspaceId: 'ws_x', workspaceName: 'shop', by: { name: 'you' }, createdAt: T0, status: 'kept', enabled: true })
const workspace = await scratchRepository('locust-probe-headings-ws-')
const drive = await startDrive({
  name: 'heading-case',
  port: 9615,
  workspace,
  outPath,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0 }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'auto' }
  },
  files: {
    'memories.json': {
      schemaVersion: 1,
      memories: [kept('mem_a', 'Run npm test before calling a code change done.'), kept('mem_b', 'Keep explanations short; lead with the answer.')]
    }
  }
})

const screen = (key) => drive.evaluate(`(async () => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: '${key}', ctrlKey: true, bubbles: true }))
  await new Promise((r) => setTimeout(r, 1100))
  return [...document.querySelectorAll('.lc-settings__heading')].map((h) => h.textContent).slice(0, 4).join(' | ')
})()`)

const sentenceCase = `(() => {
  const style = document.createElement('style')
  style.textContent = '.lc-settings__heading:not(.lc-settings__heading--section) { font-size: 16px; font-weight: 600; letter-spacing: -0.005em; text-transform: none; }'
  document.head.appendChild(style)
  return 'sentence case applied'
})()`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await sleep(1200)
  await drive.capture('A - Memory as it is (capitals)', () => screen('5'))
  await drive.capture('A - Settings as it is (capitals)', () => screen('3'))
  await drive.evaluate(sentenceCase)
  await drive.capture('B - Memory in sentence case', () => screen('5'))
  await drive.capture('B - Settings in sentence case', () => screen('3'))
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. The same two screens with the section headings as they are (A), and with only that rule set in sentence case at 16px/600 (B).` })
}
