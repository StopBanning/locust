// How does the update banner look where it sits?
//
//   node _tools/probe-update-banner.mjs [--packaged <exe>] [--tag <name>]
//
// The sweep (D5): "Update banner: off-palette navy, text not centred on its
// buttons." The banner shows only when a newer build has downloaded, which a
// drive cannot arrange, so this places the banner's own markup
// (UpdateBanner in Screens.tsx) where the app mounts it -- above the
// composer -- under the app's own stylesheet, photographs it, and measures
// the button's text against the button. Sends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `update-banner-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `update-banner-${tag}`,
  port: 9426,
  workspace: await scratchRepository('locust-banner-ws-'),
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
  const measured = JSON.parse(await drive.evaluate(`(async () => {
    const dock = document.querySelector('form.command-dock')
    if (!dock) return JSON.stringify({ error: 'no composer' })
    const banner = document.createElement('div')
    banner.className = 'lc-updatebanner'
    banner.setAttribute('role', 'status')
    banner.innerHTML = '<span class="lc-updatebanner__text">Locust 0.292.0 is downloaded and ready. It installs when you restart.</span><button type="button" class="lc-button">Restart and install</button>'
    dock.parentElement.insertBefore(banner, dock)
    await new Promise((r) => setTimeout(r, 400))
    const button = banner.querySelector('button')
    const box = button.getBoundingClientRect()
    const range = document.createRange()
    range.selectNodeContents(button)
    const text = range.getBoundingClientRect()
    const style = getComputedStyle(banner)
    const b = banner.getBoundingClientRect()
    // The app's card surface, read from its own token rather than a hex.
    const probe = document.createElement('div')
    probe.style.background = 'var(--lc-bg-raised)'
    document.body.appendChild(probe)
    const card = getComputedStyle(probe).backgroundColor
    probe.remove()
    window.__bannerBox = { x: b.left, y: b.top, width: b.width, height: b.height }
    return JSON.stringify({
      background: style.backgroundColor,
      card,
      border: style.borderTopColor,
      buttonHeight: Math.round(box.height),
      textAbove: Math.round((text.top - box.top) * 10) / 10,
      textBelow: Math.round((box.bottom - text.bottom) * 10) / 10,
      textLeft: Math.round((text.left - box.left) * 10) / 10,
      textRight: Math.round((box.right - text.right) * 10) / 10
    })
  })()`))
  say(`measured: ${JSON.stringify(measured)}`)
  const box = JSON.parse(await drive.evaluate('JSON.stringify(window.__bannerBox)'))
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', clip: { x: box.x - 16, y: box.y - 16, width: box.width + 32, height: box.height + 32, scale: 2 } })
  if (shot?.result?.data) await writeFile(join(OUT, 'banner.png'), Buffer.from(shot.result.data, 'base64'))
  check("the banner is the app's card surface, not a navy tint", measured.background === measured.card, `${measured.background} vs card ${measured.card}`)
  check("the button's text sits in its middle, top to bottom", Math.abs(measured.textAbove - measured.textBelow) <= 1, `${String(measured.textAbove)} above, ${String(measured.textBelow)} below`)
  check("and side to side", Math.abs(measured.textLeft - measured.textRight) <= 1, `${String(measured.textLeft)} left, ${String(measured.textRight)} right`)
  say(failures === 0 ? '\nUPDATE BANNER PASSED' : `\nUPDATE BANNER: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: "The update banner's own markup, placed where the app mounts it, under the app's stylesheet. Nothing was sent." })
}
