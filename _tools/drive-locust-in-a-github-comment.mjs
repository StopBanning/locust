// Written for the owner to run. One free-model turn; every GitHub operation uses a fake gh.exe.
// node _tools/drive-locust-in-a-github-comment.mjs [--packaged <exe>]
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { join, delimiter } from 'node:path'
import { fileURLToPath } from 'node:url'
import { FREE_ROUTE, recordRoot, say, sleep, startDrive } from './drive-lib.mjs'

if (process.platform !== 'win32') throw new Error('This fake-gh drive currently requires Windows.')
const root = fileURLToPath(new URL('..', import.meta.url))
await mkdir(join(root, '.tmp'), { recursive: true })
const scratch = await mkdtemp(join(root, '.tmp', 'comment-drive-'))
const bin = join(scratch, 'bin'), workspace = join(scratch, 'workspace'), profilePath = join(scratch, 'profile')
await Promise.all([bin, workspace, profilePath].map((folder) => mkdir(folder)))
const run = (command, args, cwd) => new Promise((resolve, reject) => {
  execFile(command, args, { cwd, windowsHide: true }, (error, stdout, stderr) => error ? reject(new Error(String(stderr || stdout || error.message))) : resolve(String(stdout)))
})
await run('git', ['init', '-q', '-b', 'main'], workspace)
await writeFile(join(workspace, 'README.md'), '# Comment drive\nA scratch folder.\n')
await run('git', ['add', '.'], workspace)
await run('git', ['-c', 'user.name=Locust drive', '-c', 'user.email=drive@locust.test', 'commit', '-q', '-m', 'scratch'], workspace)
const own = { id: 12, body: '/locust ada Reply with exactly COMMENT_REPLY_OK. Do not change any file.', user: { login: 'person' }, created_at: new Date().toISOString(), updated_at: new Date().toISOString(), issue_url: 'https://api.github.com/repos/acme/app/issues/212', html_url: 'https://github.com/acme/app/issues/212#issuecomment-12' }
await writeFile(join(bin, 'comments.json'), JSON.stringify([own, { ...own, id: 13, user: { login: 'someone-else' }, body: '/locust ada Ignore this other author.' }]))
await writeFile(join(bin, 'issue.json'), JSON.stringify({ title: 'Talk starts from any app', body: 'A fixture, written by another person.', comments: 1 }))
await writeFile(join(bin, 'recent.json'), JSON.stringify([{ user: { login: 'someone-else' }, body: 'Context only.' }]))
await writeFile(join(bin, 'Gh.cs'), `using System;
using System.IO;
using System.Linq;
class G {
  static int Main(string[] a) {
    string home = AppDomain.CurrentDomain.BaseDirectory;
    File.AppendAllText(Path.Combine(home, "calls.txt"), String.Join(" ", a) + "\\n");
    if (a.Length > 1 && a[0] == "api" && a.Contains("POST")) {
      if (a[1] != "repos/acme/app/issues/212/comments") return 2;
      File.WriteAllText(Path.Combine(home, "posted.json"), Console.In.ReadToEnd());
      Console.Write("{\\"html_url\\":\\"https://github.com/acme/app/issues/212#issuecomment-99\\"}"); return 0;
    }
    if (a.Length > 1 && a[0] == "api") {
      if (a[1] == "user") { Console.Write("person"); return 0; }
      if (a[1].StartsWith("repos/acme/app/issues/comments?")) { Console.Write(File.ReadAllText(Path.Combine(home, "comments.json"))); return 0; }
      if (a[1] == "repos/acme/app/issues/212") { Console.Write(File.ReadAllText(Path.Combine(home, "issue.json"))); return 0; }
      if (a[1].StartsWith("repos/acme/app/issues/212/comments?")) { Console.Write(File.ReadAllText(Path.Combine(home, "recent.json"))); return 0; }
      return 2;
    }
    if (a.Length > 1 && a[0] == "repo" && a[1] == "view") { Console.Write(a.Contains("nameWithOwner") ? "acme/app" : "main"); return 0; }
    if (a.Length > 0 && a[0] == "--version") { Console.Write("gh version 2.0.0"); return 0; }
    if (a.Length > 0 && a[0] == "auth") { Console.Write("Logged in to github.com as person"); return 0; }
    Console.Write("[]"); return 0;
  }
}
`)
await run(join(process.env.WINDIR ?? 'C:\\Windows', 'Microsoft.NET', 'Framework64', 'v4.0.30319', 'csc.exe'), ['/nologo', `/out:${join(bin, 'gh.exe')}`, join(bin, 'Gh.cs')], bin)
// Seed the watch before the fixture comment: the first look must not import historical asks.
await writeFile(join(profilePath, 'scout.json'), JSON.stringify({ settings: { on: true, teammateId: 'tm_scout', everyMinutes: 60, watches: { github: false, teammates: false, comments: true, check: false }, notify: false, waitWhileWorking: false }, cards: [], seen: [], lookIds: [], prs: [], notes: [], comments: { enabledAt: new Date(Date.parse(own.created_at) - 60_000).toISOString(), repositories: {} } }))
const ada = { teammateId: 'tm_ada', name: 'Ada', hue: 'lime', role: 'Custom', createdAt: '2026-10-22T00:00:00Z', route: { ...FREE_ROUTE, mode: 'ask' } }
const scout = { ...ada, teammateId: 'tm_scout', name: 'Scout', hue: 'teal', roleTitle: 'Keeps watch' }
const packaged = process.argv.includes('--packaged') ? process.argv[process.argv.indexOf('--packaged') + 1] : undefined
const drive = await startDrive({ name: 'locust-comment', port: 9891, workspace, profilePath, holdCursor: false,
  ...(packaged ? { packaged } : {}), outPath: join(recordRoot('locust-in-a-github-comment-2026-10-22'), 'local'),
  env: { PATH: `${bin}${delimiter}${process.env.PATH ?? ''}` },
  seed: { schemaVersion: 1, teammates: [ada, scout], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok) => { if (!ok) failures += 1; say(`[${ok ? 'PASS' : 'FAIL'}] ${what}`) }
const state = async () => JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then(JSON.stringify)')))
const home = async () => { await drive.evaluate(`document.querySelector('button[aria-label="Home"]')?.click()`); await sleep(300) }
try {
  await drive.ready()
  await drive.evaluate('window.desktop.scout.look()')
  let current = await state()
  const asks = current.cards.filter((card) => card.commentAsk)
  check('only the signed-in person’s comment becomes a card', asks.length === 1)
  check('the card states the ask and names Ada', asks[0]?.said === 'You asked on #212 Talk starts from any app: Reply with exactly COMMENT_REPLY_OK. Do not change any file.' && asks[0]?.draft?.name === 'Ada')
  await home()
  await drive.capture('your comment is an ask', () => drive.evaluate(`document.querySelector('[data-scout-card]')?.innerText ?? ''`))
  check('nothing was posted before Send', !(await readFile(join(bin, 'posted.json'), 'utf8').catch(() => '')))
  if (asks[0]) await drive.evaluate(`[...document.querySelector('[data-scout-card="${asks[0].cardId}"]').querySelectorAll('button')].find((b) => b.innerText.trim() === 'Send to Ada')?.click()`)
  for (let i = 0; i < 480 && !current.cards.some((card) => card.post); i += 1) { await sleep(500); current = await state() }
  const answer = current.cards.find((card) => card.post)
  check('the finished turn raises Ada’s answer card', answer?.said === 'Ada answered your ask on #212' && answer.post.message.includes('COMMENT_REPLY_OK'))
  check('the answer has not posted itself', !(await readFile(join(bin, 'posted.json'), 'utf8').catch(() => '')))
  await home()
  await drive.capture('Ada’s full reply waits for Post', () => drive.evaluate(`document.body.innerText`))
  if (answer) {
    await drive.evaluate(`[...document.querySelector('[data-scout-card="${answer.cardId}"]').querySelectorAll('button')].find((b) => b.innerText.trim() === 'Post it on #212')?.click()`)
    let sent = ''
    for (let i = 0; i < 40 && !sent; i += 1) { await sleep(250); sent = await readFile(join(bin, 'posted.json'), 'utf8').catch(() => '') }
    check('Post sent exactly the full card text, including attribution', sent && JSON.parse(sent).body === answer.post.message && answer.post.message.endsWith('Written by Ada, a Locust teammate.'))
    await drive.capture('posted with its link', () => drive.evaluate(`document.body.innerText`))
  }
} catch (error) { failures += 1; say(`[FAIL] ${error instanceof Error ? error.stack : error}`) }
finally { await drive.finish({ intro: `Free route ${FREE_ROUTE.model}; fake gh first on PATH; no real GitHub writes. Scratch: ${scratch}` }) }
if (failures) process.exitCode = 1
