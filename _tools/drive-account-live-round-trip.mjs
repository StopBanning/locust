// LIVE, against Locust's real relay at app.locust.lol: two throwaway Locusts on this computer, "PC" and "Mac", each
// signed in with the person's GitHub through the sign-in links this prints (they open on a phone or any browser).
// Then, with no one touching anything: the PC keeps a copy and syncs; the Mac is told a copy exists, brings it here
// with the PC's recovery code, and restarts; the restart applies it (the PC's teammate replaces the Mac's). Last, the
// copy is deleted, both sign out of GitHub and turn From anywhere off, so the relay forgets both rooms.
// No model calls. Links are written to LOCUST_LIVE_LINKS (default: live-links.txt here) as soon as they exist.
import { execFileSync } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { say, scratchRepository, startDrive } from './drive-lib.mjs'

const LINKS = process.env.LOCUST_LIVE_LINKS ?? join(process.cwd(), 'live-links.txt')
// How the Mac gets the key: approved from the PC (slice 4b, the default) or with the recovery code (LOCUST_LIVE_WAY=code).
const WAY = process.env.LOCUST_LIVE_WAY === 'code' ? 'code' : 'approve'
const LINK_ROUND_MS = 9.5 * 60_000
const ROUNDS = 3
let failed = 0
const check = (name, ok) => { if (!ok) failed += 1; say(`[${ok ? 'PASS' : 'FAIL'}] ${name}`) }
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const teammate = (id, name, hue) => ({ teammateId: id, name, hue, role: 'Custom', roleTitle: 'Live test', createdAt: '2026-10-31T12:00:00.000Z', route: { runtime: 'opencode', model: 'account-default', mode: 'ask' } })
const seedOf = (one) => ({ schemaVersion: 1, teammates: [one], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } })
const json = async (drive, call) => {
  const expression = `window.desktop.phone.${call}`
  return JSON.parse(await drive.evaluate(`(${expression}).then((answer) => JSON.stringify(answer))`))
}

/** The relaunched window, which no drive started: Chrome's own debugging line on the same port. */
async function cdpEvaluate(port, expression) {
  const targets = await (await fetch(`http://127.0.0.1:${String(port)}/json`)).json()
  for (const target of targets.filter((one) => one.type === 'page' && typeof one.webSocketDebuggerUrl === 'string')) {
    const socket = new WebSocket(target.webSocketDebuggerUrl)
    await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
    const reply = await new Promise((resolve) => {
      socket.onmessage = (message) => { const data = JSON.parse(String(message.data)); if (data.id === 1) resolve(data) }
      socket.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: `(async () => { if (window.desktop?.phone === undefined) return undefined; return JSON.stringify(await (${expression})) })()`, awaitPromise: true, returnByValue: true } }))
    })
    socket.close()
    const value = reply.result?.result?.value
    if (typeof value === 'string') return JSON.parse(value)
  }
  return undefined
}
const processesOf = (profile) => { try { return execFileSync('pgrep', ['-f', `user-data-dir=${profile}`], { encoding: 'utf8' }).split('\n').filter(Boolean) } catch { return [] } }

const pages = { pc: join(await scratchRepository('locust-live-pages-'), 'pc.txt'), mac: join(await scratchRepository('locust-live-pages-'), 'mac.txt') }
const pc = await startDrive({ name: 'account-live-pc', port: 9898, workspace: await scratchRepository('locust-live-pc-'), seed: seedOf(teammate('tm_wrenlive0000000000000', 'Wren', 'lime')), env: { LOCUST_SIGN_IN_PAGES: pages.pc }, sendsNothing: true, holdCursor: false, keep: true })
// One launch at a time (drive-lib's launch turn): the Mac starts once the PC is ready.
await pc.ready()
const mac = await startDrive({ name: 'account-live-mac', port: 9899, workspace: await scratchRepository('locust-live-mac-'), seed: seedOf(teammate('tm_atlaslive000000000000', 'Atlas', 'blue')), env: { LOCUST_SIGN_IN_PAGES: pages.mac }, sendsNothing: true, holdCursor: false, keep: true })
let macRelaunched = false
try {
  await mac.ready()
  for (const [name, drive] of [['PC', pc], ['Mac', mac]]) {
    await drive.evaluate(`window.desktop.phone.setMode('anywhere').then(() => true)`)
    let connected = false
    for (let n = 0; n < 80 && !connected; n += 1) { connected = await drive.evaluate(`window.desktop.phone.anywhere().then((s) => s.connected === true)`); if (!connected) await pause(250) }
    check(`${name}: From anywhere reaches the live relay`, connected)
    if (!connected) throw Error(`${name} did not reach the relay`)
  }

  // The person's part: two sign-ins. A link lasts ten minutes, so a new pair is made until both are signed in.
  const linkOf = async (drive, file) => {
    const said = await json(drive, 'githubSignIn()')
    if (said.message !== undefined) say(`sign-in said: ${said.message}`)
    return (await readFile(file, 'utf8')).trim().split('\n').pop()
  }
  const login = (drive) => drive.evaluate(`window.desktop.phone.anywhere().then((s) => s.github?.login ?? '')`)
  let signedIn = false
  for (let round = 1; round <= ROUNDS && !signedIn; round += 1) {
    const pcLink = (await login(pc)) === '' ? await linkOf(pc, pages.pc) : undefined
    const macLink = (await login(mac)) === '' ? await linkOf(mac, pages.mac) : undefined
    await writeFile(LINKS, `${pcLink === undefined ? '' : `PC: ${pcLink}\n`}${macLink === undefined ? '' : `Mac: ${macLink}\n`}round ${String(round)}, good until ${new Date(Date.now() + 10 * 60_000).toLocaleTimeString('en-US')}\n`, 'utf8')
    say(`LINKS READY (round ${String(round)}) in ${LINKS}`)
    const until = Date.now() + LINK_ROUND_MS
    while (Date.now() < until && !signedIn) {
      signedIn = (await login(pc)) !== '' && (await login(mac)) !== ''
      if (!signedIn) await pause(3000)
    }
  }
  check('both throwaway Locusts are signed in with GitHub', signedIn)
  if (!signedIn) throw Error('Nobody signed in within the links’ time')
  const account = await login(pc)
  check('both are signed in as the same account', account === (await login(mac)))
  await writeFile(LINKS, `Signed in as ${account}. Running the round trip now.\n`, 'utf8')

  // The PC keeps a copy.
  await pc.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  const on = await json(pc, 'accountSyncOn()')
  check('PC: Keep a copy turns on and gives the recovery code once', on.status?.sync === 'on' && /^([0-9A-Z]{5}-){10}[0-9A-Z]{5}$/.test(on.code ?? ''))
  const synced = await json(pc, 'accountSyncNow()')
  check('PC: the copy reaches the live relay', synced.lastSynced?.files > 0 && synced.message === 'Synced.')
  check('PC: the card says how much of the account’s storage it uses', synced.storage?.used > 0 && synced.storage?.quota >= 1024 * 1024 * 1024)
  say(`PC synced ${String(synced.lastSynced?.files)} files, ${String(synced.lastSynced?.bytes)} bytes; relay holds ${String(synced.storage?.used)} bytes`)
  await pc.evaluate(`document.querySelector('[data-setting="your-account"]')?.scrollIntoView({ block: 'start' })`)
  await pc.capture('01. PC: keeping a copy on the live relay', () => pc.evaluate(`document.querySelector('[data-setting="your-account"]')?.innerText ?? ''`))

  // The Mac is told, and brings it here with the code.
  await mac.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  let elsewhere
  for (let n = 0; n < 90 && elsewhere === undefined; n += 1) { elsewhere = (await json(mac, 'account()')).elsewhere; if (elsewhere === undefined) await pause(2000) }
  check('Mac: told another computer keeps a copy', elsewhere?.computers === 1)
  await mac.evaluate(`document.querySelector('[data-setting="your-account"]')?.scrollIntoView({ block: 'start' })`)
  await pause(500)
  await mac.capture('02. Mac: offered the copy, before the code', () => mac.evaluate(`document.querySelector('[data-setting="your-account"]')?.innerText ?? ''`))
  const wrong = await json(mac, `accountBringHere(${JSON.stringify(on.code.replace(/^(..)(.)/, (_, head, third) => head + (third === '0' ? '1' : '0')))})`)
  check('Mac: a code copied wrong changes nothing', wrong.ok === false && /does not read right/.test(wrong.reason))
  if (WAY === 'code') {
    const brought = await json(mac, `accountBringHere(${JSON.stringify(on.code)})`)
    check('Mac: the code opens the PC’s copy and a restart is asked for', brought.ok === true)
    macRelaunched = brought.ok === true
  } else {
    const asked = await json(mac, 'accountAsk()')
    const match = asked.asking?.match ?? ''
    check('Mac: asks the PC, and shows a code to compare', /^[0-9A-Z]{4}-[0-9A-Z]{4}$/.test(match))
    await pause(6000)
    await mac.capture('03. Mac: asking, with the code to compare', () => mac.evaluate(`document.querySelector('[data-setting="your-account"]')?.innerText ?? ''`))
    let seen
    for (let n = 0; n < 60 && seen === undefined; n += 1) { seen = (await json(pc, 'account()')).asks?.[0]; if (seen === undefined) await pause(2000) }
    check('PC: shows the Mac’s ask with the same code', seen?.match === match && typeof seen?.name === 'string')
    await pause(6000)
    await pc.evaluate(`document.querySelector('[data-setting="your-account"]')?.scrollIntoView({ block: 'start' })`)
    await pc.capture('04. PC: the Mac’s ask, with Approve and Not mine', () => pc.evaluate(`document.querySelector('[data-setting="your-account"]')?.innerText ?? ''`))
    const approved = await json(pc, `accountApprove(${JSON.stringify(seen?.room ?? '')}, ${JSON.stringify(match)})`)
    check('PC: approving sends the key, sealed to the Mac', approved.good === true)
    // The Mac's own look finds the grant within seconds, brings the copy in and restarts by itself.
    macRelaunched = approved.good === true
  }

  // The restart applies it, before any store is read.
  let applied = false
  for (let n = 0; n < 60 && !applied; n += 1) {
    await pause(1000)
    applied = await readFile(join(mac.profile, 'teammates.json'), 'utf8').then((text) => text.includes('Wren') && !text.includes('Atlas'), () => false)
  }
  check('Mac, after its restart: the PC’s teammate replaced its own', applied)
  // What it replaced is moved aside, not deleted. (The restore's note is taken by the restarted window as it opens, so
  // it is gone from disk by now: its absence with no restore still pending is the window having been told.)
  const { readdir } = await import('node:fs/promises')
  const aside = (await readdir(mac.profile)).filter((name) => name.startsWith('before-restore-'))
  const kept = aside.length === 1 ? await readFile(join(mac.profile, aside[0], 'teammates.json'), 'utf8').catch(() => '') : ''
  check('Mac: its own teammate was moved aside, not deleted', kept.includes('Atlas'))
  const pending = await readFile(join(mac.profile, 'pending-restore.json'), 'utf8').then(() => true, () => false)
  check('Mac: no restore is left pending', !pending)
  let after
  for (let n = 0; n < 60 && after === undefined; n += 1) { after = await cdpEvaluate(9899, 'window.desktop.phone.account()').catch(() => undefined); if (after === undefined) await pause(1000) }
  check('Mac, restarted: still signed in, sync off, the key kept (no new code next time)', after?.signedIn?.login === account && after?.sync === 'off' && after?.hasKey === true)
} catch (error) { failed += 1; say(error instanceof Error ? error.message : String(error)) }
finally {
  // Clean up at the relay: the copy, both GitHub ties, both rooms.
  const cleanup = []
  try {
    const deleted = await json(pc, 'accountDelete()')
    cleanup.push(`PC delete: ${String(deleted.message)}`)
    await json(pc, 'githubSignOut()').catch(() => undefined)
    await pc.evaluate(`window.desktop.phone.setMode('off').then(() => true)`).catch(() => undefined)
    cleanup.push('PC: signed out, From anywhere off')
  } catch (error) { cleanup.push(`PC cleanup failed: ${String(error)}`) }
  try {
    if (macRelaunched) {
      await cdpEvaluate(9899, 'window.desktop.phone.githubSignOut()')
      await cdpEvaluate(9899, "window.desktop.phone.setMode('off')")
    } else {
      await json(mac, 'githubSignOut()').catch(() => undefined)
      await mac.evaluate(`window.desktop.phone.setMode('off').then(() => true)`).catch(() => undefined)
    }
    cleanup.push('Mac: signed out, From anywhere off')
  } catch (error) { cleanup.push(`Mac cleanup failed: ${String(error)}`) }
  for (const line of cleanup) say(line)
  await writeFile(LINKS, `Done. ${failed === 0 ? 'All checks passed.' : `${String(failed)} failed.`}\n${cleanup.join('\n')}\n`, 'utf8').catch(() => undefined)
  await pc.finish({ intro: 'Live round trip: the PC keeps a copy on the real relay.' })
  await mac.finish({ intro: 'Live round trip: the Mac brings it here with the code.' }).catch(() => undefined)
  // The relaunched Mac was started by the app itself, not this drive: ended here.
  for (const pid of processesOf(mac.profile)) { try { process.kill(Number(pid)) } catch { /* gone */ } }
  say(failed === 0 ? 'ALL CHECKS PASSED' : `${String(failed)} FAILED`)
  process.exitCode = failed === 0 ? 0 : 1
}
