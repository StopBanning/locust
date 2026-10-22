import { describe, expect, it } from 'vitest'

import type { FolderPullRequest } from './pull-request.js'
import { pullRequestsOf } from './pull-request.js'
import {
  DEFAULT_SCOUT_SETTINGS,
  MAX_SCOUT_CARDS,
  branchFacts,
  branchRunsOf,
  cardOf,
  failedFacts,
  issueFacts,
  issuesOf,
  pullRequestFacts,
  reviewFacts,
  reviewsAskedOf,
  type IssueOpened,
  scoutAnswers,
  scoutPrompt,
  scoutSettingsFrom,
  waitingFacts,
  withNewCards
} from './scout.js'

/*
 * SCOUT NOTICES WHAT IS NEW (0.756). Colin, 2026-10-12: "maybe allow the user to change the model of proactive
 * teammate if they wish but in the meantime let's just use the best free model". Locust looks first, with no model:
 * each card leads with Locust's own sentence, made from what GitHub and the record say, and a fact is told once.
 * Scout's model only adds its take and a drafted next step -- read loosely, kept strictly: nothing it says may name
 * an item that was not asked, or a teammate who is not on the team.
 */
const URL = 'https://github.com/acme/app/pull/212'
const pr = (change: Partial<FolderPullRequest> = {}): FolderPullRequest => ({
  number: 212, title: 'Talk starts from any app', url: URL, state: 'open', checks: 'passing', checkCount: 9, notPassed: 0, ...change
})

describe('pull requests', () => {
  it('seen for the first time, are news only for what needs work now, keyed by their state', () => {
    expect(pullRequestFacts(new Map(), [pr()], 'look1')).toEqual([])
    const [failing] = pullRequestFacts(new Map(), [pr({ checks: 'failing', notPassed: 2 })], 'look1')
    expect(failing).toMatchObject({ urgency: 'now', said: 'Your pull request #212, Talk starts from any app: 2 of 9 checks did not pass.', open: { kind: 'url', url: URL } })
    expect(failing?.key).toBe(`pr:${URL}:failing::`)
    expect(pullRequestFacts(new Map(), [pr({ review: 'changes-requested', conflicts: true })], 'look1')[0]).toMatchObject({ urgency: 'soon', said: expect.stringContaining('changes were asked for, and it conflicts with its base branch') })
  })

  it('tell what changed since the last look, keyed by the look, so the same state coming back is news again', () => {
    const before = new Map([[URL, pr()]])
    const [news] = pullRequestFacts(before, [pr({ checks: 'failing', notPassed: 1 })], 'look7')
    expect(news).toMatchObject({ key: `pr:${URL}:look7`, urgency: 'now', said: 'Your pull request #212, Talk starts from any app: 1 of 9 checks did not pass.' })
    const [passed] = pullRequestFacts(new Map([[URL, pr({ checks: 'failing', notPassed: 1 })]]), [pr()], 'look8')
    expect(passed).toMatchObject({ key: `pr:${URL}:look8`, urgency: 'later', said: expect.stringContaining('all 9 checks passed') })
    expect(pullRequestFacts(before, [pr()], 'look9')).toEqual([])
  })

  it('ready and waiting -- passing, mergeable, nothing asked, untouched three days -- are said once, for later', () => {
    // The person's hearth #2, read against their own repositories on 2026-10-12: passing, mergeable, no reviewer, since 2026-08-28.
    const at = new Date('2026-10-12T15:00:00.000Z')
    const waited = pr({ updatedAt: '2026-08-28T05:03:26Z' })
    const [ready] = pullRequestFacts(new Map(), [waited], 'look1', at)
    expect(ready).toMatchObject({ key: `pr:${URL}:ready`, urgency: 'later', said: 'Your pull request #212, Talk starts from any app, passes its checks and has waited 45 days. It can be merged, or closed.', open: { kind: 'url', url: URL } })
    // One Scout has known since it was a day old says so too, once it has waited three.
    expect(pullRequestFacts(new Map([[URL, waited]]), [waited], 'look2', at).map((fact) => fact.key)).toEqual([`pr:${URL}:ready`])
    // Two days is not waiting; a draft, a conflict or changes asked are not ready.
    expect(pullRequestFacts(new Map(), [pr({ updatedAt: '2026-10-10T16:00:00Z' })], 'look1', at)).toEqual([])
    expect(pullRequestFacts(new Map(), [pr({ updatedAt: waited.updatedAt!, state: 'draft' })], 'look1', at)).toEqual([])
    expect(pullRequestFacts(new Map([[URL, pr({ updatedAt: waited.updatedAt!, conflicts: true })]]), [pr({ updatedAt: waited.updatedAt!, conflicts: true })], 'look2', at)).toEqual([])
    // Never beside other news of the same pull request: its checks passing again is the card.
    const passedAgain = pullRequestFacts(new Map([[URL, pr({ checks: 'failing', notPassed: 1 })]]), [waited], 'look3', at)
    expect(passedAgain.map((fact) => fact.key)).toEqual([`pr:${URL}:look3`])
  })

  it('are read from gh pr list, and anything that is not one is left out', () => {
    const json = JSON.stringify([
      { number: 1, title: 'One', url: 'https://github.com/acme/app/pull/1', state: 'OPEN', isDraft: false, statusCheckRollup: [{ status: 'COMPLETED', conclusion: 'FAILURE' }], reviewDecision: '', mergeable: 'MERGEABLE' },
      { number: 'two', url: 'https://github.com/acme/app/pull/2' },
      'nothing'
    ])
    expect(pullRequestsOf(json).map((one) => [one.number, one.checks])).toEqual([[1, 'failing']])
    expect(pullRequestsOf('not json')).toEqual([])
    expect(pullRequestsOf('{"number":1}')).toEqual([])
  })
})

describe('issues on the folder’s repository', () => {
  const at = new Date('2026-10-12T15:00:00.000Z')
  const issue = (number: number, createdAt: string): IssueOpened => ({ number, title: `Issue ${String(number)}`, url: `https://github.com/acme/app/issues/${String(number)}`, createdAt, author: 'dana' })

  it('opened this week are news, newest first, three a look, each said once by its address', () => {
    const facts = issueFacts([issue(1, '2026-10-01T00:00:00Z'), issue(2, '2026-10-11T00:00:00Z'), issue(3, '2026-10-12T10:00:00Z'), issue(4, '2026-10-10T00:00:00Z'), issue(5, '2026-10-09T00:00:00Z')], at)
    expect(facts.map((fact) => fact.key)).toEqual([3, 2, 4].map((number) => `issue:https://github.com/acme/app/issues/${String(number)}`))
    expect(facts[0]).toMatchObject({ urgency: 'soon', said: 'dana opened issue #3: Issue 3.', open: { kind: 'url', url: 'https://github.com/acme/app/issues/3', label: 'Open the issue' } })
  })

  it('are read from gh issue list, and anything that is not one is left out', () => {
    const json = JSON.stringify([
      { number: 7, title: 'It  crashes\\non start', url: 'https://github.com/acme/app/issues/7', createdAt: '2026-10-12T00:00:00Z', author: { login: 'dana' } },
      { number: 8, title: 'A pull request', url: 'https://github.com/acme/app/pull/8', createdAt: '2026-10-12T00:00:00Z' },
      { number: 9, title: 'No date', url: 'https://github.com/acme/app/issues/9' },
      'nothing'
    ])
    expect(issuesOf(json)).toEqual([{ number: 7, title: 'It crashes\\non start', url: 'https://github.com/acme/app/issues/7', createdAt: '2026-10-12T00:00:00Z', author: 'dana' }])
    expect(issuesOf('not json')).toEqual([])
  })
})

describe('reviews asked, and the default branch', () => {
  it('a review asked of the person names who asked', () => {
    const asked = reviewsAskedOf(JSON.stringify([{ number: 214, title: 'Rename the call log', url: 'https://github.com/acme/app/pull/214', author: { login: 'ada' } }, { number: 5, url: 'https://evil.example/pull/5' }]))
    expect(asked).toEqual([{ number: 214, title: 'Rename the call log', url: 'https://github.com/acme/app/pull/214', author: 'ada' }])
    expect(reviewFacts(asked)[0]).toMatchObject({ key: 'review:https://github.com/acme/app/pull/214', urgency: 'soon', said: 'ada asked you to review pull request #214, Rename the call log.' })
  })

  it('the newest finished run of each workflow says how the branch stands; only a failed one is news', () => {
    const runs = branchRunsOf(JSON.stringify([
      { databaseId: 9, status: 'in_progress', conclusion: '', workflowName: 'ci', url: 'https://github.com/acme/app/actions/runs/9' },
      { databaseId: 8, status: 'completed', conclusion: 'failure', workflowName: 'ci', url: 'https://github.com/acme/app/actions/runs/8' },
      { databaseId: 7, status: 'completed', conclusion: 'success', workflowName: 'ci', url: 'https://github.com/acme/app/actions/runs/7' },
      { databaseId: 6, status: 'completed', conclusion: 'success', workflowName: 'docs', url: 'https://github.com/acme/app/actions/runs/6' }
    ]))
    expect(runs.map((run) => [run.workflow, run.id, run.failed])).toEqual([['ci', 8, true], ['docs', 6, false]])
    expect(branchFacts('main', runs)).toEqual([expect.objectContaining({ key: 'branch:8', said: 'ci failed on main.' })])
  })
})

describe('teammates', () => {
  it('waiting on the person is news after ten minutes, and needs them now', () => {
    const card = { approvalId: 'ap_1', missionId: 'mis_1', teammateId: 'tm_atlas', name: 'Atlas', summary: 'Run pnpm test', requestedAt: '2026-10-12T05:00:00.000Z' }
    expect(waitingFacts([card], new Date('2026-10-12T05:09:00.000Z'))).toEqual([])
    const [waiting] = waitingFacts([card], new Date('2026-10-12T05:25:00.000Z'))
    expect(waiting).toMatchObject({ key: 'waiting:ap_1', urgency: 'now', said: 'Atlas has waited 25 minutes for your OK.', open: { kind: 'conversation', missionId: 'mis_1', label: 'Open it' } })
    // What it waits to do can be a command with a secret in it: the person sees it, Scout's model never does.
    expect(waiting?.quote).toBe('Run pnpm test')
    expect(scoutPrompt({ facts: [waiting!], team: [] })).not.toContain('pnpm test')
  })

  it('a turn that failed says whose, and why in the runtime’s own words, with what was asked for the model', () => {
    expect(failedFacts([{ missionId: 'mis_9', teammateId: 'tm_wren', name: 'Wren', asked: 'Fix the save', why: 'Model not found: opencode/no-such-model-free.' }])[0])
      .toMatchObject({ key: 'failed:mis_9', said: 'Wren\'s turn failed: Model not found: opencode/no-such-model-free.', detail: 'It was asked: Fix the save', teammateId: 'tm_wren' })
    expect(failedFacts([{ missionId: 'mis_8', asked: 'Fix the save' }])[0]?.said).toBe('A teammate\'s turn failed.')
  })
})

describe('Scout’s model', () => {
  const team = [{ teammateId: 'tm_wren', name: 'Wren' }, { teammateId: 'tm_atlas', name: 'Atlas' }]

  it('is asked about the new facts and the team, read-only, in one form', () => {
    const prompt = scoutPrompt({ facts: failedFacts([{ missionId: 'mis_9', name: 'Wren', asked: 'Fix the save', why: 'Overloaded.' }]), team: [{ name: 'Atlas', role: 'Reviews' }] })
    expect(prompt).toContain('Use only what is below: do not use any tools, open files, run commands or change anything.')
    expect(prompt).toContain("1. Wren's turn failed: Overloaded.\n   It was asked: Fix the save")
    expect(prompt).toContain('- Atlas, Reviews')
    expect(prompt).toContain('ITEM <its number>')
    expect(prompt).toContain('Say nothing that is not in what is above.')
  })

  it('is read loosely: bold labels, a colon after the number, a draft over several lines', () => {
    const answer = scoutAnswers([
      'Here is what I think.', '', '**ITEM 1:**', '**TAKE:** The timeout is new since this morning.', 'HAND TO: Wren.', 'DRAFT: Find why the save waits on Mac.', 'Run the test three times.', '',
      'ITEM 2', 'TAKE: Approved and clean.', 'HAND TO: none'
    ].join('\n'), 2, team)
    expect(answer.get(1)).toEqual({ take: 'The timeout is new since this morning.', draft: { teammateId: 'tm_wren', name: 'Wren', message: 'Find why the save waits on Mac.\nRun the test three times.' } })
    expect(answer.get(2)).toEqual({ take: 'Approved and clean.' })
  })

  it('is kept strictly: no item that was not asked, no teammate who is not on the team, no draft with nobody to send it to', () => {
    const answer = scoutAnswers([
      'ITEM 3', 'TAKE: Out of range.',
      'ITEM 1', 'HAND TO: Mallory', 'DRAFT: Delete everything.',
      'ITEM 2', 'TAKE: Kept.', 'HAND TO: Mallory', 'DRAFT: Send nothing.'
    ].join('\n'), 2, team)
    expect(answer.has(3)).toBe(false)
    expect(answer.has(1)).toBe(false)
    expect(answer.get(2)).toEqual({ take: 'Kept.' })
    expect(scoutAnswers('nothing in the form', 2, team).size).toBe(0)
  })
})

describe('cards and settings', () => {
  it('keep the newest, at most MAX_SCOUT_CARDS', () => {
    const fact = failedFacts([{ missionId: 'mis_1', asked: 'x' }])[0]!
    const many = Array.from({ length: MAX_SCOUT_CARDS }, (_, index) => cardOf(fact, `card_${String(index)}`, new Date(0), false))
    const added = cardOf(fact, 'card_new', new Date(1), true)
    const kept = withNewCards(many, [added])
    expect(kept).toHaveLength(MAX_SCOUT_CARDS)
    expect(kept[0]).toMatchObject({ cardId: 'card_new', reading: true })
  })

  it('take only what a person could have set', () => {
    expect(scoutSettingsFrom(undefined)).toEqual(DEFAULT_SCOUT_SETTINGS)
    // Your /locust comments (0.823) is on unless the person turns it off.
    expect(scoutSettingsFrom({ on: true, everyMinutes: 30, watches: { github: false }, notify: false, teammateId: 'tm_scout' })).toEqual({ on: true, teammateId: 'tm_scout', everyMinutes: 30, watches: { github: false, teammates: true, check: false, comments: true }, notify: false, waitWhileWorking: true })
    expect(scoutSettingsFrom({ everyMinutes: 1, teammateId: '../x', on: 'yes' })).toEqual(DEFAULT_SCOUT_SETTINGS)
  })
})
