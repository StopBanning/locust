// The packaged build, driven. Nobody had ever done this.
//
//   node _tools/packaged-audit.mjs
//
// Tier 1, item 4 of the beta plan: every drive this repo has ever run --
// mine and both of Grok's -- launched `electron .` against the source tree.
// That is not the build anybody installs. The packaged app is a different
// set of bytes: an asar, a production main process, `app.isPackaged` true,
// and resources resolved from beside the exe rather than from the repo.
//
// Grok has now declined a Windows pass three times, correctly -- it has no
// Windows host and would not use Wine as one. This machine is Windows, so
// this gap is mine and not theirs.
//
// WHAT THIS IS NOT. It drives `release/win-unpacked/Locust.exe`, which is
// the packaged app, and it does NOT run the NSIS installer. Shortcuts, the
// install directory, the uninstaller, per-user install and upgrade-over-old
// are still unaudited by anyone and still need a person. Saying so here
// rather than letting "packaged audited" stand for more than it did.
//
// The profile is a throwaway `--user-data-dir`, so this never touches the
// real installed Locust's records.

import { writeFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { localPath, FREE_ROUTE, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const EXE = localPath(new URL('../apps/desktop/release/win-unpacked/Locust.exe', import.meta.url))
const OUT = localPath(new URL('../docs/packaged-audit-2026-09-15/', import.meta.url))

if (!existsSync(EXE)) {
  say(`no packaged build at ${EXE} -- run node _tools/ship.mjs first`)
  process.exit(1)
}

const at = '2026-09-11T05:00:00.000Z'
const workspace = await scratchRepository('locust-packaged-')
const findings = []
const check = (what, ok, detail) => {
  findings.push({ what, ok, detail })
  say(`${ok ? 'PASS' : 'FAIL'}  ${what} -- ${detail}`)
}

const drive = await startDrive({
  name: 'packaged-audit',
  port: 9294,
  workspace,
  packaged: EXE,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE }
    ],
    missionOwners: {},
    settings: { swarm: false }
  }
})

try {
  await drive.ready()
  await sleep(2500)

  check('the packaged app starts and its renderer comes up', true, 'drive.ready() returned')

  /*
   * THE SHAPE THAT HAS COST THIS PROJECT MOST.
   *
   * A connector notice once shipped, ran zero times on Windows and only on
   * Windows because `execFile` cannot start a `.cmd`, failed inside its own
   * catch, and so looked exactly like the healthy case. Every coding CLI on
   * Windows is a `.cmd` shim in the npm prefix, and discovery is the code
   * that spawns them -- so if that class of bug is anywhere, it is here, and
   * it is invisible on Linux and invisible in a dev build that happens to
   * resolve differently.
   */
  const runtimes = await drive.evaluate(`(async () => {
    const found = await window.desktop.getLocalRuntimes()
    const list = found?.data?.runtimes ?? []
    return JSON.stringify(list.map((r) => ({ id: r.id, installed: r.installed, status: r.status, version: r.version })))
  })()`)
  const parsed = JSON.parse(runtimes)
  const ready = parsed.filter((r) => r.status === 'ready')
  check(
    'discovery finds real CLIs from the packaged build',
    ready.length > 0,
    `${String(ready.length)} ready of ${String(parsed.length)}: ${ready.map((r) => `${r.id} ${r.version ?? '?'}`).join(', ') || 'none'}`
  )
  check(
    'every ready runtime reported a version, so the probe truly ran',
    ready.length > 0 && ready.every((r) => typeof r.version === 'string' && r.version.length > 0),
    ready.map((r) => `${r.id}=${String(r.version)}`).join(' ') || 'nothing ready'
  )

  // `app.isPackaged` decides the id Windows names the app by, the updater,
  // and the line Settings prints. A packaged build calling itself a
  // development one would mean the whole distinction is not wired.
  const settings = await drive.evaluate(`(async () => {
    const buttons = [...document.querySelectorAll('button')]
    const open = buttons.find((b) => (b.innerText || '').trim() === 'Settings')
    if (open === undefined) return JSON.stringify({ noSettings: true })
    open.click()
    for (let i = 0; i < 40; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      if (document.querySelector('.lc-settings__section')) break
    }
    const text = document.body.innerText || ''
    const version = (text.match(/Locust\\s+(\\d+\\.\\d+\\.\\d+)/) || [])[1] || ''
    return JSON.stringify({
      version,
      saysDevelopment: text.includes('development build'),
      hasReportAProblem: text.includes('Report a problem'),
      namesTheLog: text.includes('locust-errors.log')
    })
  })()`)
  const seen = JSON.parse(settings)
  check('Settings names the packaged version', seen.version.length > 0, `Settings says ${seen.version || '(nothing)'}`)
  check(
    'a packaged build does not call itself a development build',
    seen.saysDevelopment === false,
    seen.saysDevelopment === true ? 'it says "development build"' : 'it does not'
  )
  check('Report a problem is there in the packaged build', seen.hasReportAProblem === true, `names the log: ${String(seen.namesTheLog)}`)

  // The diagnostics line must say `packaged`, which is the one fact a log
  // sent in from a tester cannot be reconstructed without.
  const log = join(drive.profile, 'locust-errors.log')
  const firstLine = existsSync(log)
    ? (await (await import('node:fs/promises')).readFile(log, 'utf8')).trim().split('\n')[0] ?? ''
    : ''
  check(
    'the log opens with a line that says this was a packaged run',
    firstLine.includes('packaged'),
    firstLine || '(no log)'
  )

  // The flat sidebar, in the bytes that ship.
  const sidebar = await drive.evaluate(`(() => {
    const footer = document.querySelector('.lc-sidebar__footer')
    const labels = footer === null ? [] : [...footer.querySelectorAll('button span:last-child')].map((s) => (s.innerText || '').trim())
    return JSON.stringify({ labels, hasList: document.querySelector('.lc-convlist') !== null })
  })()`)
  const shell = JSON.parse(sidebar)
  check(
    'every footer destination is reachable in the packaged build',
    ['Missions', 'Rooms', 'Routines', 'Teammates', 'Settings'].every((name) => shell.labels.includes(name)),
    shell.labels.join(' · ') || '(none)'
  )

  // Files electron-builder has to place BESIDE the asar. A missing one is
  // invisible until the feature that needs it is used.
  const bridge = await drive.evaluate(`(async () => {
    const report = await window.desktop.diagnosticsReport()
    return JSON.stringify({ path: report.path, exists: report.exists })
  })()`)
  check('the host answers IPC added this week', JSON.parse(bridge).exists === true, JSON.parse(bridge).path)

  const errors = drive.consoleErrors?.() ?? []
  check('the renderer logged no console errors', errors.length === 0, errors.slice(0, 3).join(' | ') || 'none')

  await sleep(500)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
  await (await import('node:fs/promises')).mkdir(OUT, { recursive: true })
  await writeFile(OUT + 'packaged-settings.png', Buffer.from(shot.result.data, 'base64'))
  await writeFile(OUT + 'findings.json', JSON.stringify({ exe: EXE, at: new Date().toISOString(), findings }, null, 2), 'utf8')

  const failed = findings.filter((f) => !f.ok)
  say(`\n${String(findings.length - failed.length)} of ${String(findings.length)} checks passed`)
  if (failed.length > 0) say(`FAILED: ${failed.map((f) => f.what).join(' | ')}`)
} finally {
  await drive.finish({ intro: 'packaged-audit', last: true }).catch(() => undefined)
}
