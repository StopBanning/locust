import { useState } from 'react'
import type { ReactElement } from 'react'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type { MissionRouteSummary, PublicRecoveredMission } from '../../../shared/ipc.js'
import { costLabel, costLineOrWhyNot, runCostOf } from '../cost.js'
import { buildSignalRail, buildThread, producedFiles } from '../missionView.js'
import { accountPhrase, checkpointLabel, ledgerVerificationLabel, sandboxPhrase, shortMissionId } from '../status.js'
import { modelDisplayName } from '../routeName.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { whatItMayDo } from '../../../shared/may-do.js'
import { Icon } from './Icon.js'
import { WORKSPACE_REVEAL_CHANNEL } from '../../../shared/ipc.js'
import { failureSaid, onTheComputer } from '../onTheComputer.js'

const TABS = ['Activity', 'Details', 'Artifacts', 'Receipt'] as const
type Tab = (typeof TABS)[number]

function Empty({ children }: { readonly children: string }): ReactElement {
  return <p className="lc-inspector__empty">{children}</p>
}

/**
 * Why the Artifacts tab is empty, said for THIS run (0.362).
 *
 * It said "A read-only mission produces none; artifacts appear once a run
 * can write" under every empty tab -- including an editor's run in Edit that
 * simply wrote nothing because its teammate had written the draft (the Write
 * & design drive, packaged 0.361). A run that could write and did not is not
 * a read-only run.
 */
export function noArtifactsLine(sandbox: 'read-only' | 'workspace-write' | 'full-access' | undefined, running: boolean, watched = false): string {
  if (running) return 'Files this reply changes appear here as it works.'
  if (sandbox === 'read-only') return 'This reply could only read, so it changed no files.'
  // Said flat only when the host looked at the folder itself: otherwise a
  // file a command wrote -- Penny's workbook -- is simply not known (0.364).
  if (watched) return 'This reply changed no files.'
  return 'No changed files were reported. A file a command wrote may not be listed here.'
}

/**
 * The mission inspector. The Signal Rail is the honest home for detail: the
 * thread stays semantic and everything that actually happened lives here.
 *
 * The Tools & permissions block the reference draws is deliberately reduced to
 * what this build can prove: the rows Locust handed this run's runtime in this
 * run's mode (shared/may-do.ts), and a line that leaves the rest to the
 * runtime's own settings rather than guessing at them.
 */
export function Inspector({
  events,
  workspacePath,
  running,
  route,
  restoredMission,
  onClose
}: {
  readonly events: readonly NormalizedRuntimeEvent[]
  /** The folder this ran in, so artifact paths read the way a person writes them. */
  readonly workspacePath: string | undefined
  readonly running: boolean
  readonly route: MissionRouteSummary | undefined
  readonly restoredMission: PublicRecoveredMission | undefined
  readonly onClose: () => void
}): ReactElement {
  const [tab, setTab] = useState<Tab>('Activity')
  // What this run was ACTUALLY allowed to do, read from the start receipt --
  // not from whatever mode the composer happens to show now.
  /*
   * THREE states, not two, and the third was being described as its opposite.
   *
   * `writes` collapsed workspace-write and full-access together, so an Auto
   * run -- the one that may touch the whole machine -- was labelled
   * `workspace-write` and then told the reader "deny: anything outside the
   * workspace", which is exactly what Auto is for. The conversation header
   * said "may edit anything on this machine" two inches away. Of the two
   * surfaces the person is likelier to trust the specific-looking one, and it
   * was the wrong one (Grok's audit, 2026-09-13).
   *
   * The label now comes from `sandboxPhrase`, the same function the header
   * uses, so the two cannot drift again.
   */
  const sandbox = route?.sandbox
  const writes = sandbox === 'workspace-write' || sandbox === 'full-access'
  // Runtime by runtime, from what Locust handed THAT runtime (shared/may-do.ts):
  // the same three rows for every runtime told a person that an OpenCode run
  // which had just searched the web could not reach the network (0.359).
  const mode = route?.mode ?? restoredMission?.mode
  const mayDo = route === undefined ? undefined : whatItMayDo(route.runtime, route.sandbox, mode)
  const rows = buildSignalRail(events, { running })
  const [artifactNotice, setArtifactNotice] = useState<string | undefined>(undefined)
  // Every file this run touched, gathered from the same activity the fold
  // draws so the tab and the thread cannot disagree about what happened.
  const artifacts = producedFiles(
    buildThread(events, { running, mayEdit: writes, ...(workspacePath === undefined ? {} : { workspacePath }) })
      .flatMap((item) => ('details' in item && Array.isArray(item.details) ? item.details : [])),
    workspacePath
  )

  return (
    <aside className="lc-inspector" aria-label="About this reply">
      <div className="lc-inspector__head">
        {/*
          * "Mission inspector" was the panel's name in the code, in a
          * developer's word, over a panel a person opens with a button that
          * says Activity (0.361, after "Signal rail" became "What happened"
          * in 0.354). It is about the reply above it: what happened, what it
          * was allowed, the details, the files, the receipt.
          */}
        <span className="lc-inspector__title">About this reply</span>
        <button type="button" className="lc-inspector__close" aria-label="Close About this reply" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>

      <div className="lc-tabs" role="tablist" aria-label="About this reply">
        {TABS.map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={tab === name}
            className={`lc-tab${tab === name ? ' is-active' : ''}`}
            onClick={() => setTab(name)}
          >
            {name}
          </button>
        ))}
      </div>

      <div className="lc-inspector__scroll">
        {tab === 'Activity' && (
          <>
            {/* "Signal rail" was the panel's own jargon (first-impressions pass, 0.354). */}
            <div className="lc-fieldlabel lc-mono lc-rail__label">What happened</div>
            {rows.length === 0 ? (
              <Empty>No events yet.</Empty>
            ) : (
              <div className="lc-rail">
                {rows.map((row) => (
                  <div className="lc-rail__event" key={row.key}>
                    <span className="lc-rail__gutter">
                      <span className={`lc-rail__dot lc-tone-${row.tone}${row.live ? ' is-pulsing' : ''}`} />
                      <span className="lc-rail__line" />
                    </span>
                    <span className="lc-rail__body">
                      <span className="lc-rail__name">{row.name}</span>
                      <span className="lc-rail__meta">{row.meta}</span>
                    </span>
                  </div>
                ))}
              </div>
            )}

            {mayDo !== undefined && (
              <div className="lc-permissions">
                <div className="lc-permissions__head">
                  <span className="lc-fieldlabel">What it may do</span>
                  <span className="lc-rail__meta">{sandboxPhrase(sandbox)}</span>
                </div>
                <div className="lc-permissions__rows">
                  {mayDo.rows.map((row) => (
                    <div className="lc-permissions__row" key={row.text}>
                      <span className={row.verdict === 'deny' ? 'lc-tone-red' : row.wide === true ? 'lc-tone-amber' : 'lc-tone-green'}>{row.verdict}</span>
                      <span>{row.text}</span>
                    </div>
                  ))}
                </div>
                {/*
                  * In Locust's words, not the plumbing's: "The host fixes the
                  * workspace, executable, argv and sandbox" (first-impressions
                  * pass, 0.354). And no longer a sentence about `codex exec`
                  * having no approval channel: Codex runs on its app-server now,
                  * where Approve each asks per action, so that sentence had
                  * become false about the run it sat under.
                  */}
                <p className="lc-permissions__note">
                  Locust sets the folder, the tool and these limits when the mission starts; nothing here changes while it runs. {mayDo.rest}
                </p>
              </div>
            )}
          </>
        )}

        {/*
          * IN WORDS, WITH THE EXACT STRINGS ONE HOVER AWAY.
          *
          * These were the record's own spellings -- "codex 0.153.0",
          * "account-default", "codex-account:default" -- beside a composer
          * that says "Codex / Account Default" for the same run (the design
          * review, "Details in plain words"). The names are the chip's; the
          * identifiers stay in each value's title, which is where a person
          * goes to find the string to type somewhere else.
          */}
        {tab === 'Details' && (
          <dl className="lc-receipt lc-receipt--flush">
            <dt>Runtime</dt>
            <dd title={route === undefined ? undefined : `${route.runtime} ${route.cliVersion ?? ''}`.trim()}>
              {route === undefined ? 'unknown' : `${runtimeDisplayName(route.runtime)} ${route.cliVersion ?? ''}`.trim()}
            </dd>
            <dt>Model</dt>
            <dd title={route?.model}>{route === undefined ? 'unknown' : modelDisplayName(route.runtime, route.model)}</dd>
            <dt>Account</dt>
            <dd title={route?.resolvedRouteId}>{route === undefined ? 'unknown' : accountPhrase(route)}</dd>
            <dt>Run</dt>
            <dd className="lc-mono" title={route?.missionId}>{route === undefined ? 'unknown' : shortMissionId(route.missionId)}</dd>
            <dt>Sandbox</dt>
            <dd>{route?.sandbox === undefined ? 'unknown' : sandboxPhrase(route.sandbox)}</dd>
            <dt>Events</dt>
            <dd>{events.length} recorded in this view</dd>
            {/* The Team card's rule: Cost for a priced run, Usage for tokens --
                this said Cost for token counts (review of 0.255.0, #10). */}
            <dt>{costLabel(runCostOf(events))}</dt>
            <dd className="lc-mono">
              {costLineOrWhyNot(runCostOf(events), running)}
            </dd>
          </dl>
        )}

        {tab === 'Artifacts' &&
          (artifacts.length === 0 ? (
            <Empty>{noArtifactsLine(sandbox, running, route?.watchesDisk === true)}</Empty>
          ) : (
            // The tab has made this promise since it was built and never kept
            // it: it said artifacts appear once a run can write, and then
            // showed the same empty line after runs that wrote plenty. The
            // rows come from the same entries the activity fold counts, so the
            // two cannot disagree about what this mission touched.
            <ul className="lc-artifacts">
              {artifacts.map((artifact) => (
                <li className="lc-artifacts__row" key={artifact.path}>
                  <span className="lc-artifacts__path" title={artifact.path}>
                    {artifact.shown}
                  </span>
                  {artifact.status !== undefined && <span className="lc-artifacts__status">{artifact.status}</span>}
                  {/* The computer's file manager (0.850): not drawn in a browser, which has none of it. */}
                  {onTheComputer(WORKSPACE_REVEAL_CHANNEL) === undefined && <button
                    type="button"
                    className="lc-filerow__reveal"
                    title={`Show ${artifact.shown} in the file manager`}
                    aria-label={`Show ${artifact.shown} in the file manager`}
                    onClick={() => {
                      const bridge = window.desktop
                      if (bridge === undefined) return
                      setArtifactNotice(undefined)
                      void bridge
                        .revealFile(artifact.path)
                        .then((response) => {
                          if (!response.ok) setArtifactNotice(response.message)
                        })
                        .catch((error: unknown) => setArtifactNotice(failureSaid(error, 'That file could not be shown. It is still where it was written.')))
                    }}
                  >
                    <Icon name="folder" size={13} />
                  </button>}
                </li>
              ))}
              {artifactNotice !== undefined && <li className="lc-filerow__notice">{artifactNotice}</li>}
            </ul>
          ))}

        {tab === 'Receipt' &&
          (restoredMission === undefined ? (
            /*
             * Only two honest things to say without a record: it is not
             * written yet because the run is still going, or it is being
             * read. The old line -- "appears once this mission has been
             * recovered from the ledger" -- described a mechanism, sent the
             * person nowhere, and stood under runs whose record was already
             * on disk.
             */
            <Empty>
              {running
                ? 'The receipt is written when this run finishes: its phase, checkpoints, event count and whether the ledger verifies.'
                : 'Reading this run’s receipt from the ledger…'}
            </Empty>
          ) : (
            <dl className="lc-receipt lc-receipt--flush">
              <dt>Phase</dt>
              <dd>{restoredMission.phase}</dd>
              <dt>Checkpoints</dt>
              <dd>
                {restoredMission.checkpoints.length === 0
                  ? 'none written'
                  : `${restoredMission.checkpoints.length} · last ${checkpointLabel(
                      restoredMission.checkpoints[restoredMission.checkpoints.length - 1]!.epoch
                    )}`}
              </dd>
              <dt>Events</dt>
              <dd>{restoredMission.eventCount}</dd>
              <dt>Ledger</dt>
              <dd
                className={
                  ledgerVerificationLabel(restoredMission.integrityIssueCount) === 'record readable'
                    ? 'lc-tone-green'
                    : 'lc-tone-amber'
                }
              >
                {ledgerVerificationLabel(restoredMission.integrityIssueCount)}
              </dd>
            </dl>
          ))}
      </div>
    </aside>
  )
}
