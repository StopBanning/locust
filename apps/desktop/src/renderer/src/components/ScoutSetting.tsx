import { useState } from 'react'
import type { ReactNode } from 'react'

import {
  MAX_SCOUT_GOAL, SCOUT_EVERY_MINUTES, SCOUT_QUIET_CHOICES, hourWords, inQuietHours,
  type ScoutApi, type ScoutEvery, type ScoutQuietHours, type ScoutState, type ScoutWatches
} from '../../../shared/scout.js'
import { ago } from './ScoutCards.js'
import { Select } from './Select.js'

const quietWords = (quiet: ScoutQuietHours): string => `${hourWords(quiet.from)} to ${hourWords(quiet.to)}`

/**
 * SCOUT IN SETTINGS (0.756, shared/scout.ts): on or off, what it watches, how often it looks, and whether a card that
 * needs the person now comes as a Windows notification. Its model is its teammate's, changed like any teammate's;
 * this line says which one it reads with and what that costs. Since 0.815 also what to keep watch for, in the
 * person's own words, the folder's check, quiet hours, a look that waits while their own turn runs, and what today
 * used.
 */
export function ScoutSetting({ heading, state, model, route, checkCommand, onChangeModel, api = typeof window === 'undefined' ? undefined : window.desktop?.scout }: {
  /** Settings' own heading, so its search finds this line (settings-pages-are-real). */
  readonly heading: ReactNode
  readonly state: ScoutState | undefined
  /** The model Scout reads with now, as a person reads it; undefined before it has one. */
  readonly model?: string
  /** The free model a new Scout would read with; undefined when none is listed here. */
  readonly route?: { readonly runtime: string; readonly model: string }
  /** This folder's check, from Settings > Project folder; absent when it has none. */
  readonly checkCommand?: string
  /** Opens Scout's own Edit teammate, where its Model row is: absent before Scout is on the team. */
  readonly onChangeModel?: () => void
  readonly api?: ScoutApi
}) {
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  /** The goal as it is being written; undefined while it reads as saved. */
  const [goal, setGoal] = useState<string | undefined>(undefined)
  if (api === undefined) return null
  const on = state?.settings.on === true
  const savedGoal = state?.settings.goal ?? ''
  const quiet = state?.settings.quiet
  const today = state?.today
  const status = [
    ...(state?.lastLookAt === undefined ? [] : [`Last looked ${ago(state.lastLookAt, new Date())}${state.cards.length === 0 ? ', and nothing needed you.' : '.'}`]),
    ...(today === undefined ? [] : [`Today: ${String(today.looks)} look${today.looks === 1 ? '' : 's'}, ${today.asked === 0 ? 'none of them needing its model' : `${String(today.asked)} of them read by ${model ?? 'its model'}`}.`]),
    ...(quiet !== undefined && inQuietHours(quiet, new Date()) ? [`Quiet until ${hourWords(quiet.to)}.`] : [])
  ]
  const act = async (work: () => Promise<unknown>): Promise<void> => {
    setBusy(true); setMessage('')
    try {
      const answer = await work()
      if (typeof answer === 'object' && answer !== null && 'refused' in answer) setMessage(String((answer as { refused: string }).refused))
    } catch {
      setMessage('Scout’s setting was not changed. Try again.')
    } finally { setBusy(false) }
  }
  const toggle = (): void => {
    if (on) { void act(() => api.set({ on: false })); return }
    if (state?.settings.teammateId === undefined && route === undefined) { setMessage('Scout needs a model to read with: install OpenCode for a free one, then turn it on.'); return }
    void act(() => api.turnOn(route))
  }
  const watch = (key: keyof ScoutWatches): void => {
    if (state === undefined) return
    void act(() => api.set({ watches: { ...state.settings.watches, [key]: !state.settings.watches[key] } }))
  }
  const saveGoal = (): void => {
    if (goal === undefined) return
    const words = goal
    void act(async () => { await api.set({ goal: words }); setGoal(undefined) })
  }
  const switchOf = (label: string, checked: boolean, onClick: () => void, disabled = false) =>
    <button type="button" className={`lc-switch${checked ? ' is-on' : ''}`} role="switch" aria-label={label} aria-checked={checked} disabled={state === undefined || busy || disabled} onClick={onClick}><span /></button>
  return <section className="lc-settings__section lc-settings__section--line" data-setting="scout">
    <div className="lc-settingline">
      <div className="lc-settingline__text">
        {heading}
        <p className="lc-settings__lede">{on
          ? `On. Scout looks every ${String(state?.settings.everyMinutes ?? 10)} minutes while Locust runs, and tells you on Home what needs you, with the next step drafted. It never sends, merges or changes anything by itself.${model === undefined ? '' : ` It reads with ${model}.`}`
          : 'Off. Turn it on and Scout joins your team: it looks at your pull requests and their checks, the reviews asked of you and your teammates’ turns, and tells you on Home what needs you.'}</p>
        {/*
          * Where it stands, as one line (0.815): when it last looked (0.768: a Scout with nothing to say otherwise shows
          * nothing at all), what today used -- a quiet Scout costs nothing, and this says so in numbers -- and that it
          * is in its quiet hours.
          */}
        {on && status.length > 0 && <p className="lc-settings__note" role="status">{status.join(' ')}</p>}
        {state?.modelSaid !== undefined && on && <p className="lc-settings__note" role="status">{`Last look: ${state.modelSaid}`}</p>}
        {message && <p className="lc-settings__note" role="status">{message}</p>}
      </div>
      <span className="lc-talkkeys">
        {/* Its model is its teammate's (Colin, 2026-10-12: "allow the user to change the model of proactive teammate"). */}
        {on && <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void act(() => api.look()) }}>Look now</button>}
        {on && onChangeModel !== undefined && <button type="button" className="lc-control lc-control--boxed" onClick={onChangeModel}>Change its model</button>}
        {switchOf('Scout keeps watch', on, toggle)}
      </span>
    </div>
    {on && state !== undefined && <>
      {/* What to keep watch for (0.815): the person's own words, read into every look. */}
      <div className="lc-settingline lc-settingline--sub lc-scout__goalline">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">What to keep watch for</h3>
          <p className="lc-settings__lede">In your own words. Scout reads it on every look, and says how what it finds bears on it.</p>
          <textarea className="lc-input lc-scout__goal" aria-label="What Scout keeps watch for" maxLength={MAX_SCOUT_GOAL} disabled={busy}
            placeholder="For example: keep main’s tests passing, and tell me about new issues on the sign-in page."
            value={goal ?? savedGoal}
            onChange={(event) => setGoal(event.target.value)}
            onKeyDown={(event) => { if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); saveGoal() } }} />
          {goal !== undefined && goal.trim() !== savedGoal && <span className="lc-scout__goalkeys">
            <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={saveGoal}>Save</button>
            <button type="button" className="lc-control" disabled={busy} onClick={() => setGoal(undefined)}>Cancel</button>
          </span>}
        </div>
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Pull requests and their checks</h3>
          <p className="lc-settings__lede">Yours, the reviews asked of you, and the default branch’s checks, through the GitHub CLI you signed in.</p>
        </div>
        {switchOf('Pull requests and their checks', state.settings.watches.github, () => { watch('github') })}
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Your teammates’ turns</h3>
          <p className="lc-settings__lede">One waiting on you for more than 10 minutes, and one that failed.</p>
        </div>
        {switchOf('Your teammates’ turns', state.settings.watches.teammates, () => { watch('teammates') })}
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">This folder’s check</h3>
          <p className="lc-settings__lede">{checkCommand === undefined
            ? 'Set a check in Settings > Project folder first, such as npm test.'
            : <>When the folder has changed, Scout runs <code className="lc-mono">{checkCommand}</code>, never while a turn is working here. A check that starts failing needs you now.</>}</p>
        </div>
        {switchOf('This folder’s check', state.settings.watches.check, () => { watch('check') }, checkCommand === undefined && !state.settings.watches.check)}
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">How often it looks</h3>
          <p className="lc-settings__lede">Only while Locust is running. Its model is asked only when something is new.</p>
        </div>
        <Select className="lc-control" aria-label="How often Scout looks" disabled={busy} value={state.settings.everyMinutes}
          onChange={(next) => { void act(() => api.set({ everyMinutes: Number(next) as ScoutEvery })) }}
          options={SCOUT_EVERY_MINUTES.map((minutes) => ({ value: String(minutes), label: minutes === 60 ? 'Every hour' : `Every ${String(minutes)} minutes` }))} />
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Quiet hours</h3>
          <p className="lc-settings__lede">No looks on its own and nothing in Windows. The first look after them catches up on everything.</p>
        </div>
        <Select className="lc-control" aria-label="Scout’s quiet hours" disabled={busy} value={quiet === undefined ? 'off' : `${String(quiet.from)}-${String(quiet.to)}`}
          onChange={(next) => {
            const picked = SCOUT_QUIET_CHOICES.find((choice) => `${String(choice.from)}-${String(choice.to)}` === next)
            void act(() => api.set({ quiet: picked ?? null }))
          }}
          options={[
            { value: 'off', label: 'Off' },
            ...SCOUT_QUIET_CHOICES.map((choice) => ({ value: `${String(choice.from)}-${String(choice.to)}`, label: quietWords(choice) })),
            ...(quiet === undefined || SCOUT_QUIET_CHOICES.some((choice) => choice.from === quiet.from && choice.to === quiet.to) ? [] : [{ value: `${String(quiet.from)}-${String(quiet.to)}`, label: quietWords(quiet) }])
          ]} />
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Wait while you’re working</h3>
          <p className="lc-settings__lede">A look due while your own turn runs waits for it to end, at most 30 minutes.</p>
        </div>
        {switchOf('Wait while you’re working', state.settings.waitWhileWorking, () => { void act(() => api.set({ waitWhileWorking: !state.settings.waitWhileWorking })) })}
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Tell me in Windows</h3>
          <p className="lc-settings__lede">Only for a card that needs you now, while Locust is minimized, in the tray or behind another app.</p>
        </div>
        {switchOf('Tell me in Windows', state.settings.notify, () => { void act(() => api.set({ notify: !state.settings.notify })) })}
      </div>
    </>}
  </section>
}
