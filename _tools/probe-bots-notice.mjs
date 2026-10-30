// A teammate that finishes hops in the sidebar; two that hand a message over
// look at each other. Measured on the built app, off the bots' own pixels.
//
//   node _tools/probe-bots-notice.mjs [--packaged <exe>] [--tag <name>] [--model <words>]
//   LOCUST_SPEND=1 node _tools/probe-bots-notice.mjs --haiku ...
//
// --haiku runs Wren on Claude Haiku instead of the free model (Colin,
// 2026-09-22: Claude and Codex allowed for testing, cheap tiers only), for a
// day the free models do not answer: two short turns. Replies are off either
// way -- the moment is the message being handed over, not Booty's answer --
// so Booty never runs.
//
// Colin's "have fun" list, 2026-09-23, and his answer, "you can run all
// those". Wren is asked, on the free model, to send Booty one message; replies
// are on, so Booty answers by itself. Every bot on screen is sampled every
// 60 ms from before Wren's run ends until after the message has been handed
// over, for two things read off its canvas:
//
//   - how high its drawn body is: the top row with ink, as a share of its
//     size. A hop lifts it by about a fifth; breathing and looking around
//     move it a few hundredths.
//   - where its eyes are: the middle of the face's dark ink across, in canvas
//     pixels from the middle. Looking at a teammate to its right moves it
//     right.
//
// and the host's own moments, from its updates: when Wren's run completed,
// and when the message was posted. Spends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, pickRouteScript, say, scratchRepository, sleep, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const MODEL = arg('--model') ?? 'lightning'
const HAIKU = process.argv.includes('--haiku')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `bots-notice-${tag}`)
await mkdir(OUT, { recursive: true })

const mate = (id, name, hue, shape) => ({
  teammateId: id,
  name,
  hue,
  role: 'Code & Migrations',
  avatar: { headwear: 0, accessory: 0, mouth: 0, bot: { shape, face: 'eyes' } },
  createdAt: '2026-09-05T05:00:00.000Z'
})
const workspace = await scratchRepository('locust-bots-notice-ws-')
const drive = await startDrive({
  name: `bots-notice-${tag}`,
  port: 9451,
  workspace,
  outPath: OUT,
  spends: HAIKU,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [mate('tm_wren', 'Wren', 'lime', 'droid'), mate('tm_booty', 'Booty', 'pearl', 'blob')],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 1, memoryMode: 'off', autoMode: false }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

// Runs in the page until stop(): every bot's body height and eyes, and the
// host's moments, all on one clock.
const SAMPLER = `(() => {
  if (window.__notice) window.__notice.stop()
  const t0 = performance.now()
  const moments = []
  const off = window.desktop.onCodexMissionUpdate((u) => {
    if (u.kind === 'event' && u.event.type === 'run.completed') moments.push({ at: performance.now() - t0, what: 'completed', missionId: u.missionId })
    if (u.kind === 'peer-message') moments.push({ at: performance.now() - t0, what: 'handed', from: u.message.from.name, to: u.message.to.name, direction: u.message.direction })
  })
  const where = (el) => el.closest('.lc-workroom__identity') ? 'header' : el.closest('.lc-faces') ? 'strip' : el.closest('.lc-livestep') ? 'thread' : el.closest('.lc-sidebar') ? 'sidebar' : 'elsewhere'
  const read = (canvas) => {
    const context = canvas.getContext('2d')
    const { width, height } = canvas
    const data = context.getImageData(0, 0, width, height).data
    let top = -1, ink = 0, sumX = 0
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        const i = (y * width + x) * 4
        if (data[i + 3] > 64 && top < 0) top = y
        if (data[i + 3] < 200) continue
        const lum = (0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2]) / 255
        if (lum < 0.16) { ink += 1; sumX += x }
      }
    }
    return { top, ink, eyes: ink === 0 ? null : sumX / ink - width / 2, height }
  }
  const series = {}
  const sample = () => {
    const at = performance.now() - t0
    for (const el of document.querySelectorAll('.lc-bot[data-teammate]')) {
      const canvas = el.querySelector('canvas')
      if (!canvas || canvas.width === 0) continue
      const key = where(el) + ':' + el.dataset.teammate
      const size = parseFloat(el.style.width) || el.getBoundingClientRect().width
      const shown = parseFloat(canvas.style.height) || canvas.getBoundingClientRect().height
      const got = read(canvas)
      const row = series[key] ?? (series[key] = { where: where(el), teammate: el.dataset.teammate, samples: [] })
      row.samples.push({ at, activity: el.dataset.activity, top: got.top < 0 ? null : (got.top / (got.height / shown)) / size, eyes: got.eyes })
    }
  }
  const timer = setInterval(sample, 60)
  window.__notice = { peek: () => moments, stop: () => { clearInterval(timer); off?.(); return { series, moments } } }
  return 'sampling'
})()`

try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: true })
  await drive.evaluate(`(async () => { ${teammateFace('Wren')}?.click(); await new Promise((r) => setTimeout(r, 1000)) })()`)
  await drive.evaluate(HAIKU
    ? pickRouteScript({ group: '/claude/i', search: 'haiku', row: '/haiku/i' })
    : pickRouteScript({ group: '/opencode/i', search: MODEL, row: `/${MODEL}/i` }))
  const route = String(await drive.evaluate(`(() => {
    const c = [...document.querySelectorAll('button.lc-control')].find((b) => (b.textContent ?? '').includes('/'))
    return (c?.textContent ?? '').trim()
  })()`))
  // The route is asserted BEFORE anything is sent.
  if (HAIKU ? !/haiku/i.test(route) : !/opencode/i.test(route) || !/free/i.test(route)) {
    throw new Error(`route is ${JSON.stringify(route)} -- refusing anything but ${HAIKU ? 'Claude Haiku' : 'a free OpenCode model'}`)
  }
  say(`route: ${route}`)
  const send = (text) => drive.evaluate(`(async () => {
    const field = document.querySelector('textarea[aria-label="Message"]')
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    set.call(field, ${JSON.stringify(text)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 250))
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    return 'sent'
  })()`)
  const until = async (what, limitMs) => {
    for (let waited = 0; waited < limitMs; waited += 1000) {
      await sleep(1000)
      const seen = JSON.parse(String(await drive.evaluate(`JSON.stringify(window.__notice.peek())`)))
      if (seen.filter((moment) => moment.what === what).length > 0) return true
    }
    return false
  }
  say(await drive.evaluate(SAMPLER))
  // A short turn first: its end is the hop.
  await drive.capture('Wren asked for one word', () => send('Reply with the single word HELLO and nothing else. Do not use any tools.'))
  const finished = await until('completed', 300_000)
  say(`Wren's first turn ${finished ? 'completed' : 'did not complete in five minutes'}`)
  await sleep(2500)
  // Then the handoff, written out so the model only has to say it: the host
  // posts a share block from a reply, and replies are on, so Booty answers.
  // The share block is the one the teammates' briefing teaches.
  // A real request, because it is one: asked to "reply with exactly" a block,
  // Haiku once refused it as arbitrary text in the share form.
  await drive.capture('Wren asked to hand Booty a message', () => send('Ask your teammate Booty, with a share block to Booty, to reply with exactly the word TANGERINE. Do not use any tools, and say nothing else.'))
  const handedOver = await until('handed', 300_000)
  say(`the message ${handedOver ? 'was handed over' : 'was not handed over in five minutes'}`)
  say(`Wren said: ${String(await drive.evaluate(`[...document.querySelectorAll('.lc-thread .lc-agentline__body')].map((n) => n.textContent.trim()).join(' | ').slice(-300)`))}`)
  await sleep(6000)
  const { series, moments } = JSON.parse(String(await drive.evaluate(`JSON.stringify(window.__notice.stop())`)))
  await writeFile(join(OUT, 'samples.json'), JSON.stringify({ series, moments }, null, 1))
  say(`moments: ${JSON.stringify(moments)}`)

  const completed = moments.find((moment) => moment.what === 'completed')
  const handed = moments.find((moment) => moment.what === 'handed' && moment.direction === 'posted' && moment.from === 'Wren')
  const within = (row, from, to) => (row?.samples ?? []).filter((sample) => sample.at >= from && sample.at <= to)
  const lift = (row, from, to) => {
    const window = within(row, from, to).filter((sample) => sample.top !== null)
    const rest = within(row, from - 1500, from).filter((sample) => sample.top !== null)
    if (window.length === 0 || rest.length === 0) return null
    const resting = rest.map((sample) => sample.top).sort((a, b) => a - b)[Math.floor(rest.length / 2)]
    return Number((resting - Math.min(...window.map((sample) => sample.top))).toFixed(3))
  }
  if (completed === undefined) {
    check("Wren's run completed", false, 'no run.completed seen')
  } else {
    const strip = series['strip:tm_wren']
    const header = series['header:tm_wren']
    const stripLift = lift(strip, completed.at, completed.at + 1900)
    const headerLift = lift(header, completed.at, completed.at + 1900)
    say(`after Wren finished: the strip's Wren lifted ${String(stripLift)} of its size, the header's ${String(headerLift)}`)
    check("Wren's face in the sidebar hops once when the run ends", stripLift !== null && stripLift >= 0.1, String(stripLift))
    check('the header over the reply does not hop', headerLift !== null && headerLift < 0.05, String(headerLift))
  }
  if (handed === undefined) {
    say('  (no message was handed over: the free model did not use the share block form -- the glance is unmeasured this run)')
    check('Wren handed Booty a message (the model used the share block form)', false, JSON.stringify(moments))
  } else {
    const eyes = (key) => {
      const window = within(series[key], handed.at + 300, handed.at + 1400).map((sample) => sample.eyes).filter((value) => value !== null)
      return window.length === 0 ? null : Number((window.reduce((sum, value) => sum + value, 0) / window.length).toFixed(2))
    }
    const order = await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-faces .lc-bot[data-teammate]')].map((el) => el.dataset.teammate))`)
    const line = JSON.parse(String(order))
    const wrenLeft = line.indexOf('tm_wren') < line.indexOf('tm_booty')
    const wren = eyes('strip:tm_wren')
    const booty = eyes('strip:tm_booty')
    say(`the strip, left to right: ${line.join(', ')}; eyes across while the message is handed: Wren ${String(wren)}, Booty ${String(booty)} canvas px from the middle`)
    const toward = (value, right) => value !== null && (right ? value >= 1.2 : value <= -1.2)
    check('Wren looks toward Booty', toward(wren, wrenLeft), String(wren))
    check('and Booty looks back at Wren', toward(booty, !wrenLeft), String(booty))
  }
  say(failures === 0 ? '\nBOTS NOTICE PASSED' : `\nBOTS NOTICE: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Wren asked to hand Booty a message on the free model, replies on; every bot sampled from its pixels. Nothing was spent.' })
}
