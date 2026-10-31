// Settings › Your account in the built app, signed out, in a throwaway profile. No model calls, nothing sent.
// The signed-in states need a relay and a GitHub sign-in: the two-profile drive against `wrangler dev` (slice 6).
// Negative control: a build without the card has no [data-setting="your-account"] and fails the first check.
import { say, scratchRepository, startDrive } from './drive-lib.mjs'
const drive = await startDrive({ name: 'your-account', port: 9896, workspace: await scratchRepository('locust-your-account-'), sendsNothing: true, holdCursor: false, keep: true })
const section = `document.querySelector('[data-setting="your-account"]')`
let failed = 0
const check = (name, ok) => { if (!ok) failed += 1; say(`[${ok ? 'PASS' : 'FAIL'}] ${name}`) }
const wait = async (expression, seconds = 60) => {
  for (let n = 0; n < seconds * 4; n += 1) {
    if (await drive.evaluate(expression)) return
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  throw Error(`Timed out: ${expression}`)
}
try {
  await drive.ready()
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
  await wait(`!!document.querySelector('.lc-settings__navitem')`)
  await drive.evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].find(b => b.innerText.trim() === 'General')?.click()`)
  await wait(`!!${section}`)
  await wait(`${section}.innerText.includes('Sign in once')`)
  await drive.evaluate(`${section}.scrollIntoView({ block: 'start' })`)
  const card = await drive.capture('01. Your account, signed out', () => drive.evaluate(`({ text: ${section}.innerText, switches: ${section}.querySelectorAll('[role="switch"]').length, next: ${section}.nextElementSibling?.getAttribute('data-setting') })`))
  check('the card is headed Your account and says what an account does', card.text.startsWith('Your account') && card.text.includes('follow you to any computer where you sign in'))
  check('it says where to sign in, and offers no switch before a sign-in', card.text.includes('Sign in with GitHub there') && card.switches === 0)
  check('it sits just above Your phone, where the sign-in is', card.next === 'phone')
} catch (error) { failed += 1; say(error instanceof Error ? error.message : String(error)) }
finally { await drive.finish({ intro: 'Your account, signed out, in the built app. Nothing sent.' }); say(failed === 0 ? 'ALL CHECKS PASSED' : `${failed} FAILED`); process.exitCode = failed === 0 ? 0 : 1 }
