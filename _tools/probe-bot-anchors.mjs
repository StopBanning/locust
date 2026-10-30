// Do a bot's presence dot and waiting ring sit on the bot, or on its box?
//
//   node _tools/probe-bot-anchors.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-09-23, with a frame of the home screen's machine: "looks like
// the online dots are off a bit, the square for the blue guy is off a bit".
// Every bot is drawn on a canvas larger than its box (overscan) and lifted
// (rise), while the dot and the ring were placed against the box -- so where
// they land depends on the shape. This reads, for every bot on screen, where
// its body is actually drawn (the canvas's own pixels, the median of twelve
// frames so a bob or a glance does not move the answer), and where its ring
// and dot are, and says how far each is off.
//
// On the bot means: the ring's centre within 4% of the bot's size of the
// body's centre, and the dot's centre within 4% of the body's lower-right
// edge -- the 45-degree point of the ellipse inside its bounds, where an
// avatar's status dot sits (botAnchors.ts). Sends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `bot-anchors-${tag}`)
await mkdir(OUT, { recursive: true })

const mate = (id, name, hue, shape) => ({
  teammateId: id,
  name,
  hue,
  role: 'Code & Migrations',
  avatar: { headwear: 0, accessory: 0, mouth: 0, bot: { shape, face: 'eyes' } },
  createdAt: '2026-09-05T05:00:00.000Z'
})
const workspace = await scratchRepository('locust-bot-anchors-ws-')
const drive = await startDrive({
  name: `bot-anchors-${tag}`,
  port: 9477,
  workspace,
  outPath: OUT,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [mate('tm_wren', 'Wren', 'lime', 'droid'), mate('tm_pip', 'Pip', 'violet', 'ghost'), mate('tm_sable', 'Sable', 'teal', 'hopper')],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

// Every bot's body, ring and dot, in page pixels. No backticks inside: this is a template literal.
const MEASURE = `(async () => {
  const bots = [...document.querySelectorAll('.lc-bot')].filter(b => b.querySelector('canvas'))
  const bodyOf = (canvas) => {
    const context = canvas.getContext('2d')
    if (!context || canvas.width === 0) return null
    const data = context.getImageData(0, 0, canvas.width, canvas.height).data
    let l = canvas.width, t = canvas.height, r = -1, b = -1
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        if (data[(y * canvas.width + x) * 4 + 3] > 40) {
          if (x < l) l = x
          if (x > r) r = x
          if (y < t) t = y
          if (y > b) b = y
        }
      }
    }
    if (r < 0) return null
    const box = canvas.getBoundingClientRect()
    const kx = box.width / canvas.width
    const ky = box.height / canvas.height
    return { l: box.left + l * kx, t: box.top + t * ky, r: box.left + (r + 1) * kx, b: box.top + (b + 1) * ky }
  }
  // A ring pulses by a transform; its laid-out box is what it is anchored by.
  const laidOut = (element) => {
    const width = element.offsetWidth, height = element.offsetHeight
    const parent = element.offsetParent
    const at = parent ? parent.getBoundingClientRect() : { left: 0, top: 0 }
    return { l: at.left + element.offsetLeft, t: at.top + element.offsetTop, r: at.left + element.offsetLeft + width, b: at.top + element.offsetTop + height }
  }
  // The body, the ring and the dot read in the SAME instant: a floating
  // ghost's bob moves all three, and reading them apart measured the bob.
  const samples = bots.map(() => [])
  for (let i = 0; i < 12; i += 1) {
    bots.forEach((bot, index) => {
      const body = bodyOf(bot.querySelector('canvas'))
      if (!body) return
      const ring = bot.querySelector('.lc-bot__ring')
      const dot = bot.querySelector('.lc-presence')
      const dotBox = dot ? dot.getBoundingClientRect() : null
      samples[index].push({
        body,
        ring: ring ? laidOut(ring) : null,
        dot: dotBox ? { cx: (dotBox.left + dotBox.right) / 2, cy: (dotBox.top + dotBox.bottom) / 2, d: dotBox.width } : null
      })
    })
    await new Promise(r => setTimeout(r, 150))
  }
  const median = (values) => {
    const sorted = [...values].sort((a, b) => a - b)
    return sorted[Math.floor(sorted.length / 2)]
  }
  // Everything relative to the body's own top-left in that sample, then the
  // median, then put back on the median body: the bob cancels out.
  const relative = (list, pick) => {
    const own = list.filter(s => pick(s) !== null)
    if (own.length === 0) return null
    const keys = Object.keys(pick(own[0]))
    const out = {}
    for (const key of keys) {
      const across = key === 'l' || key === 'r' || key === 'cx'
      const down = key === 't' || key === 'b' || key === 'cy'
      out[key] = median(own.map(s => pick(s)[key] - (across ? s.body.l : down ? s.body.t : 0)))
    }
    return out
  }
  return JSON.stringify(bots.map((bot, index) => {
    const list = samples[index]
    const body = list.length === 0 ? null : { l: median(list.map(s => s.body.l)), t: median(list.map(s => s.body.t)), r: median(list.map(s => s.body.r)), b: median(list.map(s => s.body.b)) }
    const place = (value) => {
      if (value === null || body === null) return null
      const out = {}
      for (const [key, v] of Object.entries(value)) {
        out[key] = key === 'l' || key === 'r' || key === 'cx' ? v + body.l : key === 't' || key === 'b' || key === 'cy' ? v + body.t : v
      }
      return out
    }
    const ringAt = place(relative(list, s => s.ring))
    const dotAt = place(relative(list, s => s.dot))
    const box = bot.getBoundingClientRect()
    return {
      where: bot.closest('.lc-cover') ? 'cover' : bot.closest('.lc-sidebar, nav, aside') ? 'sidebar' : 'other',
      type: bot.dataset.bot ?? '?',
      size: Math.round(box.width),
      box: { l: box.left, t: box.top, r: box.right, b: box.bottom },
      body,
      anchors: bot.style.cssText.split(';').filter(d => d.includes('--lc-bot-')).map(d => d.trim()).join('; '),
      ring: ringAt,
      dot: dotAt
    }
  }))
})()`

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.waitFor(`document.querySelector('.lc-cover__face .lc-bot[data-bot="hopper"]')?.dataset.state !== 'still'`, { timeoutMs: 60_000, what: 'the cover on' })
  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: true })
  await new Promise((r) => setTimeout(r, 2500))
  const seen = await drive.capture('every bot: its body, its ring, its dot', () => drive.evaluate(MEASURE))
  const bots = JSON.parse(String(seen))
  const rows = []
  for (const bot of bots) {
    if (bot.body === null) continue
    const w = bot.body.r - bot.body.l
    const h = bot.body.b - bot.body.t
    const row = { where: bot.where, type: bot.type, size: bot.size, bodyW: Math.round(w), bodyH: Math.round(h) }
    if (bot.ring !== null) {
      const dx = (bot.ring.l + bot.ring.r) / 2 - (bot.body.l + bot.body.r) / 2
      const dy = (bot.ring.t + bot.ring.b) / 2 - (bot.body.t + bot.body.b) / 2
      row.ring = { dx: Math.round(dx), dy: Math.round(dy) }
      check(`${bot.where} ${bot.type}: the ring is centred on the bot`, Math.hypot(dx, dy) <= 0.04 * bot.size, `off by ${Math.round(dx)}, ${Math.round(dy)} px (bot ${String(bot.size)} px)`)
    }
    if (bot.dot !== null) {
      // The 45-degree point of the ellipse inside the body's bounds: on its lower-right edge.
      const reach = Math.SQRT1_2 / 2
      const wantX = (bot.body.l + bot.body.r) / 2 + reach * w
      const wantY = (bot.body.t + bot.body.b) / 2 + reach * h
      const dx = bot.dot.cx - wantX
      const dy = bot.dot.cy - wantY
      row.dot = { dx: Math.round(dx), dy: Math.round(dy) }
      check(`${bot.where} ${bot.type}: the dot sits on the bot's lower-right edge`, Math.hypot(dx, dy) <= 0.04 * bot.size, `off by ${Math.round(dx)}, ${Math.round(dy)} px (bot ${String(bot.size)} px)`)
    }
    rows.push(row)
  }
  await writeFile(join(OUT, 'anchors.json'), JSON.stringify({ bots, rows }, null, 1))
  say(JSON.stringify(rows))
  say(failures === 0 ? '\nBOT ANCHORS PASSED' : `\nBOT ANCHORS: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Every bot on the home screen and the sidebar: where its body is drawn, and whether its ring and dot sit on it. Sends nothing.' })
}
