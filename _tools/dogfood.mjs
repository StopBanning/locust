// Locust works on Locust, in a worktree it cannot damage.
//
//   node _tools/dogfood.mjs                  one long mission, free model
//   node _tools/dogfood.mjs --task plan      pick a task (see TASKS below)
//   node _tools/dogfood.mjs --keep           leave the worktree for inspection
//
// Every other drive hands the app a scratch repo with two files in it. That
// exercises the plumbing and nothing else: a two-file project has no
// architecture to get lost in, no naming convention to notice, no reason for a
// plan to have more than one step. The long-running behaviour that a person
// actually meets -- a plan that survives ten tool calls, a thread that stays
// readable at turn twenty, a receipt that is still honest after a mission that
// touched nine files -- has never been driven, because nothing has ever given
// the app work big enough to need it.
//
// So: this repo, which is the biggest real codebase on hand and the one whose
// bugs matter most here.
//
// THE WORKTREE IS NOT A CONVENIENCE, IT IS THE WHOLE SAFETY STORY.
//
// A teammate editing the checkout that the running app was built from is a bad
// afternoon: the edit lands, a watcher rebuilds, the app under test changes
// beneath the mission, and the record no longer describes any build that ever
// existed. Worse, several sessions write to the live tree. So the workspace is
// a detached-HEAD `git worktree` under the scratch root, removed at the end
// unless --keep. Nothing here ever names the live checkout as a workspace, and
// the guard below refuses to start if it somehow does.
//
// The mission's own instructions tell the teammate it is being observed and
// that its notes on the app are the point. That is deliberate: the finding is
// usually not "the model got the task wrong" but "the app made the task hard
// to do", and the only witness to the second one is the thing doing the work.
//
// Cost: OpenCode's free model by default, so a long run is free. Pass
// --route to spend real money on purpose.

import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import { SCRATCH_ROOT } from '../_tools/scratch-root.mjs'
import { localPath, git, pickRouteScript, say, sendAndWaitScript, startDrive } from './drive-lib.mjs'

const REPO = resolve(localPath(new URL('..', import.meta.url)))
const SCRATCH = SCRATCH_ROOT

/**
 * The tasks, chosen so that each one is genuinely long and none of them can
 * break anything if the model does it badly.
 *
 * Two rules they all obey. They are READ-HEAVY and write to ONE new file, so a
 * bad run produces a bad document rather than a bad edit spread over the
 * codebase -- and the worktree throws the document away anyway. And none of
 * them asks for a build or a test run: a fresh worktree has no node_modules,
 * `pnpm test` would fail for a reason that has nothing to do with the app, and
 * a failure that is not about what is being measured is worse than no run.
 */
const TASKS = {
  // Many files, one document, no ambiguity about when it is done. The point is
  // turn count and whether the plan survives it.
  survey: {
    turns: 'long',
    text: [
      'Read every file under apps/desktop/src/main that ends in .ts and is not a test.',
      'Write docs/DOGFOOD-main-survey.md: one section per file, each giving the file, one sentence on what it is responsible for, and the exported names.',
      'Work through them in a stable order and do not stop partway. When the document covers every file, say so and stop.'
    ].join(' ')
  },
  // A question with a real answer in the code, reached only by following
  // several hops. Exercises a plan that has to change as it learns.
  trace: {
    turns: 'long',
    text: [
      'Trace what happens between a person pressing the send button and a runtime process being spawned.',
      'Start at the renderer form in apps/desktop/src/renderer, follow the IPC channel into apps/desktop/src/main, and continue into packages/runtime-adapters until you reach the actual spawn call.',
      'Write docs/DOGFOOD-send-trace.md naming every file and function on the path in order, with the line where each hop happens.',
      'If a hop is not what you expected, say so in the document rather than smoothing it over.'
    ].join(' ')
  },
  // The one with the highest chance of finding a real defect, because it makes
  // the model compare what the code does against what the code says it does.
  claims: {
    turns: 'long',
    text: [
      'Read the block comments at the top of every file under apps/desktop/src/shared.',
      'For each one, check whether the code below it still does what the comment claims.',
      'Write docs/DOGFOOD-comment-audit.md listing every comment that no longer matches its code, quoting the claim and naming the line that contradicts it.',
      'Do not change any code. A comment you cannot check is a row that says so.'
    ].join(' ')
  }
}

/** What the teammate is told before the task, and why it is told it. */
const BRIEFING = [
  '# You are testing Locust, from inside Locust',
  '',
  'This workspace is a throwaway git worktree of the Locust source. You are',
  'running inside the Locust desktop app, working on the code of the app itself. Nothing',
  'you write here reaches the real checkout, and the whole folder is deleted',
  'when the session ends. Work normally.',
  '',
  'Two things are being measured, and only one of them is your task.',
  '',
  '1. The task, below. Do it properly and finish it.',
  '2. The app. You are the only witness to what using it was like.',
  '',
  'So: if anything about the app itself gets in your way -- a plan that lost a',
  'step, an approval that asked something you could not answer from what was on',
  'screen, a receipt that did not match what you actually did, a file you could',
  'not read, an error that told you nothing -- write it down in your final',
  'message under a heading "What the app did". Be specific and quote what you',
  'saw. If nothing got in your way, say that instead; a clean report is a',
  'result, and inventing a complaint to seem useful is worse than none.',
  '',
  'Do not try to fix anything you find wrong with Locust. Report it.',
  '',
  'There is no node_modules here. Do not run builds or tests; read the source.'
].join('\n')

const argv = process.argv.slice(2)
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`)
  return at === -1 ? fallback : (argv[at + 1] ?? fallback)
}
const keep = argv.includes('--keep')
const taskName = flag('task', 'survey')
const task = TASKS[taskName]
if (task === undefined) {
  say(`no such task: ${taskName}. try: ${Object.keys(TASKS).join(', ')}`)
  process.exit(1)
}

const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
const workspace = join(SCRATCH, `dogfood-${taskName}-${stamp}`)

// The guard the whole design rests on. If the workspace ever resolved to the
// live checkout -- a bad SCRATCH, a symlink, a future edit here -- a teammate
// on accept-edits would start writing into the tree this app was built from.
// Refuse rather than find out.
if (resolve(workspace).toLowerCase().startsWith(REPO.toLowerCase())) {
  say(`refusing: the workspace ${workspace} is inside the live checkout ${REPO}`)
  process.exit(1)
}

await mkdir(SCRATCH, { recursive: true })
say(`worktree: ${workspace}`)
// Detached, so this never creates or moves a branch in the real repo.
await git(['worktree', 'add', '--detach', workspace, 'HEAD'], REPO)
await writeFile(join(workspace, 'LOCUST.md'), BRIEFING, 'utf8')

const drive = await startDrive({
  name: `dogfood-${taskName}`,
  port: 9362,
  workspace,
  keep,
  seed: {
    schemaVersion: 1,
    teammates: [
      {
        teammateId: 'tm_wren',
        name: 'Wren',
        hue: 'lime',
        role: 'Code & Migrations',
        createdAt: '2026-09-07T18:00:00.000Z'
      }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const route = flag('route', undefined)
const DOCUMENT = { survey: 'DOGFOOD-main-survey.md', trace: 'DOGFOOD-send-trace.md', claims: 'DOGFOOD-comment-audit.md' }

try {
  await drive.capture('opened on a worktree of its own source', () => drive.ready())

  await drive.capture('route', () =>
    drive.evaluate(
      route === undefined
        ? pickRouteScript({ group: '/opencode/i', search: 'free', row: '/free/i' })
        : pickRouteScript({ group: '/./', search: route, row: `/${route}/i` })
    )
  )

  // Twenty minutes. A survey of thirty files is not a two-minute mission, and
  // a bound that cut it off early would measure the bound, not the app.
  await drive.capture('the long mission', () =>
    drive.evaluate(sendAndWaitScript(task.text, { waitSeconds: 1200 }))
  )

  // Read separately from the step above, because `sendAndWaitScript` reports
  // that the stop button went away -- which it does for a run that failed just
  // as surely as for one that worked. The first run of this harness recorded
  // "finished" over a mission whose own header said `failed`, and a record that
  // does that is worse than no record. So the outcome comes off the header.
  await drive.capture('what the app says the outcome was', () =>
    drive.evaluate(`(() => {
      const header = document.querySelector('.lc-workroom__mission')
      if (!header) return 'no mission header on screen'
      const line = header.innerText.replace(/\\s+/g, ' ').trim()
      const failed = /\\bfailed\\b|could not continue/i.test(line + ' ' + (document.querySelector('.lc-thread')?.innerText ?? ''))
      return (failed ? 'FAILED -- ' : '') + line
    })()`)
  )

  // What the teammate said about the app, which is half the reason for the run.
  // Kept separately from the mission outcome because a mission can succeed
  // while the app behaved badly, and that is the interesting case.
  await drive.capture('what it said about the app', () =>
    drive.evaluate(`(() => {
      const thread = document.querySelector('.lc-thread')
      if (!thread) return 'no thread'
      const text = thread.innerText
      const at = text.lastIndexOf('What the app did')
      return at === -1 ? 'no "What the app did" section; tail: ' + text.slice(-1200) : text.slice(at, at + 3000)
    })()`)
  )

  // Did the work actually land on disk. A thread that reads like success and a
  // folder with nothing in it is a failure mode worth catching by hand.
  await drive.capture('the document it was asked for', async () => {
    const wrote = await readFile(join(workspace, 'docs', DOCUMENT[taskName]), 'utf8').catch(() => undefined)
    return wrote === undefined
      ? `NOT WRITTEN -- the mission ended without ${DOCUMENT[taskName]}`
      : `${String(wrote.length)} bytes; first lines: ${wrote.slice(0, 400).replace(/\s+/g, ' ')}`
  })
} finally {
  await drive.finish({
    intro: `Locust working on a detached worktree of its own source, task \`${taskName}\`, on ${route === undefined ? "OpenCode's free model" : String(route)}. The workspace was \`${workspace}\` and is ${keep ? 'kept' : 'removed'}.`,
    extra: [
      '## What this run is for',
      '',
      'Two things, and the second is the one a unit test cannot reach:',
      '',
      '1. Did a long mission on a real codebase finish, and did the file it was',
      '   asked for actually appear on disk.',
      '2. What the teammate said under "What the app did" — the only first-hand',
      '   account of what using Locust was like for the length of a real task.',
      ''
    ].join('\n')
  })
  if (!keep) {
    await git(['worktree', 'remove', '--force', workspace], REPO).catch((error) => {
      say(`could not remove the worktree: ${String(error)}`)
    })
  } else {
    say(`kept: ${workspace} (remove with: git worktree remove --force "${workspace}")`)
  }
}
