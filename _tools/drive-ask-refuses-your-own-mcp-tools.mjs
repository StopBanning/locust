// Ask on OpenCode runs none of the person's own MCP tools, and Approve each asks for them on a card (0.821).
//
//   node _tools/drive-ask-refuses-your-own-mcp-tools.mjs [--packaged <exe>] [--tag <name>]
//
// Free model (LOCUST_FREE_MODEL, DeepSeek V4.1 Flash by default): spends nothing.
//
// OpenCode allows any tool its config does not name, and a person's own MCP servers load in every run: measured
// 2026-10-22 on 1.18.27, a `save_note` tool from an MCP server in the person's global OpenCode config ran in Locust's
// Ask without asking and wrote note.txt into the folder; in Approve each it ran with no card. This drive gives OpenCode
// a global config of its own (XDG_CONFIG_HOME, for this app only -- the person's own OpenCode settings are never read
// or touched) whose one MCP server writes note.txt when its tool is called. Ada (Ask) is asked to call it: nothing is
// written. Ada then reads a file: reading still works. Bea (Approve each) is asked to call it: a card stops it, and
// note.txt is written once it is approved.
//
// On a build before 0.821 the first and last checks FAIL (the control).

import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { git, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const MODEL = process.env.LOCUST_FREE_MODEL ?? 'opencode/deepseek-v4.1-flash-free'
const OUT = join(recordRoot('ask-refuses-your-own-mcp-tools-2026-10-22'), tag)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-drive-ask-mcp-ws-')
await writeFile(join(workspace, 'calc.py'), 'def add(a, b):\n    return a + b\n', 'utf8')
await git(['add', 'calc.py'], workspace)
await git(['commit', '-q', '-m', 'add calc'], workspace)
const note = join(workspace, 'note.txt')

// OpenCode's global config for this app only, with the person-like MCP server in it.
const xdg = await mkdtemp(join(tmpdir(), 'locust-drive-ask-mcp-config-'))
const server = join(xdg, 'notes-server.mjs')
await writeFile(server, [
  "import { writeFileSync } from 'node:fs'",
  `const NOTE = ${JSON.stringify(note)}`,
  "const send = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\\n')",
  "let buffer = ''",
  "process.stdin.setEncoding('utf8')",
  "process.stdin.on('data', (chunk) => {",
  "  buffer += chunk",
  "  let at",
  "  while ((at = buffer.indexOf('\\n')) >= 0) {",
  "    const line = buffer.slice(0, at).trim(); buffer = buffer.slice(at + 1)",
  "    if (!line) continue",
  "    const rpc = JSON.parse(line)",
  "    if (rpc.method === 'initialize') send(rpc.id, { protocolVersion: rpc.params?.protocolVersion ?? '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'notes', version: '1.0.0' } })",
  "    else if (rpc.method === 'tools/list') send(rpc.id, { tools: [{ name: 'save_note', description: 'Save a note to note.txt in the project.', inputSchema: { type: 'object', properties: { text: { type: 'string' } }, required: ['text'] } }] })",
  "    else if (rpc.method === 'tools/call') { writeFileSync(NOTE, String(rpc.params?.arguments?.text ?? '')); send(rpc.id, { content: [{ type: 'text', text: 'Saved.' }] }) }",
  "    else if (rpc.id !== undefined) send(rpc.id, {})",
  "  }",
  "})",
  ''
].join('\n'), 'utf8')
await mkdir(join(xdg, 'opencode'), { recursive: true })
await writeFile(join(xdg, 'opencode', 'opencode.json'), JSON.stringify({ $schema: 'https://opencode.ai/config.json', mcp: { notes: { type: 'local', command: [process.execPath, server], enabled: true } } }, null, 2), 'utf8')

const drive = await startDrive({
  name: `ask-mcp-${tag}`, port: 9947, workspace, outPath: OUT, launchElsewhere: true, env: { XDG_CONFIG_HOME: xdg },
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-10-22T01:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'ask' } },
      { teammateId: 'tm_bea', name: 'Bea', hue: 'lime', role: 'Docs & QA', createdAt: '2026-10-22T01:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'approve-each' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

const ASK_FOR_THE_NOTE = 'Call the notes server\'s save_note tool once, with the text "hello from Locust". Then say in one sentence whether it worked.'
/** Send what is typed, and wait for the first card -- or for the run to end without one. */
const sendUntilCard = (text) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 400))
  document.querySelector('button[aria-label="Send"]')?.click()
  for (let i = 0; i < 600; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    const approval = document.querySelector('[role=group][aria-label="Approval required"]')
    if (approval) { await new Promise((r) => setTimeout(r, 700)); return JSON.stringify({ text: approval.innerText.split(/\\s+/).join(' ').slice(0, 400) }) }
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) break
  }
  return JSON.stringify({ text: '' })
})()`
/** Send what is typed (nothing, to go on with a run already waiting), answer every card with Approve once, and wait for the end. */
const sendAnswering = (text) => `(async () => {
  if (${JSON.stringify(text ?? null)} !== null) {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(text ?? '')})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 400))
    document.querySelector('button[aria-label="Send"]')?.click()
  }
  const cards = []
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    const approval = document.querySelector('[role=group][aria-label="Approval required"]')
    if (approval) {
      // A new card takes no approval for its first half second (ApprovalCard, FRESH_CARD_MS).
      await new Promise((r) => setTimeout(r, 700))
      cards.push((approval.querySelector('.lc-approval__detail')?.innerText ?? approval.innerText).split(/\\s+/).join(' ').slice(0, 120))
      const button = [...approval.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))
      if (!button) break
      button.click()
      await new Promise((r) => setTimeout(r, 900))
      continue
    }
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) break
  }
  await new Promise((r) => setTimeout(r, 2500))
  return JSON.stringify({ cards, thread: (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-1500) })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2000)

  await drive.evaluate(openTeammateScript('Ada'))
  await sleep(800)
  const asked = JSON.parse(String(await drive.capture('Ada in Ask, asked to call the person\'s own MCP tool', () => drive.evaluate(sendAnswering(ASK_FOR_THE_NOTE)))))
  check('Ask ran none of it: note.txt was not written', !existsSync(note), `${existsSync(note) ? `WRITTEN: ${JSON.stringify(await readFile(note, 'utf8'))}` : 'not written'} || ${asked.thread.slice(-260)}`)
  check('and asked nobody about it: Ask has no cards', asked.cards.length === 0, JSON.stringify(asked.cards))

  const read = String(await drive.capture('Ada still reads', () => drive.evaluate(sendAndWaitScript('Read calc.py with your read tool and quote its second line exactly.', { waitSeconds: 240 }))))
  check('reading still works in Ask', /return a \+ b/.test(read), read.slice(-260))

  await rm(note, { force: true })
  await drive.evaluate(openTeammateScript('Bea'))
  await sleep(800)
  // The card is pictured before it is answered: it is new in 0.821, and what it says is the person's to judge.
  const card = JSON.parse(String(await drive.capture('Bea in Approve each: the card for the person\'s own MCP tool', () => drive.evaluate(sendUntilCard(ASK_FOR_THE_NOTE)))))
  check('Approve each stopped for it on a card that names the tool', /save_note/i.test(card.text), card.text || 'no card')
  check('and nothing was written before the press', !existsSync(note), existsSync(note) ? 'WRITTEN' : 'not written')
  const approved = JSON.parse(String(await drive.capture('approved once, it ran', () => drive.evaluate(sendAnswering(undefined)))))
  check('and it ran once approved: note.txt was written', existsSync(note), approved.thread.slice(-260))
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. Ada (Ask) and Bea (Approve each) on OpenCode / ${MODEL}; OpenCode's global config is the drive's own, with one MCP server whose save_note writes note.txt.`, extra: `Checks failed: ${String(failures)}` })
  await rm(xdg, { recursive: true, force: true }).catch(() => undefined)
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
