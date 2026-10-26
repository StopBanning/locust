// A teammate on Kilo's Auto Free model: one turn in Auto, one in Ask.
//
//   LOCUST_SPEND=1 LOCUST_DRIVE_PATH_FIRST=<kilo.exe folder> node _tools/drive-a-kilo-teammate-runs-free.mjs
//
// Auto Free is a free gateway route. The window is opened with spends so the
// free-only gate does not refuse it, and the route picked is still Auto Free.
// Ask must change nothing. Auto writes one file and reads it back.
// Frames land under D:/work/scratch/grok-kilo/. The window is 1200 by 720.

import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { openTeammateScript, pickRouteScript, say, scratchRepository, sendAndWaitScript, startDrive } from './drive-lib.mjs'

const LINE = 'pebble-line-auto'
const ASK_LINE = 'pebble-line-ask'
const NOTE = 'kilo-note.txt'
const ASK_FILE = 'kilo-ask.txt'
const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
const outPath = join('D:/work/scratch/grok-kilo', `${stamp}-kilo-free`)

const workspace = await scratchRepository('locust-drive-kilo-')
const drive = await startDrive({
  name: 'kilo-free',
  port: 9348,
  workspace,
  outPath,
  spends: true,
  focused: true,
  seed: {
    schemaVersion: 1,
    teammates: [
      // account-default is the session's own model. Opening the teammate must move off it onto Auto Free.
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-26T04:00:00.000Z', route: { runtime: 'kilo', model: 'account-default', mode: 'ask' } },
      { teammateId: 'tm_vale', name: 'Vale', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-10-26T04:00:00.000Z', route: { runtime: 'kilo', model: 'account-default', mode: 'auto' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: true }
  }
})

const checks = []
const check = (name, ok, detail) => {
  checks.push({ name, ok, detail })
  say(`${ok ? 'ok' : 'FAIL'}  ${name} — ${detail}`)
}

const chip = () => drive.evaluate(`(() => {
  const control = [...document.querySelectorAll('.lc-control')].find((button) => button.getAttribute('aria-haspopup') === 'listbox')
  if (!control) return ''
  const text = control.innerText.replace(/\\s+/g, ' ').trim()
  const title = (control.getAttribute('title') ?? '').replace(/\\s+/g, ' ').trim()
  return text + ' || ' + title
})()`)

const waitForChip = async () => {
  const deadline = Date.now() + 90_000
  let last = ''
  while (Date.now() < deadline) {
    last = String(await chip() ?? '')
    const [text, title] = last.split(' || ')
    if (/auto free/i.test(text ?? '') || /kilo-auto\\/free/i.test(title ?? '')) return last
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  return last
}

const pickMode = (pattern) => `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((button) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(button.innerText))
  if (!control) return 'no mode control'
  if (!${pattern}.test(control.innerText.trim())) {
    control.click()
    await new Promise((resolve) => setTimeout(resolve, 400))
    const item = [...document.querySelectorAll('[role=menuitemradio]')].find((button) => ${pattern}.test(button.innerText.trim()))
    if (!item) { control.click(); return 'mode not offered: ' + [...document.querySelectorAll('[role=menuitemradio]')].map((button) => button.innerText.trim()).join(' | ') }
    item.click()
    await new Promise((resolve) => setTimeout(resolve, 400))
  }
  return 'mode: ' + control.innerText.replace(/[ \\t\\n]+/g, ' ').trim()
})()`

const listWorkspace = async () => (await readdir(workspace)).sort().join(', ')

try {
  await drive.resize(1200, 720)
  await drive.capture('the window is ready', async () => {
    const ready = await drive.ready()
    return String(ready)
  })

  await drive.capture('Wren is on Kilo Auto Free before anyone picks', async () => {
    const opened = await drive.evaluate(openTeammateScript('Wren'))
    const shown = await waitForChip()
    const row = await drive.evaluate(`(async () => {
      const control = [...document.querySelectorAll('.lc-control')].find((button) => button.getAttribute('aria-haspopup') === 'listbox')
      if (!control) return 'no route control'
      if (!document.querySelector('.lc-picker')) control.click()
      let found = ''
      for (let attempt = 0; attempt < 40 && found === ''; attempt += 1) {
        const picker = document.querySelector('.lc-picker')
        if (!picker) { control.click(); await new Promise((resolve) => setTimeout(resolve, 500)); continue }
        const box = picker.querySelector('.lc-picker__input')
        if (box && box.value !== 'Auto Free') {
          const setInput = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
          setInput.call(box, 'Auto Free')
          box.dispatchEvent(new Event('input', { bubbles: true }))
          await new Promise((resolve) => setTimeout(resolve, 400))
        }
        let group = ''
        for (const node of picker.querySelector('.lc-picker__list')?.children ?? []) {
          const header = node.querySelector('.lc-picker__group')
          if (header) group = header.innerText.replace(/\\s+/g, ' ').trim()
          const candidate = node.querySelector('.lc-picker__row')
          if (candidate && /auto free/i.test(candidate.innerText) && /kilo/i.test(group)) {
            found = group + ' || ' + candidate.innerText.replace(/\\s+/g, ' ').trim()
            break
          }
        }
        if (found === '') await new Promise((resolve) => setTimeout(resolve, 500))
      }
      return found || 'Auto Free row not listed'
    })()`)
    const chipText = shown.split(' || ')[0] ?? ''
    const chipTitle = shown.split(' || ').slice(1).join(' || ')
    check('default model', /auto free/i.test(chipText) || /kilo-auto\/free/i.test(chipTitle), shown)
    check('picker row', /kilo/i.test(String(row)) && /auto free/i.test(String(row)) && /confidential/i.test(String(row)) && !/· your account/i.test(String(row)), String(row))
    return `${opened} || chip: ${shown} || row: ${row}`
  })

  const beforeAsk = await listWorkspace()
  await drive.capture('Ask changes nothing', async () => {
    const mode = await drive.evaluate(pickMode('/^Ask\\b/'))
    const route = await drive.evaluate(pickRouteScript({ group: '/^kilo$/i', search: 'Auto Free', row: '/Auto Free/i' }))
    const turn = await drive.evaluate(sendAndWaitScript(
      `Create a file named ${ASK_FILE} whose only line is ${ASK_LINE}. If you cannot create or change files, say so in one sentence and do not create a file.`,
      { waitSeconds: 150 }
    ))
    const after = await listWorkspace()
    let askWrote = false
    try {
      askWrote = (await readFile(join(workspace, ASK_FILE), 'utf8')).includes(ASK_LINE)
    } catch { askWrote = false }
    check('ask mode', /^mode: Ask/.test(String(mode)), String(mode))
    check('ask route', /auto free/i.test(String(route)), String(route))
    check('ask turn finished', String(turn).startsWith('finished:') && !/Starting Kilo/i.test(String(turn)), String(turn).slice(0, 400))
    check('ask changed nothing', beforeAsk === after && askWrote === false, `before ${beforeAsk} ; after ${after}`)
    return `${mode} || ${route} || ${String(turn).slice(0, 500)}`
  })

  await drive.capture('Auto writes the note and reads it back', async () => {
    const opened = await drive.evaluate(openTeammateScript('Vale'))
    const mode = await drive.evaluate(pickMode('/^Auto\\b/'))
    const route = await drive.evaluate(pickRouteScript({ group: '/^kilo$/i', search: 'Auto Free', row: '/Auto Free/i' }))
    const turn = await drive.evaluate(sendAndWaitScript(
      `In this folder, write a file named ${NOTE} whose only line is ${LINE}. Then read that file and reply with exactly that line and nothing else. Do not create any other file.`,
      { waitSeconds: 150 }
    ))
    let body = ''
    try { body = await readFile(join(workspace, NOTE), 'utf8') } catch { body = '' }
    const thread = String(turn)
    check('auto mode', /^mode: Auto/.test(String(mode)), String(mode))
    check('auto route', /auto free/i.test(String(route)), String(route))
    const reply = thread.slice(thread.indexOf(LINE) + LINE.length)
    check('auto turn finished', thread.startsWith('finished:') && !/Starting Kilo/i.test(thread), thread.slice(0, 400))
    check('file written', body.includes(LINE), body.slice(0, 200) || 'file missing')
    check('file read back', reply.includes(LINE), reply.slice(-300))
    return `${opened} || ${mode} || ${route} || ${thread.slice(0, 500)}`
  })
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
  checks.push({ name: 'drive', ok: false, detail: error instanceof Error ? error.message : String(error) })
} finally {
  const errorFiles = await readdir(drive.out).then((names) => names.filter((name) => name.endsWith('.errors.txt'))).catch(() => [])
  check('no renderer errors', errorFiles.length === 0, errorFiles.length === 0 ? 'none' : errorFiles.join(', '))
  const failed = checks.filter((entry) => entry.ok !== true)
  await drive.finish({
    intro: 'Wren runs Ask and Vale runs Auto, both on Kilo Auto Free, in a scratch folder. Ask is asked to create a file and must not. Auto writes one line and reads it back. The window is 1200 by 720.'
  })
  say(failed.length === 0 ? 'every check passed' : `${String(failed.length)} check(s) failed`)
  if (failed.length > 0) process.exitCode = 1
}
