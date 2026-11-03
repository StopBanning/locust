// Reads the send button's state and paint with words typed in the chat box: disabled or not, its fill, its glow,
// and what covers it. For the polish pass's send-button change (0.863). Spends nothing.
//   node _tools/probe-send-button.mjs
import { say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const drive = await startDrive({
  name: 'probe-send-button',
  port: 9467,
  workspace: await scratchRepository('locust-probe-send-'),
  sendsNothing: true,
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
try {
  say(String(await drive.ready()))
  await sleep(1500)
  const typed = await drive.evaluate(`(() => {
    const box = document.querySelector('.lc-composer__box textarea')
    if (!box) return 'no box'
    box.focus()
    document.execCommand('insertText', false, 'Plan the release checklist for Friday')
    return box.value
  })()`)
  say(`typed: ${String(typed)}`)
  await sleep(600)
  say(String(await drive.evaluate(`(() => {
    const send = document.querySelector('.lc-composer__controls .lc-send')
    if (!send) return 'no send'
    const style = getComputedStyle(send)
    const box = send.getBoundingClientRect()
    const top = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
    return JSON.stringify({ disabled: send.disabled, classes: send.className, background: style.backgroundColor, shadow: style.boxShadow.slice(0, 60), color: style.color, opacity: style.opacity, topmost: top === null ? null : top.className || top.tagName, parent: send.parentElement?.className })
  })()`)))
} finally {
  await drive.finish({ title: 'Send button probe', intro: 'The send button with words typed.' })
}
