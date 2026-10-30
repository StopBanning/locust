// An attached IMAGE, drawn as itself.
//
//   node _tools/drive-attach-image.mjs
//
// Colin asked for images in chat. The model half already worked -- measured,
// Claude Code, Cursor and Copilot all open a screenshot they are pointed at
// (docs/ATTACHMENTS-MEASURED-2026-09-08.md) -- so what was missing was the
// person's half: attaching a screenshot and seeing the word `shot.png`, with
// no way to tell WHICH screenshot without leaving the app.
//
// This attaches a real PNG from outside the workspace (so the copy-in path
// runs too) and checks that a thumbnail is actually painted, at a real size,
// in both the composer and the thread. Nothing is sent, so nothing is spent.

import { copyFile, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-drive-attachimg-ws-')
// A real screenshot, from a drive record, kept outside the workspace so this
// exercises the copy-in path as well.
const elsewhere = await mkdtemp(join(tmpdir(), 'locust-photos-'))
const photo = join(elsewhere, 'screenshot.png')
await copyFile(
  localPath(new URL('../docs/user-session/2026-09-08T13-41-08-attach/01-launch.png', import.meta.url)),
  photo
)

const drive = await startDrive({
  name: 'attach-image',
  port: 9391,
  workspace,
  env: { LOCUST_ATTACH_PATHS: photo },
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

try {
  await drive.capture('launch', () => drive.ready())

  await drive.capture('open a mission', () => drive.evaluate(`(async () => {
    ${teammateFace('Wren')}?.click()
    await new Promise(r => setTimeout(r, 700))
    return 'opened'
  })()`))

  await drive.capture('attach a PNG from outside the workspace', () => drive.evaluate(`(async () => {
    document.querySelector('button[data-satellite="attach"]').click()
    // A data: URL has to be built by the host and decoded by the renderer, so
    // this waits longer than a plain state change would need.
    await new Promise(r => setTimeout(r, 2500))
    const thumb = document.querySelector('.lc-attached__tile .lc-thumb')
    if (!thumb) return 'NO THUMBNAIL -- ' + [...document.querySelectorAll('.lc-attached__tile')].map(t => t.textContent?.trim()).join(', ')
    const box = thumb.getBoundingClientRect()
    return JSON.stringify({
      painted: thumb.complete && thumb.naturalWidth > 0,
      naturalSize: thumb.naturalWidth + 'x' + thumb.naturalHeight,
      drawnAt: Math.round(box.width) + 'x' + Math.round(box.height),
      isDataUrl: thumb.src.startsWith('data:image/png;base64,'),
      stillNamesTheFile: document.querySelector('.lc-attached__name')?.textContent
    }, null, 1)
  })()`))

  await drive.capture('a text file beside it draws no picture', () => drive.evaluate(`(async () => {
    // The negative control. A row that painted SOMETHING for every file would
    // pass the check above without the feature working.
    const tiles = document.querySelectorAll('.lc-attached__tile')
    const thumbs = document.querySelectorAll('.lc-attached__tile .lc-thumb')
    return 'tiles: ' + tiles.length + ', thumbnails: ' + thumbs.length + ' (one image attached, so these should read 1 and 1)'
  })()`))

  await drive.capture('the composer, with the picture in it', () => drive.evaluate(`'see the PNG'`))
} finally {
  await drive.finish({
    intro: 'Attaching an image: whether it is drawn as itself rather than named, in the composer and after sending.'
  })
}

say('done')
