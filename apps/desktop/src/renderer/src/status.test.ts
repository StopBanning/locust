import { describe, expect, it } from 'vitest'

import type { PublicRuntimeStatus } from '../../shared/ipc.js'
import { swarmEffortFor } from './App.js'
import {
  effortIsInModelId,
  modelLabelFor,
  defaultRoute,
  defaultEffort,
  effortAfterRouteChange,
  checkpointLabel,
  collapseConversations,
  listedAsMission,
  modeLabel,
  recentRouteRows,
  routeSearchText,
  facePresenceFor,
  connectedRuntimeCount,
  handoffAvailability,
  handoffTitle,
  ledgerVerificationLabel,
  missionPhaseView,
  capRouteRows,
  modeRunsOn,
  modesFor,
  ownerToSelect,
  sandboxPhrase,
  accountPhrase,
  modeUnavailableReason,
  flagshipRank,
  missionsMatching,
  orderRouteRows,
  integrationOf,
  RUNTIME_INTEGRATION,
  formatBytes,
  prunePreviewSummary,
  ROUTE_GROUP_LIMIT,
  routeRowStatus,
  routeRowTag,
  runtimeIsUsable,
  shortMissionId,
  teammateStatusView, ROUTE_SEARCH_GROUP_LIMIT } from './status.js'
import type { RouteTag } from './status.js'
import { startRoute } from './status.js'
import type { PublicModel } from '../../shared/ipc.js'

function runtime(overrides: Partial<PublicRuntimeStatus> = {}): PublicRuntimeStatus {
  return {
    id: 'codex',
    displayName: 'Codex CLI',
    installed: true,
    version: '0.151.0-alpha.7.2',
    auth: 'authenticated',
    ready: true,
    status: 'ready',
    ...overrides
  }
}

describe('the routes a person moves between', () => {
  const rows = [
    { key: 'codex:gpt-5.6-sol', group: 'Codex CLI · your account' },
    { key: 'claude:sonnet', group: 'Claude Code · your account' },
    { key: 'cursor:composer-2.5', group: 'Cursor Agent · your account' }
  ]

  // A list long enough for a shortcut to shorten something.
  const longList = [
    ...rows,
    ...Array.from({ length: 9 }, (_, index) => ({
      key: `codex:filler-${String(index)}`,
      group: 'Codex CLI · your account'
    }))
  ]

  it('lifts the recently used routes into a group of their own, newest first', () => {
    const recent = recentRouteRows(longList, ['claude:sonnet', 'codex:gpt-5.6-sol'])
    expect(recent.map((row) => row.group)).toEqual(['Recent', 'Recent'])
    expect(recent.map((row) => row.key)).toEqual(['recent:claude:sonnet', 'recent:codex:gpt-5.6-sol'])
  })

  it('withholds the group for a single recent route, which is the one you are on', () => {
    // Against the LONG list, so the short-list and half-the-list rules cannot
    // withhold it for their own reasons -- this has to fail when the
    // single-recent rule is the thing that breaks. The sweep caught exactly
    // that: written against the short list, the test passed no matter what
    // this rule did.
    expect(recentRouteRows(longList, ['claude:sonnet'])).toEqual([])
    expect(recentRouteRows(longList, [])).toEqual([])
  })

  it('withholds the group when the whole list is short enough to take in at once', () => {
    // The justification for the group was 27 rows scrolling at 330px. Three
    // rows on screen have nothing to skip past, so a shortcut shortens nothing.
    expect(recentRouteRows(rows, ['claude:sonnet', 'codex:gpt-5.6-sol'])).toEqual([])
  })

  it('withholds the group when it would be half the list or more', () => {
    // With two routes total the shortcut is a second copy of the picker.
    const many = Array.from({ length: 12 }, (_, index) => ({
      key: `codex:model-${String(index)}`,
      group: 'Codex CLI · your account'
    }))
    const halfOrMore = many.slice(0, 6).map((row) => row.key)
    expect(recentRouteRows(many, halfOrMore, 6)).toEqual([])
    const aFew = many.slice(0, 3).map((row) => row.key)
    expect(recentRouteRows(many, aFew)).toHaveLength(3)
  })

  it('never resurrects a route the runtime has stopped offering', () => {
    const recent = recentRouteRows(longList, ['codex:retired-model', 'claude:sonnet', 'cursor:composer-2.5'])
    expect(recent.map((row) => row.key)).toEqual(['recent:claude:sonnet', 'recent:cursor:composer-2.5'])
  })

  it('stops at the limit rather than repeating the whole list', () => {
    const many = ['codex:gpt-5.6-sol', 'claude:sonnet', 'cursor:composer-2.5']
    expect(recentRouteRows(longList, many, 2)).toHaveLength(2)
  })
})

describe('the mode a roster card shows', () => {
  it('names the mode the teammate actually last ran in', () => {
    // The card printed the literal string `read-only` for every teammate,
    // whatever they had run in -- false the moment a runtime could edit.
    expect(modeLabel('accept-edits')).toBe('edit')
    expect(modeLabel('approve-each')).toBe('approve each action')
    expect(modeLabel('ask')).toContain('read-only')
  })

  it('says a teammate who has never run has no mode yet, rather than inventing one', () => {
    expect(modeLabel(undefined)).toBe('not set yet')
  })
})

describe('what belongs in the mission list', () => {
  it('keeps a run that has a mission id, settled or not', () => {
    expect(listedAsMission({ missionId: 'mission_1', active: false })).toBe(true)
    expect(listedAsMission({ missionId: 'mission_1', active: true })).toBe(true)
  })

  it('keeps a run that is still starting, because that is how a teammate reads as working', () => {
    expect(listedAsMission({ missionId: undefined, active: true })).toBe(true)
  })

  it('drops a refused send, which settled without ever being given a mission id', () => {
    // Otherwise it sits in the list forever under an internal key, and
    // "Copy mission id" hands back that key.
    expect(listedAsMission({ missionId: undefined, active: false })).toBe(false)
  })
})

describe('nothing is live unless discovery proved it', () => {
  // This is the invariant the product's trust claim rests on. If it can be
  // broken, every other label in the shell is decoration.
  it('never issues ACTIVE or READY for a runtime that is not ready', () => {
    const notReady: readonly Partial<PublicRuntimeStatus>[] = [
      { ready: false, status: 'probe-failed' },
      { ready: false, status: 'offline' },
      { ready: false, status: 'auth-required' },
      { ready: false, status: 'not-installed', installed: false },
      // The dangerous one: a probe that failed while a stale `ready` flag
      // survived. Either field alone must be enough to withhold the label.
      { ready: true, status: 'probe-failed' },
      { ready: false, status: 'ready' }
    ]
    for (const overrides of notReady) {
      const status = routeRowStatus(runtime(overrides), 'live', true)
      expect(status.tag).not.toBe('ACTIVE')
      expect(status.tag).not.toBe('READY')
      expect(status.selectable).toBe(false)
    }
  })

  it('marks a ready runtime active only when it is the active one', () => {
    expect(routeRowStatus(runtime(), 'live', true).tag).toBe('ACTIVE')
    expect(routeRowStatus(runtime(), 'live', false).tag).toBe('READY')
  })

  it('a runtime whose last run ended on the usage limit says so instead of READY', () => {
    // MEASURED user session 1, 2026-09-05: two Codex missions in a row failed
    // on quota, and Settings, the welcome list and every picker row of it
    // still read READY. Signed in was true; runnable was not.
    const said = "You've hit your usage limit. Try again at Sep 7th, 2026 1:57 AM."
    const limited = routeRowStatus(runtime(), 'live', false, said)
    expect(limited.tag).toBe('AT LIMIT')
    expect(limited.selectable).toBe(true)
    expect(limited.detail).toContain(said)
    // Outranks ACTIVE: the active route refusing to run is the thing to show.
    expect(routeRowTag(routeRowStatus(runtime(), 'live', true, said), true)).toBe('AT LIMIT')
    // A runtime that is not runnable anyway keeps its own reason; the limit
    // note must not upgrade a SIGN IN row to something selectable.
    const signedOut = routeRowStatus(runtime({ ready: false, status: 'ready' }), 'live', false, said)
    expect(signedOut.tag).not.toBe('AT LIMIT')
    expect(signedOut.selectable).toBe(false)
  })

  it('never calls a half-built adapter live, even when its runtime is ready', () => {
    const status = routeRowStatus(runtime({ id: 'claude', displayName: 'Claude Code' }), 'preview', true)
    expect(status.tag).toBe('PREVIEW')
    // Selectable is the point -- Claude must be visibly choosable -- but the
    // row has to say what has not been established about it.
    expect(status.selectable).toBe(true)
    expect(status.detail).toMatch(/not been tried through a whole run/)
    /*
     * It used to end "so runs are not durable yet", which was written for an
     * adapter that could not record. Muse Code became the first real PREVIEW
     * row on 2026-09-21 and its runs ARE durable -- same ledger, same
     * receipts; what is unproven is a run under a paying provider. A row
     * that overstates the gap is as wrong as one that hides it.
     */
    expect(status.detail).not.toMatch(/not durable/)
  })

  it('keeps a planned runtime non-interactive whatever discovery says', () => {
    const status = routeRowStatus(runtime({ ready: true, status: 'ready' }), 'planned', true)
    expect(status.tag).toBe('PLANNED')
    expect(status.selectable).toBe(false)
  })

  it('separates sign-in from missing and from broken', () => {
    expect(routeRowStatus(runtime({ installed: false, ready: false, status: 'not-installed' }), 'live', false).tag)
      .toBe('NOT INSTALLED')
    expect(routeRowStatus(runtime({ ready: false, status: 'auth-required', auth: 'unauthenticated' }), 'live', false).tag)
      .toBe('SIGN IN')
    expect(routeRowStatus(runtime({ ready: false, status: 'offline' }), 'live', false).detail)
      .toMatch(/could not be reached/)
    // Installed but not answering is CHECKING, not UNAVAILABLE: the shell
    // asks again, and only a runtime that is not on the machine is missing.
    expect(routeRowStatus(runtime({ ready: false, status: 'probe-failed' }), 'live', false).tag).toBe('CHECKING')
    expect(routeRowStatus(runtime({ ready: false, status: 'offline' }), 'live', false).tag).toBe('CHECKING')
    expect(routeRowStatus(runtime({ ready: false, status: 'probe-failed' }), 'live', false).selectable).toBe(false)
  })

  it('counts only usable runtimes as connected', () => {
    expect(
      connectedRuntimeCount([
        runtime(),
        runtime({ id: 'claude', ready: false, status: 'auth-required' }),
        runtime({ id: 'omniroute', ready: false, status: 'not-installed', installed: false })
      ])
    ).toBe(1)
  })

  it('does not count a runtime this build cannot actually run', () => {
    // A guard rather than a repair: no live miscount has been observed, and
    // the one that looked like it on 2026-09-05 was the probe counting the
    // word READY while the sixth runtime was EXPERIMENTAL and genuinely
    // runnable. What this pins is that a runtime discovery calls ready but
    // this build cannot run -- Gemini probes and reports signed in -- is not
    // counted as connected.
    expect(connectedRuntimeCount([runtime(), runtime({ id: 'gemini' })])).toBe(1)
  })

  it('counts a runtime whose adapter is finished but unofficial', () => {
    // Experimental is a caveat about the SURFACE, not about whether it runs --
    // Antigravity completes missions today. Excluding it would understate.
    expect(connectedRuntimeCount([runtime({ id: 'antigravity' })])).toBe(1)
  })

  it('Antigravity through its CLI carries no Experimental tag; through its app it still does (0.541)', () => {
    // Colin, 2026-10-02: "its still listed as experimental in model picker".
    expect(routeRowStatus(runtime({ id: 'antigravity', displayName: 'Antigravity', throughCli: true }), 'experimental', false).tag).toBe('READY')
    expect(routeRowStatus(runtime({ id: 'antigravity', displayName: 'Antigravity' }), 'experimental', false).tag).toBe('EXPERIMENTAL')
  })
})

describe('teammate status', () => {
  it('reports a blocked runtime even while a mission looks like it is running', () => {
    const view = teammateStatusView({
      runtime: runtime({ ready: false, status: 'auth-required' }),
      hasRunningMission: true,
      pendingApprovals: 0,
      roleLabel: 'Code & Migrations'
    })
    expect(view.status).toBe('blocked')
    expect(view.tone).toBe('red')
  })

  it('puts a pending approval ahead of working', () => {
    const view = teammateStatusView({
      runtime: runtime(),
      hasRunningMission: true,
      pendingApprovals: 1,
      roleLabel: 'Code & Migrations'
    })
    expect(view.status).toBe('approval-needed')
  })

  it('pulses only while working', () => {
    const working = teammateStatusView({
      runtime: runtime(),
      hasRunningMission: true,
      pendingApprovals: 0,
      roleLabel: 'Code & Migrations'
    })
    const idle = teammateStatusView({
      runtime: runtime(),
      hasRunningMission: false,
      pendingApprovals: 0,
      roleLabel: 'Code & Migrations'
    })
    expect(working.pulse).toBe(true)
    expect(idle.pulse).toBe(false)
    expect(idle.label).toMatch(/idle$/)
  })
})

describe('mission phase and receipt truth', () => {
  it('distinguishes every terminal phase', () => {
    expect(missionPhaseView('running').tag).toBe('RUNNING')
    expect(missionPhaseView('completed').tag).toBe('COMPLETED')
    expect(missionPhaseView('interrupted').tag).toBe('INTERRUPTED')
    expect(missionPhaseView('failed').tag).toBe('FAILED')
    expect(missionPhaseView('cancelled').tag).toBe('CANCELLED')
  })

  it('will not present a completed mission as clean when its ledger is not', () => {
    // The run finished; its record did not. The receipt card must not print
    // `verified` over that, and the phase line must not read plain "Completed".
    const view = missionPhaseView('completed', true)
    expect(view.tone).toBe('amber')
    expect(view.label).toMatch(/incomplete/)
    // It names WHAT was verified. A bare green "verified" beside COMPLETED
    // reads as an endorsement of the work, which this app knows nothing
    // about; the Missions header has always said "ledger verified" for the
    // same fact (Astra's 0.87.1 audit).
    expect(ledgerVerificationLabel(1)).toBe('record incomplete')
    expect(ledgerVerificationLabel(0)).toBe('record readable')
    expect(ledgerVerificationLabel(0)).not.toBe('verified')
  })
})

describe('provenance formatting', () => {
  it('shortens a real mission id rather than inventing a counter', () => {
    expect(shortMissionId('mission_b806b862-bd4d-4fad-a011-efea6140f46d')).toBe('b806b862')
    expect(shortMissionId('b806b862-bd4d')).toBe('b806b862')
  })

  it('labels checkpoints from their 1-based epoch', () => {
    expect(checkpointLabel(1)).toBe('ck_1')
    expect(checkpointLabel(14)).toBe('ck_14')
  })

  it('treats usability as needing both the flag and the probe', () => {
    expect(runtimeIsUsable(runtime())).toBe(true)
    expect(runtimeIsUsable(runtime({ ready: false }))).toBe(false)
    expect(runtimeIsUsable(runtime({ status: 'probe-failed' }))).toBe(false)
  })
})

describe('handoff availability', () => {
  it('offers a handoff only once the mission has a runId to address', () => {
    // The window between submitting and the host's receipt is real, and a
    // control offered in it silently does nothing when clicked.
    expect(handoffAvailability(true, false, false)).toBe('starting')
    expect(handoffAvailability(true, true, false)).toBe('available')
  })

  it('offers nothing when no mission is running', () => {
    expect(handoffAvailability(false, false, false)).toBe('idle')
    // A stale runId from a finished mission must not make the control live.
    expect(handoffAvailability(false, true, false)).toBe('idle')
  })

  it('refuses a second switch while one is in flight', () => {
    // A handoff cannot be undone, so racing two of them would leave the user
    // with runs they never asked for.
    expect(handoffAvailability(true, true, true)).toBe('switching')
    expect(handoffAvailability(false, false, true)).toBe('switching')
  })

  it('says why whenever the control cannot do its job', () => {
    // Silence on a disabled control is the failure being prevented here.
    expect(handoffTitle('starting')).toMatch(/waiting/i)
    expect(handoffTitle('switching')).toMatch(/handing/i)
    expect(handoffTitle('available')).toMatch(/hand this to another/i)
    // Idle needs no explanation: the control is doing its ordinary job.
    expect(handoffTitle('idle')).toBeUndefined()
  })
})

describe('face activity', () => {
  it('decides the face from the same inputs as the label, so the two agree', () => {
    const base = { runtime: undefined, anyRuntimeUsable: true, hasRunningMission: false, pendingApprovals: 0, roleLabel: 'Docs & QA' }
    expect(teammateStatusView({ ...base, hasRunningMission: true, liveActivity: 'thinking' })).toMatchObject({ activity: 'thinking', label: 'Docs & QA · thinking' })
    expect(teammateStatusView({ ...base, hasRunningMission: true })).toMatchObject({ activity: 'working', label: 'Docs & QA · working' })
    expect(teammateStatusView({ ...base, pendingApprovals: 1 })).toMatchObject({ activity: 'waiting', label: 'Docs & QA · waiting on you' })
    expect(teammateStatusView({ ...base, anyRuntimeUsable: false })).toMatchObject({ activity: 'blocked' })
    expect(teammateStatusView({ ...base, recentlyDone: true })).toMatchObject({ activity: 'done', label: 'Docs & QA · done' })
    expect(teammateStatusView(base)).toMatchObject({ activity: 'idle', label: 'Docs & QA · idle' })
  })

  it('carries every state on the presence dot, so a still face still reads', () => {
    expect(facePresenceFor('working')).toBe('working')
    expect(facePresenceFor('approval-needed')).toBe('approval')
    expect(facePresenceFor('blocked')).toBe('blocked')
    expect(facePresenceFor('idle')).toBe('none')
  })
})

describe('a picker group that would not fit', () => {
  // One runtime lists 217 models on a real account. These stand in for them.
  const many = (group: string, count: number, activeIndex?: number) =>
    Array.from({ length: count }, (_, index) => ({
      group,
      label: `model-${String(index)}`,
      tag: (index === activeIndex ? 'ACTIVE' : 'READY') as RouteTag
    }))

  it('leaves a group that fits exactly as it is, and says nothing about it', () => {
    const rows = many('Codex CLI', ROUTE_GROUP_LIMIT)
    const capped = capRouteRows(rows, ROUTE_GROUP_LIMIT, false)
    expect(capped.rows).toEqual(rows)
    expect(capped.hiddenByGroup.size).toBe(0)
  })

  it('caps a long group and counts every row it is not showing', () => {
    const capped = capRouteRows(many('Cursor Agent', 217), ROUTE_GROUP_LIMIT, false)
    expect(capped.rows).toHaveLength(ROUTE_GROUP_LIMIT)
    expect(capped.hiddenByGroup.get('Cursor Agent')).toBe(217 - ROUTE_GROUP_LIMIT)
  })

  it('keeps the runtimes below a long group reachable', () => {
    const capped = capRouteRows(
      [...many('Cursor Agent', 217), ...many('Claude Code', 3)],
      ROUTE_GROUP_LIMIT,
      false
    )
    expect(capped.rows.filter((row) => row.group === 'Claude Code')).toHaveLength(3)
  })

  it('never hides the route you are on, and still shows the cap', () => {
    const capped = capRouteRows(many('Cursor Agent', 217, 180), ROUTE_GROUP_LIMIT, false)
    expect(capped.rows).toHaveLength(ROUTE_GROUP_LIMIT)
    expect(capped.rows.some((row) => row.tag === 'ACTIVE')).toBe(true)
    expect(capped.hiddenByGroup.get('Cursor Agent')).toBe(217 - ROUTE_GROUP_LIMIT)
  })

  it('widens the cap while searching, and still counts what it holds back', () => {
    // It used to lift the cap entirely, and one letter produced 92 rows
    // (0.21.2 QA). A search for `a` narrows nothing; the count is what keeps
    // the narrowing honest.
    const rows = many('Cursor Agent', 217)
    const capped = capRouteRows(rows, ROUTE_GROUP_LIMIT, true)
    expect(capped.rows).toHaveLength(ROUTE_SEARCH_GROUP_LIMIT)
    expect(capped.hiddenByGroup.get('Cursor Agent')).toBe(217 - ROUTE_SEARCH_GROUP_LIMIT)
    // A group that fits under the wider cap is untouched.
    const few = capRouteRows(many('Claude Code', 9), ROUTE_GROUP_LIMIT, true)
    expect(few.rows).toHaveLength(9)
    expect(few.hiddenByGroup.size).toBe(0)
  })
})

describe('what a prune says before it happens', () => {
  const preview = (deleted: number, continuity = 0, running = 0) => ({
    deleted: Array.from({ length: deleted }, (_, i) => `d${String(i)}`),
    keptForContinuity: Array.from({ length: continuity }, (_, i) => `c${String(i)}`),
    keptAsRunning: Array.from({ length: running }, (_, i) => `r${String(i)}`)
  })

  it('states the count that would go, and names everything held back', () => {
    expect(prunePreviewSummary(preview(3, 2, 1))).toBe(
      'Delete 3 turns for good, with 2 turns kept as part of a conversation you are keeping and 1 turn kept because they are running.'
    )
  })

  it('says why nothing would go, rather than just saying nothing', () => {
    expect(prunePreviewSummary(preview(0, 2))).toBe(
      'Nothing would be deleted: 2 turns still part of a conversation you are keeping.'
    )
    expect(prunePreviewSummary(preview(0))).toBe('Nothing is old enough to delete.')
  })

  it('counts one mission as one', () => {
    expect(prunePreviewSummary(preview(1))).toBe('Delete 1 turn for good.')
  })
})

describe('sizes a person can read', () => {
  it('scales to the largest unit that keeps the number short', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(900)).toBe('900 B')
    expect(formatBytes(2048)).toBe('2.0 KB')
    expect(formatBytes(15 * 1024)).toBe('15 KB')
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB')
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe('3.0 GB')
  })

  it('says unknown rather than printing nonsense', () => {
    expect(formatBytes(Number.NaN)).toBe('unknown')
    expect(formatBytes(-1)).toBe('unknown')
  })
})

describe('how far each integration goes', () => {
  it('says live only for the runtimes that can own a mission today', () => {
    expect(RUNTIME_INTEGRATION.codex).toBe('live')
    expect(RUNTIME_INTEGRATION.claude).toBe('live')
    expect(RUNTIME_INTEGRATION.cursor).toBe('live')
  })

  it('does not claim a runtime the host refuses to run', () => {
    // Gemini CLI is discovered and can be signed into, but no mission can run
    // under it, so no screen may draw it as ready.
    expect(RUNTIME_INTEGRATION.gemini).toBe('planned')
    expect(RUNTIME_INTEGRATION.omniroute).toBe('planned')
  })

  it('treats a runtime it has never heard of as planned, not as live', () => {
    expect(integrationOf('something-new')).toBe('planned')
  })
})

describe('the tag on the route you are set to', () => {
  const runtime = (overrides: Partial<PublicRuntimeStatus> = {}): PublicRuntimeStatus => ({
    id: 'codex',
    displayName: 'Codex CLI',
    installed: true,
    version: '0.151.0',
    auth: 'authenticated',
    ready: true,
    status: 'ready',
    ...overrides
  })

  it('says ACTIVE only on a row that could actually run', () => {
    expect(routeRowTag(routeRowStatus(runtime(), 'live', true), true)).toBe('ACTIVE')
  })

  it('does not put ACTIVE on a runtime that is not there', () => {
    // A fresh install points the composer at Codex before discovery has
    // found anything. The row used to read lime ACTIVE directly above the
    // sentence "Codex CLI was not found on this machine."
    const missing = routeRowStatus(runtime({ installed: false, ready: false, status: 'not-installed' }), 'live', true)
    expect(routeRowTag(missing, true)).toBe('NOT INSTALLED')
    const signedOut = routeRowStatus(runtime({ ready: false, status: 'auth-required', auth: 'unauthenticated' }), 'live', true)
    expect(routeRowTag(signedOut, true)).toBe('SIGN IN')
  })
})

describe('a teammate with no work of their own', () => {
  const ready: PublicRuntimeStatus = {
    id: 'claude',
    displayName: 'Claude Code',
    installed: true,
    version: '2.1.0',
    auth: 'authenticated',
    ready: true,
    status: 'ready'
  }

  it('is idle, not blocked, when something could run', () => {
    const view = teammateStatusView({
      runtime: undefined,
      anyRuntimeUsable: true,
      hasRunningMission: false,
      pendingApprovals: 0,
      roleLabel: 'Docs & QA'
    })
    expect(view.status).toBe('idle')
  })

  it('is blocked when nothing is signed in at all', () => {
    const view = teammateStatusView({
      runtime: undefined,
      anyRuntimeUsable: false,
      hasRunningMission: false,
      pendingApprovals: 0,
      roleLabel: 'Docs & QA'
    })
    expect(view.status).toBe('blocked')
  })

  it('is judged against the runtime its own mission is on', () => {
    // Claude is signed in and running this teammate's work. Asking about a
    // different runtime told the person she needed to sign in while her
    // output was streaming on screen.
    const view = teammateStatusView({
      runtime: ready,
      anyRuntimeUsable: true,
      hasRunningMission: true,
      pendingApprovals: 0,
      roleLabel: 'Code & Migrations'
    })
    expect(view.status).toBe('working')
  })
})

describe('a read-only mode, where it is real', () => {
  it('offers Cursor Ask and Plan on Windows too (0.485: its own ask mode holds)', () => {
    // It was refused on Windows: plan mode wrote files when pushed and the
    // sandbox cannot run there. Ask mode refused three pushed writes
    // (2026-09-30), so the mode is real on Windows now.
    expect(modeRunsOn('ask', 'cursor', 'win32')).toBe(true)
    expect(modeRunsOn('plan', 'cursor', 'win32')).toBe(true)
    expect(modesFor('cursor', 'win32')).toEqual(['ask', 'plan', 'accept-edits', 'auto'])
    expect(modeUnavailableReason('ask', 'cursor', 'win32')).toBeUndefined()
  })

  it('and where the sandbox exists', () => {
    expect(modesFor('cursor', 'darwin')).toEqual(['ask', 'plan', 'accept-edits', 'auto'])
  })

  it('leaves the other runtimes alone on every platform', () => {
    expect(modesFor('codex', 'win32')).toEqual(['ask', 'plan', 'accept-edits', 'approve-each', 'auto'])
    // Claude Code stops and asks too since 0.837 (its own default mode, through Locust's permission host).
    expect(modesFor('claude', 'win32')).toEqual(['ask', 'plan', 'accept-edits', 'approve-each', 'auto'])
  })

  it('offers Plan exactly where Ask is', () => {
    // Plan became the fourth MODE rather than a switch beside one (design
    // pass, 2026-09-05), so it is offered exactly where Ask is.
    expect(modeRunsOn('plan', 'gemini')).toBe(false)
    // Antigravity holds Ask through its CLI (0.540), so Plan too.
    expect(modeRunsOn('plan', 'antigravity')).toBe(true)
    expect(modeLabel('plan')).toBe('plan · read-only')
  })
})

describe('which modes a route can actually run', () => {
  it('keeps per-action approvals to the runtime that can stop and ask', () => {
    expect(modeRunsOn('approve-each', 'codex')).toBe(true)
    expect(modeRunsOn('approve-each', 'cursor')).toBe(false)
    expect(modeRunsOn('approve-each', 'claude')).toBe(true)
  })

  it('lets every runtime read and edit', () => {
    for (const runtime of ['codex', 'claude', 'cursor'] as const) {
      expect(modeRunsOn('ask', runtime)).toBe(true)
      // Claude Code included: `--permission-mode acceptEdits` with the editing
      // tools named is a real edit mode, and the argv follows the mode.
      expect(modeRunsOn('accept-edits', runtime)).toBe(true)
    }
  })

  it('names the runtime when it says no, so the menu can explain itself', () => {
    expect(modeUnavailableReason('approve-each', 'cursor')).toContain('Cursor Agent')
    expect(modeUnavailableReason('ask', 'cursor')).toBeUndefined()
  })
})

describe('the order rows are offered in', () => {
  const row = (key: string, group: string) => ({ key, group })

  it('puts what this person has run first, newest first, inside its own runtime', () => {
    const rows = [
      row('cursor:a', 'CURSOR'),
      row('cursor:b', 'CURSOR'),
      row('cursor:c', 'CURSOR')
    ]
    const ordered = orderRouteRows(rows, ['cursor:c', 'cursor:b'])
    expect(ordered.map((entry) => entry.key)).toEqual(['cursor:c', 'cursor:b', 'cursor:a'])
  })

  it('never moves a row out of its runtime', () => {
    const rows = [row('codex:a', 'CODEX'), row('cursor:b', 'CURSOR'), row('codex:c', 'CODEX')]
    const ordered = orderRouteRows(rows, ['cursor:b', 'codex:c'])
    expect(ordered.map((entry) => entry.group)).toEqual(['CODEX', 'CODEX', 'CURSOR'])
    expect(ordered.map((entry) => entry.key)).toEqual(['codex:c', 'codex:a', 'cursor:b'])
  })

  it('leaves the runtime order alone when nothing has been run', () => {
    const rows = [row('codex:a', 'CODEX'), row('cursor:b', 'CURSOR')]
    expect(orderRouteRows(rows, []).map((entry) => entry.key)).toEqual(['codex:a', 'cursor:b'])
  })
})

describe('the curated shortlist', () => {
  it('puts the flagship families above the rest when nothing has been run', () => {
    const rows = [
      { key: 'cursor:aardvark-1', group: 'CURSOR', model: 'aardvark-1' },
      { key: 'cursor:cursor-grok-4.6', group: 'CURSOR', model: 'cursor-grok-4.6' },
      { key: 'cursor:zebra-9', group: 'CURSOR', model: 'zebra-9' }
    ]
    expect(orderRouteRows(rows, []).map((row) => row.model)).toEqual([
      'cursor-grok-4.6',
      'aardvark-1',
      'zebra-9'
    ])
  })

  it('still puts what this person ran above the curated list', () => {
    // Their own ledger outranks my judgement about what people want.
    const rows = [
      { key: 'cursor:cursor-grok-4.6', group: 'CURSOR', model: 'cursor-grok-4.6' },
      { key: 'cursor:zebra-9', group: 'CURSOR', model: 'zebra-9' }
    ]
    expect(orderRouteRows(rows, ['cursor:zebra-9']).map((row) => row.model)).toEqual([
      'zebra-9',
      'cursor-grok-4.6'
    ])
  })

  it('puts a newer flagship family above the one it replaces', () => {
    // The catalog itself is read from each runtime, so a new model needs no
    // code to appear. This curated list is the one exception, and it only
    // decides ORDER -- so the check is that GPT-6 outranks GPT-5.6 rather
    // than that either is present.
    const astra = flagshipRank('gpt-6-astra')
    const sol = flagshipRank('gpt-5.6-sol')
    expect(astra).toBeDefined()
    expect(sol).toBeDefined()
    expect(astra!).toBeLessThan(sol!)
  })

  it('hides nothing: a model on no list is still offered', () => {
    const rows = [{ key: 'cursor:obscure', group: 'CURSOR', model: 'obscure' }]
    expect(orderRouteRows(rows, [])).toHaveLength(1)
    expect(flagshipRank('obscure')).toBeUndefined()
  })
})

describe('searching missions', () => {
  const rows = [
    { title: 'Audit the config', missionId: 'mission_abc12345' },
    { title: 'Rewrite the README', missionId: 'mission_def67890' }
  ]

  it('matches the words a person can see, whatever the case', () => {
    expect(missionsMatching(rows, 'audit').map((row) => row.missionId)).toEqual(['mission_abc12345'])
    expect(missionsMatching(rows, 'README').map((row) => row.missionId)).toEqual(['mission_def67890'])
  })

  it('matches an id, so one pasted from a receipt finds its mission', () => {
    expect(missionsMatching(rows, 'def678').map((row) => row.title)).toEqual(['Rewrite the README'])
  })

  it('shows everything when nothing was typed', () => {
    expect(missionsMatching(rows, '   ')).toHaveLength(2)
  })

  it('shows nothing when nothing matches, rather than everything', () => {
    expect(missionsMatching(rows, 'nonsense')).toEqual([])
  })
})

describe('the sidebar lists conversations, not turns', () => {
  const turn = (id: string, title: string, phase: string, rootId?: string, parentId?: string) => ({
    missionId: id,
    title,
    phase,
    integrityIssueCount: 0,
    ...(rootId === undefined ? {} : { rootId }),
    ...(parentId === undefined ? {} : { parentId })
  })

  it('collapses a three-turn exchange into one row', () => {
    const rows = collapseConversations([
      turn('m1', 'test', 'completed'),
      turn('m2', 'test', 'completed', 'm1', 'm1'),
      turn('m3', 'test', 'completed', 'm1', 'm2')
    ])
    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({ missionId: 'm3', turns: 3 })
    expect(rows[0]?.memberIds).toEqual(['m1', 'm2', 'm3'])
  })

  it('names the row for what the person typed first, and opens its newest turn', () => {
    const rows = collapseConversations([
      turn('m1', 'set up the parser', 'completed'),
      turn('m2', 'now add tests', 'completed', 'm1', 'm1')
    ])
    expect(rows[0]?.title).toBe('set up the parser')
    expect(rows[0]?.missionId).toBe('m2')
  })

  it('reads as running while any turn of it is running', () => {
    const rows = collapseConversations([
      turn('m1', 'x', 'completed'),
      turn('m2', 'x', 'running', 'm1', 'm1')
    ])
    expect(rows[0]?.phase).toBe('running')
  })

  it('surfaces an integrity issue from any turn, not just the last', () => {
    const rows = collapseConversations([
      { ...turn('m1', 'x', 'completed'), integrityIssueCount: 2 },
      turn('m2', 'x', 'completed', 'm1', 'm1')
    ])
    expect(rows[0]?.integrityIssueCount).toBe(2)
  })

  it('leaves an ordinary single-run mission exactly as it was', () => {
    const rows = collapseConversations([turn('m1', 'one off', 'completed')])
    expect(rows[0]).toMatchObject({ missionId: 'm1', title: 'one off', turns: 1 })
  })

  it('keeps separate conversations separate, in the order they were read', () => {
    const rows = collapseConversations([
      turn('a1', 'first', 'completed'),
      turn('b1', 'second', 'completed'),
      turn('a2', 'first', 'completed', 'a1', 'a1')
    ])
    expect(rows.map((row) => row.title)).toEqual(['first', 'second'])
  })

  it('still returns a row when a hand-edited chain has no leaf', () => {
    // Both turns claim to continue the other. Nothing is dropped.
    const rows = collapseConversations([
      turn('m1', 'x', 'completed', 'm1', 'm2'),
      turn('m2', 'x', 'completed', 'm1', 'm1')
    ])
    expect(rows).toHaveLength(1)
    expect(rows[0]?.turns).toBe(2)
  })
})

describe('searching routes the way names are said', () => {
  it('folds the punctuation a CLI spells a model with', () => {
    expect(routeSearchText('opencode/muse-spark-1.3-contributor-free')).toBe('opencode muse spark 1 3 contributor free')
    expect(routeSearchText('  Muse   Spark ')).toBe('muse spark')
    expect(routeSearchText('OpenCode muse-spark-1.3').includes(routeSearchText('muse spark 1.3'))).toBe(true)
  })
})

describe("the words for what a teammate does", () => {
  it("uses a Custom teammate's own title, and the role name for everyone else", async () => {
    const { roleLabelOf } = await import('../../shared/ipc.js')
    expect(roleLabelOf({ role: 'Custom', roleTitle: 'Release manager' })).toBe('Release manager')
    expect(roleLabelOf({ role: 'Custom' })).toBe('Custom')
    expect(roleLabelOf({ role: 'Custom', roleTitle: '   ' })).toBe('Custom')
    expect(roleLabelOf({ role: 'Docs & QA', roleTitle: 'ignored' })).toBe('Docs & QA')
  })
})

describe('the route a mission starts on', () => {
  const models: readonly PublicModel[] = [
    {
      id: 'cursor-grok-4.6-high-fast',
      runtime: 'cursor',
      displayName: 'cursor-grok-4.6',
      description: '',
      supportedEfforts: ['low', 'high', 'high-fast'],
      variants: { low: 'cursor-grok-4.6-low', high: 'cursor-grok-4.6-high', 'high-fast': 'cursor-grok-4.6-high-fast' }
    },
    { id: 'sonnet', runtime: 'claude', displayName: 'Sonnet', description: '', supportedEfforts: ['low', 'medium', 'high'] }
  ]

  it('turns an effort into the variant it names and sends no effort beside it', () => {
    expect(startRoute(models, 'cursor', 'cursor-grok-4.6-high-fast', 'low')).toEqual({ model: 'cursor-grok-4.6-low' })
  })

  it('drops an effort the variants do not name rather than refuse the run', () => {
    expect(startRoute(models, 'cursor', 'cursor-grok-4.6-high-fast', 'medium')).toEqual({ model: 'cursor-grok-4.6-high-fast' })
  })

  it('finds the family when the id in hand is one of its variants, not its own id', () => {
    // THE regression (Colin, 2026-09-07: "before that model and effort was
    // working perfectly fine"). A Cursor family never listed without an effort
    // takes the FIRST VARIANT SEEN as its id, so the id in hand is routinely a
    // sibling variant of the one the effort names. Matching on `id` alone found
    // no family, the effort travelled as a separate value, and Cursor refused
    // the run: "Cursor Agent takes no effort level. Nothing was recorded."
    expect(startRoute(models, 'cursor', 'cursor-grok-4.6-low', 'high')).toEqual({ model: 'cursor-grok-4.6-high' })
    // And the same path must still never hand Cursor a loose effort, whichever
    // variant it started from -- that is the thing that actually breaks a run.
    for (const from of ['cursor-grok-4.6-low', 'cursor-grok-4.6-high', 'cursor-grok-4.6-high-fast']) {
      for (const effort of ['low', 'high', 'high-fast', 'medium', 'nonsense']) {
        expect(startRoute(models, 'cursor', from, effort).effort).toBeUndefined()
      }
    }
  })

  it('never sends an effort to a model that advertises none', () => {
    // Colin, 2026-09-08: choosing OpenCode while an effort was set failed the
    // run -- "OpenCode takes no effort level. Nothing was recorded." The effort
    // was a leftover from the previous route. Every place that changes a route
    // has to clear it, and adopting a teammate's own route did not; this is the
    // one path every start takes.
    const withNone: readonly PublicModel[] = [
      ...models,
      { id: 'opencode/muse-spark-1.3', runtime: 'opencode', displayName: 'Muse Spark', description: '', supportedEfforts: [] }
    ]
    expect(startRoute(withNone, 'opencode', 'opencode/muse-spark-1.3', 'medium')).toEqual({
      model: 'opencode/muse-spark-1.3'
    })
  })

  it('sends an OpenCode model only an effort it lists', () => {
    // OpenCode takes any variant name and silently ignores one the model does
    // not list (measured 2026-09-25), so a leftover would be shown as applied
    // and never be (A6.5).
    const ling: readonly PublicModel[] = [
      ...models,
      { id: 'opencode/ling', runtime: 'opencode', displayName: 'Ling', description: '', supportedEfforts: ['low', 'medium', 'high'] }
    ]
    expect(startRoute(ling, 'opencode', 'opencode/ling', 'high')).toEqual({ model: 'opencode/ling', effort: 'high' })
    expect(startRoute(ling, 'opencode', 'opencode/ling', 'xhigh')).toEqual({ model: 'opencode/ling' })
  })

  it('still sends an effort for a model the catalogue has never heard of', () => {
    // The control, and the reason the guard checks for a KNOWN empty list. An
    // account default, or a probe that has not answered yet, says nothing about
    // efforts -- dropping one there would quietly downgrade a Claude run that
    // asked for `high`.
    expect(startRoute(models, 'claude', 'account-default', 'high')).toEqual({
      model: 'account-default',
      effort: 'high'
    })
  })

  it('sends the effort as itself where the runtime takes it as a flag', () => {
    expect(startRoute(models, 'claude', 'sonnet', 'medium')).toEqual({ model: 'sonnet', effort: 'medium' })
    expect(startRoute(models, 'claude', 'sonnet', undefined)).toEqual({ model: 'sonnet' })
  })
})

describe('the Auto mode, which has to be switched on before it is offered', () => {
  it('is always offered, whatever the workspace switch says', () => {
    // Colin, 2026-09-06: "always allow auto to be chosen from the permission
    // dropdown, we want the user experience to be fluid." Picking it is what
    // turns the switch on; the host still asks the switch as a run starts.
    expect(modesFor('codex', 'win32')).toContain('auto')
  })

  it('is offered on every runtime Locust has a handle on, Antigravity (0.540) and Muse (0.773) included', () => {
    for (const runtime of ['codex', 'claude', 'cursor', 'opencode', 'copilot', 'antigravity', 'muse'] as const) {
      expect(modesFor(runtime, 'win32')).toContain('auto')
    }
  })

  it('is never the mode a route falls back to', () => {
    // The fallback takes the first offered mode, so Auto being last in the
    // list is what keeps a refused choice from widening into the widest one.
    expect(modesFor('codex', 'win32')[0]).not.toBe('auto')
    expect(modesFor('cursor', 'win32')[0]).not.toBe('auto')
  })

  it('has its own phrase for what a run was allowed', () => {
    // "may edit the workspace" on a run that could touch the whole machine
    // would understate what happened, on the receipt that exists to say it.
    expect(sandboxPhrase('full-access')).toBe('may edit anything on this machine')
    expect(sandboxPhrase('workspace-write')).toBe('may edit the workspace')
    expect(sandboxPhrase('read-only')).toBe('read-only')
    expect(sandboxPhrase(undefined)).toBe('read-only')
  })

  it('names the sign-in a run used in words, not by its route id', () => {
    // The inspector's Details said "codex-account:default" (the design
    // review: "Details in plain words").
    expect(accountPhrase({ runtime: 'codex', model: 'account-default', resolvedRouteId: 'codex-account:default' })).toBe('your Codex CLI sign-in')
    expect(accountPhrase({ runtime: 'antigravity', model: 'flash', resolvedRouteId: 'antigravity:hub' })).toBe('through the Antigravity app')
    // A free model needs no account; "your OpenCode sign-in" would say it did.
    expect(accountPhrase({ runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', resolvedRouteId: 'opencode-account:default' })).toBe('none — a free model')
  })

  it('says what it allowed, not what it was called', () => {
    expect(modeLabel('auto')).toBe('auto \u00b7 whole machine')
  })
})

describe('who the composer addresses when a conversation is opened', () => {
  const owners = { mission_booty: 'tm_booty', mission_wren: 'tm_wren' }

  it("follows the mission's owner, so the header and the composer agree", () => {
    // Clicking Booty's message in an exchange opened Booty's run under a
    // header naming Booty, while the composer still said "Message Wren..."
    // and Wren's card stayed lit (0.35.0 targeted QA).
    expect(ownerToSelect('mission_booty', owners, 'tm_wren')).toBe('tm_booty')
  })

  it("prefers the run's own owner, which is what the header uses", () => {
    // The header reads `run.teammateId ?? missionOwners[id]`; this used to
    // read only the second. They agreed everywhere anyone could trace, except
    // while a run has started and the host has not recorded it yet -- the
    // exact window that produced the 0.35.0 bug (QA, 2026-09-06).
    expect(ownerToSelect('mission_new', {}, 'tm_wren', 'tm_booty')).toBe('tm_booty')
    // It is the first answer, not an override of a recorded one it disagrees
    // with: no such case exists, and if one appears the run is the fresher.
    expect(ownerToSelect('mission_booty', { mission_booty: 'tm_booty' }, 'tm_wren', undefined)).toBe('tm_booty')
  })

  it('addresses nobody for a conversation nobody owns (0.551)', () => {
    // Keeping Wren sent Colin's reply in a teammate-less conversation to the
    // teammate picked before, queued behind that teammate's other conversation.
    expect(ownerToSelect('mission_nobody', owners, 'tm_wren')).toBeUndefined()
    expect(ownerToSelect('mission_nobody', owners, undefined)).toBeUndefined()
  })
})

describe('the route you are on sorts first in its group', () => {
  // "account-default" is neither recently used nor a flagship name, so it fell
  // into the bottom band -- below six models and behind a "1 more model · type
  // to search them" line -- which is where a new person's own route was
  // hiding. The row the composer points at is the last thing that should need
  // finding.
  const row = (key: string, tag?: string): { key: string; group: string; model: string; tag?: string } => ({
    key,
    group: 'CODEX CLI',
    model: key,
    ...(tag === undefined ? {} : { tag })
  })

  it('puts the active row above flagships and everything else', () => {
    const ordered = orderRouteRows(
      [row('gpt-6-astra'), row('gpt-5.6-sol'), row('account-default', 'ACTIVE')],
      []
    )
    expect(ordered[0]?.key).toBe('account-default')
  })

  it('still beats a recently used row, because it is the one in use now', () => {
    const ordered = orderRouteRows(
      [row('gpt-5.6-sol'), row('account-default', 'ACTIVE')],
      ['gpt-5.6-sol']
    )
    expect(ordered[0]?.key).toBe('account-default')
  })

  it('leaves the order alone when nothing is active', () => {
    const ordered = orderRouteRows([row('gpt-6-astra'), row('gpt-5.6-sol')], ['gpt-5.6-sol'])
    expect(ordered[0]?.key).toBe('gpt-5.6-sol')
  })
})

describe('effort reads as a constant', () => {
  // A model switch used to clear effort to undefined, so picking a model
  // left the control showing nothing at all -- Colin, 2026-09-07: "if the
  // user just clicks the model it defaults to no effort with no effort
  // screen, have it be a constant".
  it('prefers medium, the ordinary setting on every runtime here', () => {
    expect(defaultEffort(['low', 'medium', 'high', 'xhigh', 'max'])).toBe('medium')
  })

  it('takes the middle when there is no medium', () => {
    expect(defaultEffort(['low', 'high', 'max'])).toBe('high')
  })

  it('biases upward on a two-level model, as picking medium out of five does', () => {
    expect(defaultEffort(['low', 'high'])).toBe('high')
  })

  it('has nothing to show when the model reports no levels', () => {
    expect(defaultEffort([])).toBeUndefined()
  })

  it('carries a chosen level across a model switch that supports it', () => {
    expect(effortAfterRouteChange('xhigh', ['low', 'medium', 'xhigh'])).toBe('xhigh')
  })

  it('falls back to the new model default rather than to nothing', () => {
    // The old behaviour sent nothing AND showed nothing. Guarding against an
    // unadvertised level does not require an empty control.
    expect(effortAfterRouteChange('ultra', ['low', 'medium', 'high'])).toBe('medium')
  })

  it('still shows nothing when the new model reports no levels', () => {
    expect(effortAfterRouteChange('high', [])).toBeUndefined()
  })
})

describe('the level on the chip is the level the run is given', () => {
  // For one release it was not. `effort` starts undefined and is assigned only
  // when someone opens the dropdown, while the chip rendered
  // `effort ?? defaultEffort(supported)`. So from every launch the chip said
  // "medium" and the run was started with no effort argument at all. The two
  // now share one expression; these tests pin them together.
  const models = [
    { id: 'gpt-5.6-sol', runtime: 'codex' as const, displayName: 'Sol', description: '', supportedEfforts: ['low', 'medium', 'high'] },
    { id: 'auto', runtime: 'copilot' as const, displayName: 'Auto', description: '', supportedEfforts: [] }
  ]
  // Exactly what Composer.tsx renders on the chip.
  const displayed = (modelId: string, swarm: boolean, chosen: string | undefined): string | undefined => {
    const supported = models.find((model) => model.id === modelId)?.supportedEfforts ?? []
    return swarm ? supported[supported.length - 1] : chosen ?? defaultEffort(supported)
  }

  it('agree when nobody has chosen a level', () => {
    expect(swarmEffortFor(models, 'gpt-5.6-sol', false, undefined, 'codex')).toBe('medium')
    expect(swarmEffortFor(models, 'gpt-5.6-sol', false, undefined, 'codex')).toBe(displayed('gpt-5.6-sol', false, undefined))
  })

  it('agree when someone has', () => {
    expect(swarmEffortFor(models, 'gpt-5.6-sol', false, 'high', 'codex')).toBe(displayed('gpt-5.6-sol', false, 'high'))
  })

  it('agree under swarm, which overrides both', () => {
    expect(swarmEffortFor(models, 'gpt-5.6-sol', true, 'low', 'codex')).toBe('high')
    expect(swarmEffortFor(models, 'gpt-5.6-sol', true, 'low', 'codex')).toBe(displayed('gpt-5.6-sol', true, 'low'))
  })

  it("agree on a Cursor family, whose id already names a level", () => {
    // Cursor's Opus 5.5 stands on its `-medium` variant (Cursor lists that one
    // under the bare name); the chip reads the level from the id, and so must
    // the run. And a family whose id is its low variant is not run at medium.
    const cursor: readonly PublicModel[] = [
      {
        id: 'claude-opus-5-5-medium',
        runtime: 'cursor',
        displayName: 'Claude Opus 5.5 1M',
        description: '',
        supportedEfforts: ['low', 'medium', 'high', 'max'],
        variants: { low: 'claude-opus-5-5-low', medium: 'claude-opus-5-5-medium', high: 'claude-opus-5-5-high', max: 'claude-opus-5-5-max' },
        defaultEffort: 'medium'
      },
      {
        id: 'grok-4.7-low',
        runtime: 'cursor',
        displayName: 'Grok 4.7',
        description: '',
        supportedEfforts: ['low', 'medium', 'high'],
        variants: { low: 'grok-4.7-low', medium: 'grok-4.7-medium', high: 'grok-4.7-high' }
      }
    ]
    expect(swarmEffortFor(cursor, 'claude-opus-5-5-medium', false, undefined, 'cursor')).toBe('medium')
    expect(swarmEffortFor(cursor, 'grok-4.7-low', false, undefined, 'cursor')).toBe('low')
    expect(swarmEffortFor(cursor, 'grok-4.7-low', false, 'high', 'cursor')).toBe('high')
    expect(swarmEffortFor(cursor, 'claude-opus-5-5-medium', true, undefined, 'cursor')).toBe('max')
  })

  it("start a new route on the level the runtime names as its default, where it names one", () => {
    // Cursor lists Kimi K3's max variant under the bare name, and Opus 4.6's
    // high one; Locust's own rule (medium, else the middle) would have said
    // high and max.
    expect(defaultEffort(['low', 'high', 'max'], 'max')).toBe('max')
    expect(defaultEffort(['high', 'max'], 'high')).toBe('high')
    expect(defaultEffort(['high', 'max'])).toBe('max')
    // A default the model does not offer is not a default.
    expect(defaultEffort(['low', 'medium', 'high'], 'ultra')).toBe('medium')
    expect(effortAfterRouteChange('xhigh', ['high', 'max'], 'high')).toBe('high')
    expect(effortAfterRouteChange('max', ['high', 'max'], 'high')).toBe('max')
  })

  it('agree on a model with no levels: both say nothing, and nothing is sent', () => {
    expect(swarmEffortFor(models, 'auto', false, undefined, 'copilot')).toBeUndefined()
    expect(displayed('auto', false, undefined)).toBeUndefined()
  })

  it('never invents a level for a model the catalogue does not know', () => {
    expect(swarmEffortFor(models, 'not-in-catalogue', false, undefined, 'codex')).toBeUndefined()
  })
})

describe('a machine with nothing installed', () => {
  // A first outside tester, 2026-09-07: the welcome screen sold OpenCode
  // ("one install and you have a working teammate") above a composer claiming
  // `Codex CLI / account-default`, on a machine with no CLIs at all. A
  // default that names something absent is the defect defaultRoute exists to
  // fix; naming the runtime the app is recommending is at least a claim the
  // next click can make true.
  const runtime = (id: string, ready: boolean): PublicRuntimeStatus =>
    ({ id, displayName: id, ready, status: ready ? 'ready' : 'missing' }) as unknown as PublicRuntimeStatus

  it('names the runtime the first-run screen recommends, not one that is absent', () => {
    expect(defaultRoute([runtime('codex', false), runtime('opencode', false)]).runtime).toBe('opencode')
  })

  it('names nothing else when discovery has not reported at all', () => {
    expect(defaultRoute([]).runtime).toBe('opencode')
  })

  it('still prefers a runtime that can actually run right now', () => {
    expect(defaultRoute([runtime('codex', true), runtime('opencode', false)]).runtime).toBe('codex')
  })

  /*
   * Colin, 2026-09-23: "the pitch is full model control in a great UI, not
   * free. Most users will be on Claude or Codex plans; OpenCode is a bonus"
   * -- don't start on OpenCode when Claude or Codex is signed in.
   */
  it('and among those, Claude Code first, then Codex, then OpenCode', () => {
    expect(defaultRoute([runtime('opencode', true), runtime('codex', true), runtime('claude', true)]).runtime).toBe('claude')
    expect(defaultRoute([runtime('opencode', true), runtime('codex', true), runtime('claude', false)]).runtime).toBe('codex')
    expect(defaultRoute([runtime('codex', false), runtime('opencode', true), runtime('claude', false)]).runtime).toBe('opencode')
    // Anything else that can run, when none of the three can.
    expect(defaultRoute([runtime('cursor', true), runtime('opencode', false)]).runtime).toBe('cursor')
  })
})

describe('a model id read beside the runtime that serves it', () => {
  // The composer read `OpenCode / opencode/ling-3.0-flash-fin-free` and
  // truncated to `OpenCode / opencode/big-pic...` -- the runtime named twice,
  // and the part that identifies the model cut off (outside tester,
  // 2026-09-07).
  it('drops a provider that merely repeats the runtime', () => {
    expect(modelLabelFor('opencode', 'opencode/ling-3.0-flash-fin-free')).toBe('ling-3.0-flash-fin-free')
  })

  it('keeps a provider that is real information', () => {
    // OpenCode aggregates providers: which one answers is worth knowing.
    expect(modelLabelFor('opencode', 'anthropic/claude-sonnet-4')).toBe('anthropic/claude-sonnet-4')
  })

  it('matches case-insensitively, as the ids are written either way', () => {
    expect(modelLabelFor('OpenCode', 'opencode/big-pickle')).toBe('big-pickle')
  })

  it('leaves an id with no provider alone', () => {
    expect(modelLabelFor('claude', 'sonnet')).toBe('sonnet')
  })

  it('leaves a leading slash alone rather than eating the id', () => {
    expect(modelLabelFor('opencode', '/weird')).toBe('/weird')
  })
})

describe('an effort that lives inside the model id', () => {
  // Cursor lists every effort as its own model and its builder refuses a
  // separate effort. A routine taught on Cursor stored BOTH -- `{model:
  // "composer-2.5-fast", effort: "fast"}` -- and threw on every replay,
  // measured 2026-09-07 in routine-smoke: 2 ledgers where 3 were expected,
  // because step 1 never opened a mission.
  const cursorFamily = {
    runtime: 'cursor',
    id: 'composer-2.5',
    displayName: 'Composer 2.5',
    supportedEfforts: ['fast'],
    variants: { fast: 'composer-2.5-fast' }
  } as unknown as PublicModel
  const codexModel = {
    runtime: 'codex',
    id: 'gpt-5.6',
    displayName: 'GPT-5.6',
    supportedEfforts: ['low', 'high']
  } as unknown as PublicModel

  it('is recognised through the family id', () => {
    expect(effortIsInModelId([cursorFamily, codexModel], 'cursor', 'composer-2.5')).toBe(true)
  })

  it('is recognised through the variant id a run actually resolves to', () => {
    // The stored route carries the RESOLVED id, which is not a row of its own.
    expect(effortIsInModelId([cursorFamily, codexModel], 'cursor', 'composer-2.5-fast')).toBe(true)
  })

  it('is not claimed for a runtime that takes the effort beside the model', () => {
    // Codex must still store and send its level -- that is the whole feature.
    expect(effortIsInModelId([cursorFamily, codexModel], 'codex', 'gpt-5.6')).toBe(false)
  })

  it('is not claimed for a model nothing in the catalog folds', () => {
    expect(effortIsInModelId([codexModel], 'cursor', 'composer-2.5-fast')).toBe(false)
  })
})
