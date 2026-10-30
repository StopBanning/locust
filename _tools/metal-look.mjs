// A close photograph of the send button's metal, awake.
//
//   node _tools/metal-look.mjs <name>
//
// Whatever `metal.ts` is currently set to — build first, then run this. It
// exists because the strength and preset are exactly the kind of setting that
// gets argued about from memory, and the argument is settled by looking.
//
// Three things it does that an ordinary drive capture does not:
//
//   it TYPES first, because an empty composer disables the send button and a
//   disabled button fires no pointer events at all;
//   it hovers with a REAL pointer, parked elsewhere first, because an enter
//   is a transition and a page whose cursor has never been anywhere has
//   nothing to transition from;
//   it waits for `data-paused` to be ABSENT, which is how this library says
//   "awake" — it writes "true" or nothing, never "false".
//
// Then it clips the screenshot to the button rather than the window, so the
// thing being judged is the thing in the picture.
//
// Free: sends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const NAME = (process.argv[2] ?? 'now').replace(/[^a-z0-9.-]+/gi, '-')
const OUT = localPath(new URL('../docs/chain-measure/', import.meta.url))

const workspace = await scratchRepository('locust-metal-look-ws-')
const drive = await startDrive({ name: 'metal-look', port: 9527, workspace, spends: false })

try {
  await drive.capture('launch', () => drive.ready())

  await drive.capture('wake the send button and photograph it', async () => {
    await drive.evaluate(`(() => {
      const field = document.querySelector('form.command-dock textarea')
      const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
      set.call(field, 'hello')
      field.dispatchEvent(new Event('input', { bubbles: true }))
      return true
    })()`)
    await drive.waitFor(`document.querySelector('form.command-dock .lc-send')?.disabled === false`, {
      what: 'the send button to become pressable',
      timeoutMs: 8_000
    })
    const box = JSON.parse(await drive.evaluate(`(() => {
      const b = document.querySelector('form.command-dock .lc-send').getBoundingClientRect()
      return JSON.stringify({ x: b.left, y: b.top, w: b.width, h: b.height })
    })()`))

    await drive.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 10, y: 10, buttons: 0 })
    await new Promise((r) => setTimeout(r, 200))
    await drive.send('Input.dispatchMouseEvent', {
      type: 'mouseMoved',
      x: Math.round(box.x + box.w / 2),
      y: Math.round(box.y + box.h / 2),
      buttons: 0
    })
    const awake = await drive
      .waitFor(`document.querySelector('form.command-dock .metal-fx-root')?.hasAttribute('data-paused') === false`, {
        what: 'the shader to wake',
        timeoutMs: 8_000
      })
      .catch(() => false)
    // A moment of actual animation, so the frame is the effect running rather
    // than its first frame.
    await new Promise((r) => setTimeout(r, 900))

    // Generous margin: the ring and its glow live outside the button's box.
    const pad = 26
    const shot = await drive.send('Page.captureScreenshot', {
      format: 'png',
      captureBeyondViewport: false,
      clip: {
        x: Math.max(0, box.x - pad),
        y: Math.max(0, box.y - pad),
        width: box.w + pad * 2,
        height: box.h + pad * 2,
        scale: 6
      }
    })
    await mkdir(OUT, { recursive: true })
    const file = join(OUT, `metal-${NAME}.png`)
    if (shot?.result?.data) await writeFile(file, Buffer.from(shot.result.data, 'base64'))
    say(`   awake: ${String(awake)} — ${file}`)
    return `awake ${String(awake)} · ${file}`
  })
} catch (error) {
  say(`failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `A close photograph of the send button's metal, awake. Setting: ${NAME}.` })
}
