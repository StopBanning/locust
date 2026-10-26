import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { MissionApprovalRequest, PublicRuntimeStatus, PublicTeammate } from '../../shared/ipc.js'
import { runtimeDisplayName } from '../../shared/runtimes.js'
import { ApprovalCard } from './components/ApprovalCard.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { RoutePicker } from './components/RoutePicker.js'
import { RuntimeMark } from './components/RuntimeMark.js'
import { Sidebar } from './components/Sidebar.js'
import { MARKED_FACE_MIN, TeammateBot } from './components/TeammateBot.js'
import { RUNTIME_MARKS } from './runtimeMarks.js'
import MARKS_SOURCE from './runtimeMarks.ts?raw'
import MARK_COMPONENT from './components/RuntimeMark.tsx?raw'

/**
 * THE RUNTIMES WEAR THEIR MARKS (0.383).
 *
 * Colin, 2026-09-26, on Orca: it "use[s] alot of logos to signify which model
 * is being used by what". Locust spelled every runtime out in mono text; a
 * mark reads before a word does. His answer to marks on the chip, the rows,
 * the board, the approval cards and Home: "i like all your other ideas, all
 * good."
 */

const EVERY_RUNTIME = ['codex', 'claude', 'cursor', 'gemini', 'opencode', 'copilot', 'antigravity', 'muse', 'kilo', 'acp'] as const

describe('the marks', () => {
  it('cover every runtime a teammate can run on', () => {
    for (const runtime of EVERY_RUNTIME) {
      const mark = RUNTIME_MARKS[runtime]
      expect(mark.paths.length, runtime).toBeGreaterThan(0)
      expect(mark.source.length, runtime).toBeGreaterThan(0)
    }
    expect(Object.keys(RUNTIME_MARKS).sort()).toEqual([...EVERY_RUNTIME].sort())
  })

  it('are bundled: nothing in the module is a place to fetch one from', () => {
    // Orca asks Google's favicon service for every logo, which tells Google
    // which tools a person uses. A local-first app draws its own.
    const source = MARKS_SOURCE
    expect(source).not.toMatch(/https?:\/\//)
    // Named sources in the comments are documentation; what is DRAWN is
    // path data and nothing else.
    expect(source).not.toMatch(/fetch\(|new Image|<img|<image|xlink:href|data:image/)
    for (const [runtime, mark] of Object.entries(RUNTIME_MARKS)) {
      for (const d of mark.paths) expect(d, runtime).toMatch(/^[Mm][MmLlHhVvCcSsQqTtAaZz0-9.,\s-]*$/)
    }
    // The component draws the paths inline, and loads nothing either.
    const component = MARK_COMPONENT
    expect(component).not.toMatch(/https?:\/\/|fetch\(|<img|<image/)
  })

  it("draw Codex as OpenAI's blossom, as Colin asked (0.385), in the words' colour", () => {
    // Colin, 2026-09-26, with the blossom attached: "lets use the actual
    // codex/openai logo" -- where 0.383 drew Codex as its banner's `>_`.
    const codex = RUNTIME_MARKS.codex
    expect(codex.paths).toHaveLength(1)
    expect(codex.paths[0]).toMatch(/^M22\.2819 9\.8211a5\.9847 5\.9847 0 0 0-\.5157-4\.9108/)
    expect(codex.source).toContain('OpenAI')
    expect(codex.gradient).toBeUndefined()
    const html = renderToStaticMarkup(<RuntimeMark runtime="codex" />)
    expect(html).toContain('fill="currentColor"')
  })

  it("are decorative beside a name, and named where they stand in for one", () => {
    const plain = renderToStaticMarkup(<RuntimeMark runtime="claude" />)
    expect(plain).toContain('aria-hidden="true"')
    expect(plain).not.toContain('role="img"')
    // No colour of its own on the element: the stylesheet decides it.
    expect(plain).not.toContain('style=')

    const named = renderToStaticMarkup(<RuntimeMark runtime="copilot" label="Copilot CLI" />)
    expect(named).toContain('role="img"')
    expect(named).toContain('aria-label="Copilot CLI"')
    expect(named).toContain('<title>Copilot CLI</title>')
  })

  it("draw Gemini in Google's own gradient, its stops tokens, each instance its own", () => {
    const html = renderToStaticMarkup(
      <>
        <RuntimeMark runtime="gemini" />
        <RuntimeMark runtime="gemini" />
      </>
    )
    const ids = [...html.matchAll(/<radialGradient id="([^"]+)"/g)].map((match) => match[1])
    expect(ids).toHaveLength(2)
    expect(new Set(ids).size).toBe(2)
    for (const id of ids) expect(html).toContain(`fill="url(#${String(id)})"`)
    for (const token of ['--lc-mark-gemini-violet', '--lc-mark-gemini-blue', '--lc-mark-gemini-cyan']) expect(html).toContain(`stop-color:var(${token})`)
    expect(html).toContain('viewBox="0 0 28 28"')
  })

  it('grey Gemini with its words: no gradient when muted', () => {
    const html = renderToStaticMarkup(<RuntimeMark runtime="gemini" muted />)
    expect(html).toContain('class="lc-runtimemark is-muted"')
    expect(html).not.toContain('radialGradient')
    expect(html).toContain('fill="currentColor"')
  })

  it("draw Antigravity's own arch and Meta's mark for Muse Code, not Locust's monograms", () => {
    expect(RUNTIME_MARKS.antigravity.viewBox).toBe('14 14.2 83.6 83.6')
    expect(RUNTIME_MARKS.antigravity.source).toContain("Antigravity's own")
    expect(RUNTIME_MARKS.muse.source).toContain('Meta')
    // Every mark is its maker's; none is a glyph Locust drew -- except two that have no mark we may copy.
    // An agent the person added (0.777) has no one maker, so Locust's own plug says "one you added".
    // Kilo's wordmark is not copied; the row wears a K drawn here, and the source says so.
    for (const [runtime, mark] of Object.entries(RUNTIME_MARKS)) {
      if (runtime === 'acp') expect(mark.source).toMatch(/^Locust: a plug/)
      else if (runtime === 'kilo') expect(mark.source).toMatch(/^Locust: a K/)
      else expect(mark.source, runtime).not.toMatch(/^Locust/)
    }
  })

  it('draw nothing for what no teammate runs on', () => {
    expect(renderToStaticMarkup(<RuntimeMark runtime="omniroute" />)).toBe('')
  })
})

const ready = (id: PublicRuntimeStatus['id']): PublicRuntimeStatus => ({
  id,
  displayName: runtimeDisplayName(id as (typeof EVERY_RUNTIME)[number]),
  installed: true,
  version: '1.0.0',
  auth: 'authenticated',
  ready: true,
  status: 'ready'
})
const signedOut = (id: PublicRuntimeStatus['id']): PublicRuntimeStatus => ({ ...ready(id), auth: 'unauthenticated', ready: false, status: 'auth-required' })

function composerChip(runtimes: readonly PublicRuntimeStatus[], runtime: string, model: string): string {
  const props = {
    runtimes,
    limitedRuntimes: new Map(),
    discoveryPhase: 'ready',
    models: [],
    resolvedModels: new Map(),
    route: { runtime, model, routeId: `${runtime}:${model}` },
    mode: 'accept-edits',
    onModeChange: () => undefined,
    onRouteChange: () => undefined,
    onSend: () => undefined,
    running: false,
    platform: 'win32'
  } as unknown as ComposerProps
  const html = renderToStaticMarkup(<Composer {...props} />)
  const start = html.indexOf('aria-haspopup="listbox"')
  return html.slice(start, html.indexOf('</button>', start))
}

describe("the composer's route chip", () => {
  it("wears the runtime's mark where the ready dot was", () => {
    const chip = composerChip([ready('claude')], 'claude', 'sonnet')
    expect(chip).toContain('data-runtime="claude"')
    expect(chip).not.toContain('lc-dot')
  })

  it('greys the mark when that runtime cannot take work -- the one thing the dot said', () => {
    const chip = composerChip([ready('codex'), signedOut('claude')], 'claude', 'sonnet')
    expect(chip).toMatch(/class="lc-runtimemark is-muted"[^>]*data-runtime="claude"/)
  })

  it("shows no maker's mark on a model of the person's own, and none when nothing is connected", () => {
    const own = composerChip([ready('opencode')], 'opencode', 'own-1a2b3c4d/acme-70b')
    expect(own).not.toContain('lc-runtimemark')
    expect(own).toContain('lc-dot')
    const nothing = composerChip([signedOut('claude')], 'claude', 'sonnet')
    expect(nothing).toContain('No AI agent')
    expect(nothing).not.toContain('lc-runtimemark')
  })
})

describe('the other places a runtime is named', () => {
  it('an approval card says which program is asking with its mark', () => {
    const request: MissionApprovalRequest = {
      approvalId: 'ap_1',
      runId: 'run_1',
      missionId: 'mission_1',
      runtime: 'copilot',
      kind: 'command',
      summary: 'Run npm test',
      detail: 'npm test',
      cwd: 'C:\\work',
      requestedAt: '2026-09-26T04:00:00.000Z',
      blocking: true
    } as MissionApprovalRequest
    const html = renderToStaticMarkup(<ApprovalCard request={request} onDecide={() => undefined} onAnswer={() => undefined} busy={false} />)
    expect(html).toMatch(/data-runtime="copilot"[^]*Copilot CLI/)
  })

  it("the picker's runtime groups carry their marks", () => {
    const html = renderToStaticMarkup(
      <RoutePicker
        runtimes={[ready('claude'), ready('cursor')]}
        limitedRuntimes={new Map()}
        models={[]}
        resolvedModels={new Map()}
        recentRoutes={[]}
        active={{ runtime: 'claude', model: 'account-default' }}
        onSelect={() => undefined}
        onClose={() => undefined}
      />
    )
    expect(html).toMatch(/class="lc-picker__group"><svg[^>]*data-runtime="claude"/)
    expect(html).toMatch(/class="lc-picker__group"><svg[^>]*data-runtime="cursor"/)
  })
})

/*
 * THE SIDEBAR'S FACES WEAR THEIR MARKS.
 *
 * The sidebar names nobody's route in words -- the faces are the identity, and
 * the route line went to the Team screen -- so on the one surface that shows
 * everyone at once, which model is who could not be seen at all. A face now
 * carries its runtime's mark on the body's lower left, opposite the presence
 * dot: what it is doing on one side, what it runs on on the other.
 */
const at = '2026-09-26T05:00:00.000Z'
const teammate = (teammateId: string, name: string, route?: PublicTeammate['route']): PublicTeammate =>
  ({ teammateId, name, hue: 'lime', role: 'Code & Migrations', createdAt: at, avatar: seedAvatar(teammateId), ...(route === undefined ? {} : { route }) }) as PublicTeammate

describe("a teammate's face", () => {
  it('wears the mark of the runtime it is given, on a badge', () => {
    const html = renderToStaticMarkup(<TeammateBot hue="lime" avatar={seedAvatar('tm_wren')} size={26} runtime="claude" />)
    expect(html).toMatch(/class="lc-bot__mark" data-runtime="claude"><svg[^>]*data-runtime="claude"/)
  })

  it('wears none when it is too small for a mark to be a mark, or is given no runtime', () => {
    expect(renderToStaticMarkup(<TeammateBot hue="lime" avatar={seedAvatar('tm_wren')} size={MARKED_FACE_MIN - 1} runtime="claude" />)).not.toContain('lc-bot__mark')
    expect(renderToStaticMarkup(<TeammateBot hue="lime" avatar={seedAvatar('tm_wren')} size={26} />)).not.toContain('lc-bot__mark')
  })

  it("in the sidebar's row of faces, says which model is who -- and nothing on a model of their own", () => {
    const noop = (): void => undefined
    const html = renderToStaticMarkup(
      <Sidebar
        runtimes={[]}
        missions={[]}
        teammates={[
          teammate('tm_wren', 'Wren', { runtime: 'claude', model: 'sonnet', mode: 'accept-edits' }),
          teammate('tm_juno', 'Juno', { runtime: 'codex', model: 'account-default', mode: 'accept-edits' }),
          teammate('tm_acme', 'Acme', { runtime: 'opencode', model: 'own-1a2b3c4d/acme-70b', mode: 'accept-edits' }),
          teammate('tm_new', 'Newt')
        ]}
        viewByTeammate={{}}
        routineStepByTeammate={{}}
        missionOwners={{}}
        selectedMissionId={undefined}
        selectedTeammateId={undefined}
        onSelectMission={noop}
        onMissionMenu={noop}
        onTeammateMenu={noop}
        pendingApprovals={{}}
        liveActivity={{}}
        starting={[]}
        recentlyDone={[]}
        recentlyReceived={[]}
        onSelectTeammate={noop}
        onNewConversationWith={noop}
        onOpenHub={noop}
        onAddMenu={noop}
        composerShown={false}
        onOpenSettings={noop}
        onOpenMissions={noop}
        onOpenTeammates={noop}
        rooms={[]}
        routines={[]}
        currentRoomId={undefined}
        onOpenRoom={noop}
        onOpenRooms={noop}
        onOpenAutomations={noop}
        onHome={noop}
        groups={[]}
      />
    )
    const faces = html.split('class="lc-faces__one').slice(1)
    const markOf = (name: string): string | null => {
      const face = faces.find((one) => one.includes(`aria-label="${name} — open their conversation"`)) ?? ''
      return /class="lc-bot__mark" data-runtime="([a-z]+)"/.exec(face)?.[1] ?? null
    }
    expect(markOf('Wren')).toBe('claude')
    expect(markOf('Juno')).toBe('codex')
    // A model of the person's own is named as theirs, with no maker's mark;
    // a teammate who has never run has no route to show.
    expect(markOf('Acme')).toBeNull()
    expect(markOf('Newt')).toBeNull()
  })
})
