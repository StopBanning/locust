import { useEffect, useRef } from 'react'
import type { ReactElement } from 'react'
import { windowWords } from '../machineWords.js'

import {
  CLOUD_AGENT_ACCOUNTS,
  CLOUD_AGENT_MAKERS,
  CLOUD_AGENT_NAMES,
  CLOUD_MACHINE_WORDS,
  type CloudAgent,
  type CloudSetupStep,
  type CloudSignInStep
} from '../../../shared/cloud-computer.js'
import { Icon } from './Icon.js'
import { useModal } from '../useModal.js'

/**
 * YOUR CLOUD COMPUTER, THE FIRST TIME (0.761). Asked when Cloud is first picked for a teammate whose maker has no
 * cloud of its own, in a repository that has no cloud computer yet: what it is, what it costs, and -- once the
 * person says yes -- each step of making it as it happens. MEASURED 2026-10-12: made in 3.5 s, configured about 30 s
 * later, OpenCode installed in 21 s.
 *
 * CLAUDE CODE AND CODEX THERE (0.768): for their teammates the agent goes on instead, and is signed in there as the
 * last step -- its maker's own page opened in the person's browser, one Authorize, no code. On a computer that is
 * already set up, the same dialog asks only for that sign-in (`sign-in`), then shows it happen (`signing-in`).
 */
export type CloudSetupState =
  | { readonly phase: 'ask'; readonly repo: string; readonly agent?: CloudAgent }
  | { readonly phase: 'working'; readonly repo: string; readonly step: CloudSetupStep; readonly agent?: CloudAgent; readonly waiting?: true }
  | { readonly phase: 'ready'; readonly repo: string; readonly agent?: CloudAgent }
  | { readonly phase: 'sign-in'; readonly repo: string; readonly computer: string; readonly agent: CloudAgent }
  | { readonly phase: 'signing-in'; readonly repo: string; readonly computer: string; readonly agent: CloudAgent; readonly step: CloudSignInStep; readonly woke?: true }
  | { readonly phase: 'signed-in'; readonly repo: string; readonly agent: CloudAgent; readonly woke?: true }
  | { readonly phase: 'permission'; readonly message: string; readonly allowed?: boolean }
  | { readonly phase: 'not-on-github'; readonly message: string }
  | { readonly phase: 'failed'; readonly message: string; readonly repo?: string; readonly computer?: string; readonly agent?: CloudAgent }

interface Step<Id extends string> {
  readonly step: Id
  readonly now: string
  readonly done: string
  readonly takes: string
}

function setupSteps(agent: CloudAgent | undefined, waiting: boolean): readonly Step<Exclude<CloudSetupStep, 'ready'>>[] {
  const name = agent === undefined ? 'OpenCode' : CLOUD_AGENT_NAMES[agent]
  return [
    { step: 'making', now: 'Making the computer', done: 'Computer made', takes: 'a few seconds' },
    { step: 'readying', now: 'Letting it set itself up', done: 'Set up', takes: 'about 30 s' },
    { step: 'installing', now: `Installing ${name}`, done: `${name} installed`, takes: agent === undefined ? 'about 20 s' : 'about 30 s' },
    ...(agent === undefined
      ? []
      : [{
          step: 'signing-in' as const,
          now: waiting ? `Click Authorize on ${CLOUD_AGENT_MAKERS[agent]}'s page in your browser` : `Opening ${CLOUD_AGENT_MAKERS[agent]}'s sign-in in your browser`,
          done: `Signed in to ${name}`,
          takes: waiting ? 'waiting for you' : 'a few seconds'
        }])
  ]
}

function signInSteps(agent: CloudAgent, woke: boolean): readonly Step<CloudSignInStep>[] {
  const name = CLOUD_AGENT_NAMES[agent]
  const maker = CLOUD_AGENT_MAKERS[agent]
  return [
    ...(woke ? [{ step: 'waking' as const, now: 'Waking your cloud computer', done: 'Awake', takes: 'about a minute and a half' }] : []),
    { step: 'installing', now: `Getting ${name} ready there`, done: `${name} ready there`, takes: 'a few seconds' },
    { step: 'opening', now: `Opening ${maker}'s sign-in in your browser`, done: `${maker}'s page opened`, takes: 'a few seconds' },
    { step: 'waiting', now: `Click Authorize on ${maker}'s page`, done: 'Signed in', takes: 'waiting for you' }
  ]
}

function Steps<Id extends string>({ steps, at, working, label }: { readonly steps: readonly Step<Id>[]; readonly at: number; readonly working: boolean; readonly label: 'Setting up' | 'Signing in' }): ReactElement {
  return (
    <ol className="lc-cloudsetup__steps" aria-label={label === 'Signing in' ? 'Signing in' : 'Setting up'}>
      {steps.map((entry, index) => {
        const done = index < at
        const now = index === at && working
        return (
          <li key={entry.step} className={`lc-cloudsetup__step${done ? ' is-done' : now ? ' is-now' : ''}`} {...(now ? { 'aria-current': 'step' as const } : {})}>
            <span className="lc-cloudsetup__mark" aria-hidden="true">{done ? <Icon name="check" size={12} /> : <span className="lc-cloudsetup__dot" />}</span>
            <span className="lc-cloudsetup__what">{done ? entry.done : entry.now}</span>
            <span className="lc-cloudsetup__takes lc-mono">{done ? 'done' : now ? entry.takes : ''}</span>
          </li>
        )
      })}
    </ol>
  )
}

export function CloudSetupDialog({
  state,
  teammateName,
  onSetUp,
  onSignIn,
  onStopSignIn,
  onAllow,
  onCheckAgain,
  onClose
}: {
  readonly state: CloudSetupState
  readonly teammateName: string | undefined
  readonly onSetUp: () => void
  /** Sign the agent in on that computer: its page opens in the person's browser. */
  readonly onSignIn: () => void
  readonly onStopSignIn: () => void
  readonly onAllow: () => void
  readonly onCheckAgain: () => void
  readonly onClose: () => void
}): ReactElement {
  // Focus goes into the dialog, not onto a button: a ring drawn round Set it up read as already chosen.
  const box = useRef<HTMLElement>(null)
  useEffect(() => {
    box.current?.focus()
  }, [state.phase])
  const who = teammateName ?? 'Your teammate'
  const repoName = 'repo' in state && state.repo !== undefined ? (state.repo.split('/').pop() ?? state.repo) : undefined
  const agent = 'agent' in state ? state.agent : undefined
  const busy = state.phase === 'working' || state.phase === 'signing-in'
  // While its page is open, the rest is the person's: Stop is theirs too.
  const signingIn = state.phase === 'signing-in' || (state.phase === 'working' && state.step === 'signing-in')
  const steps = setupSteps(agent, state.phase === 'working' && state.waiting === true)
  const at = state.phase === 'working' ? steps.findIndex((entry) => entry.step === state.step) : state.phase === 'ready' ? steps.length : -1
  const signSteps = state.phase === 'signing-in' || state.phase === 'signed-in' ? signInSteps(state.agent, state.woke === true) : []
  const signAt = state.phase === 'signing-in' ? signSteps.findIndex((entry) => entry.step === state.step) : signSteps.length
  // Escape is the x, and does what the x does in each phase (design pass, 2026-10-19): it had none, and Tab walked out
  // to the page behind the scrim.
  useModal(box, () => {
    if (signingIn) onStopSignIn()
    else if (!busy) onClose()
  })

  return (
    <div className="lc-scrim">
      <section ref={box} tabIndex={-1} className="lc-dialog lc-cloudsetup" role="dialog" aria-modal="true" aria-label="Your cloud computer">
        <div className="lc-dialog__head">
          <Icon name="cloud" size={15} />
          <span className="lc-dialog__title">Your cloud computer</span>
          {repoName !== undefined && <span className="lc-dialog__sub">{repoName}</span>}
          <button type="button" className="lc-dialog__close" aria-label="Close" title="Close" disabled={busy && !signingIn} onClick={signingIn ? onStopSignIn : onClose}>
            <Icon name="close" size={13} />
          </button>
        </div>
        <div className="lc-dialog__body">

        {(state.phase === 'ask' || state.phase === 'working' || state.phase === 'ready') && (
          <>
            <p className="lc-cloudsetup__lead">
              {`${who} will work on a computer GitHub runs for you, with ${repoName ?? 'this repository'} on it, instead of on this ${windowWords().computer}. It keeps going when this ${windowWords().computer} sleeps, and nothing changes here until you apply it.`}
            </p>
            <div className="lc-cloudsetup__facts">
              <div className="lc-cloudsetup__fact"><span className="lc-cloudsetup__label">Size</span><span>{CLOUD_MACHINE_WORDS.replace(', ', ' · ')} memory</span></div>
              <div className="lc-cloudsetup__fact"><span className="lc-cloudsetup__label">Free each month</span><span>60 hours, from GitHub</span></div>
              <div className="lc-cloudsetup__fact"><span className="lc-cloudsetup__label">After that</span><span>$0.18 an hour</span></div>
            </div>
            <p className="lc-cloudsetup__note">
              It stops itself after 10 minutes with nothing to do. GitHub keeps the count and the bill: with no payment method on GitHub, it stops at the free hours instead of charging you.
            </p>
            {agent !== undefined && (
              <p className="lc-cloudsetup__note">
                {`${CLOUD_AGENT_NAMES[agent]} goes on it, signed in with your ${CLOUD_AGENT_ACCOUNTS[agent]} account: Locust opens ${CLOUD_AGENT_MAKERS[agent]}'s own page in your browser, and you click Authorize. There is no code to copy.`}
              </p>
            )}
          </>
        )}

        {(state.phase === 'working' || state.phase === 'ready') && <Steps steps={steps} at={at} working={state.phase === 'working'} label="Setting up" />}

        {(state.phase === 'sign-in' || state.phase === 'signing-in') && (
          <>
            <p className="lc-cloudsetup__lead">
              {`${CLOUD_AGENT_NAMES[state.agent]} is not signed in on your cloud computer yet. Sign in once: Locust opens ${CLOUD_AGENT_MAKERS[state.agent]}'s own sign-in page in your browser, and you click Authorize, as you would on this ${windowWords().computer}. There is no code to copy.`}
            </p>
            <p className="lc-cloudsetup__note">
              The sign-in stays on that computer, and goes when the computer is deleted. Locust never sees it.
            </p>
          </>
        )}
        {state.phase === 'signed-in' && (
          <p className="lc-cloudsetup__lead">
            {`${CLOUD_AGENT_NAMES[state.agent]} is signed in on your cloud computer. It stays signed in there, and goes when the computer is deleted.`}
          </p>
        )}
        {(state.phase === 'signing-in' || state.phase === 'signed-in') && <Steps steps={signSteps} at={signAt} working={state.phase === 'signing-in'} label="Signing in" />}

        {state.phase === 'permission' && (
          <>
            <p className="lc-cloudsetup__lead">{state.message}</p>
            <p className="lc-cloudsetup__note">
              Allow it opens a terminal with a one-time code and GitHub's page in your browser. Enter the code there, then come back and check again. Locust never sees your GitHub sign-in.
            </p>
          </>
        )}
        {(state.phase === 'not-on-github' || state.phase === 'failed') && <p className="lc-cloudsetup__lead" role="alert">{state.message}</p>}
        </div>

        <div className="lc-dialog__foot">
          {state.phase === 'ask' && (
            <>
              <span className="lc-cloudsetup__note">{agent === undefined ? 'About a minute, only the first time.' : 'About two minutes, only the first time.'}</span>
              <button type="button" className="lc-primarybutton" onClick={onSetUp}>Set it up</button>
              <button type="button" className="lc-button" onClick={onClose}>Not now</button>
            </>
          )}
          {state.phase === 'working' && (signingIn
            ? (
              <>
                <span className="lc-cloudsetup__note">{state.waiting === true ? 'Finish in your browser, then come back here.' : 'Opening your browser.'}</span>
                <button type="button" className="lc-button" onClick={onStopSignIn}>Stop</button>
              </>
            )
            : <span className="lc-cloudsetup__note">Setting up. About a minute, only the first time.</span>)}
          {state.phase === 'ready' && (
            <>
              <span className="lc-cloudsetup__note">{agent === undefined ? `Ready. Send ${who} a task and it runs there.` : `Ready. Send ${who} a task and ${CLOUD_AGENT_NAMES[agent]} runs it there.`}</span>
              <button type="button" className="lc-primarybutton" onClick={onClose}>Done</button>
            </>
          )}
          {state.phase === 'sign-in' && (
            <>
              <span className="lc-cloudsetup__note">Once, for this computer.</span>
              <button type="button" className="lc-primarybutton" onClick={onSignIn}>{`Sign in with ${CLOUD_AGENT_ACCOUNTS[state.agent]}`}</button>
              <button type="button" className="lc-button" onClick={onClose}>Not now</button>
            </>
          )}
          {state.phase === 'signing-in' && (
            <>
              <span className="lc-cloudsetup__note">{state.step === 'waiting' ? 'Finish in your browser, then come back here.' : 'Getting the sign-in ready.'}</span>
              <button type="button" className="lc-button" onClick={onStopSignIn}>Stop</button>
            </>
          )}
          {state.phase === 'signed-in' && (
            <>
              <span className="lc-cloudsetup__note">{`Ready. Send ${who} a task and ${CLOUD_AGENT_NAMES[state.agent]} runs it there.`}</span>
              <button type="button" className="lc-primarybutton" onClick={onClose}>Done</button>
            </>
          )}
          {state.phase === 'permission' && (
            <>
              <button type="button" className="lc-primarybutton" onClick={onAllow}>Allow it in GitHub</button>
              <button type="button" className="lc-button" onClick={onCheckAgain}>Check again</button>
              <button type="button" className="lc-button" onClick={onClose}>Not now</button>
            </>
          )}
          {state.phase === 'not-on-github' && <button type="button" className="lc-button" onClick={onClose}>OK</button>}
          {state.phase === 'failed' && (
            <>
              {state.computer !== undefined && state.agent !== undefined
                ? <button type="button" className="lc-primarybutton" onClick={onSignIn}>Try again</button>
                : state.repo !== undefined && <button type="button" className="lc-primarybutton" onClick={onSetUp}>Try again</button>}
              <button type="button" className="lc-button" onClick={onClose}>Close</button>
            </>
          )}
        </div>
      </section>
    </div>
  )
}
