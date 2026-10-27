import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

import {
  DEFAULT_SCOUT_FIXES, MAX_SCOUT_GOAL, MAX_SCOUT_NOTE, MAX_SCOUT_NOTES, SCOUT_BRIEF_HOURS, SCOUT_EVERY_MINUTES, SCOUT_FIXES_PER_DAY, SCOUT_QUIET_CHOICES, hourWords, inQuietHours,
  type ScoutApi, type ScoutEvery, type ScoutQuietHours, type ScoutState, type ScoutWatches
} from '../../../shared/scout.js'
import type { ScoutAwayStatus } from '../../../shared/scout-away.js'
import { lookLines, lookName, setSomethingAside, type ScoutHeldAlert, type ScoutLogEntry } from '../../../shared/scout-log.js'
import { ago } from './ScoutCards.js'
import { Select } from './Select.js'

const quietWords = (quiet: ScoutQuietHours): string => `${hourWords(quiet.from)} to ${hourWords(quiet.to)}`

/*
 * PREPARE FIXES BEFORE I LOOK (0.828, main/scout-fix.ts): off until the person turns it on, since a fix runs on the
 * picked teammate's plan; who prepares them; and how many a day.
 */
function FixesLine({ state, team, busy, checkCommand, act, api, switchOf }: {
  readonly state: ScoutState
  readonly team: readonly { readonly teammateId: string; readonly name: string }[]
  readonly busy: boolean
  readonly checkCommand: string | undefined
  readonly act: (work: () => Promise<unknown>) => Promise<void>
  readonly api: ScoutApi
  readonly switchOf: (label: string, checked: boolean, onClick: () => void, disabled?: boolean) => ReactNode
}) {
  const fixes = state.settings.fixes ?? DEFAULT_SCOUT_FIXES
  const picked = team.find((mate) => mate.teammateId === fixes.teammateId)
  const set = (change: Partial<typeof fixes>): void => { void act(() => api.set({ fixes: { ...fixes, ...change } })) }
  const canFix = state.settings.watches.check && checkCommand !== undefined
  return <div className="lc-settingline lc-settingline--sub lc-scout__goalline">
    <div className="lc-settingline__text">
      <h3 className="lc-settings__heading">Prepare fixes before I look</h3>
      <p className="lc-settings__lede">When the folder’s check starts failing, a teammate fixes it in a copy of the folder and Locust runs the check there again. Nothing reaches your folder until you press Apply, and it never pushes or opens a pull request. It runs on that teammate’s plan.</p>
      {!canFix && <p className="lc-settings__note">Turn on This folder’s check first.</p>}
      {fixes.on && <span className="lc-scout__goalkeys">
        <Select className="lc-control" aria-label="Who prepares Scout’s fixes" disabled={busy || team.length === 0} value={picked?.teammateId ?? ''}
          onChange={(next) => { set({ teammateId: next }) }}
          options={[...(picked === undefined ? [{ value: '', label: team.length === 0 ? 'Add a teammate first' : 'Pick a teammate' }] : []), ...team.map((mate) => ({ value: mate.teammateId, label: mate.name }))]} />
        <Select className="lc-control" aria-label="Fixes a day" disabled={busy} value={String(fixes.perDay)}
          onChange={(next) => { set({ perDay: Number(next) }) }}
          options={SCOUT_FIXES_PER_DAY.map((count) => ({ value: String(count), label: `At most ${String(count)} a day` }))} />
      </span>}
    </div>
    {switchOf('Prepare fixes before I look', fixes.on, () => { set(fixes.on ? { on: false } : { on: true, ...(picked === undefined && team[0] !== undefined ? { teammateId: team[0].teammateId } : {}) }) }, !fixes.on && !canFix)}
  </div>
}

/*
 * SCOUT WHILE YOUR COMPUTER IS OFF (0.829, main/scout-away.ts): a look from the repository's own workflow, added by a
 * pull request the person merges. Waiting on that merge, it asks GitHub every so often whether it happened.
 */
const AWAY_CHECK_MS = 20_000
function AwayLine({ api, switchOf, lastLookWords }: {
  readonly api: NonNullable<ScoutApi['away']>
  readonly switchOf: (label: string, checked: boolean, onClick: () => void, disabled?: boolean) => ReactNode
  readonly lastLookWords: (iso: string) => string
}) {
  const [status, setStatus] = useState<ScoutAwayStatus | undefined>(undefined)
  useEffect(() => {
    let alive = true
    api.status(true).then((next) => { if (alive) setStatus(next) }, () => undefined)
    const off = api.onChanged((next) => { if (alive) setStatus(next) })
    return () => { alive = false; off() }
  }, [api])
  // A pull request to merge: the first look's, or a newer look's (0.838).
  const waiting = status?.state === 'waiting' || (status?.state === 'on' && status.update !== undefined)
  useEffect(() => {
    if (!waiting) return
    const timer = setInterval(() => { void api.status(true).then(setStatus, () => undefined) }, AWAY_CHECK_MS)
    return () => clearInterval(timer)
  }, [api, waiting])
  const on = status?.state === 'on' || status?.state === 'waiting'
  const set = (next: boolean | 'update'): void => { void api.set(next).then(setStatus, () => undefined) }
  const open = (url: string): void => { void window.desktop?.openLink(url).catch(() => undefined) }
  const lede = status?.state === 'on'
    ? `On for ${status.repo}. A workflow there looks ${status.every === 1 ? 'every hour' : 'every two hours'} while your computer is off and leaves what it found for your phone; with Locust open it does nothing more. ${status.private ? 'It uses about 360 of the 2,000 free Actions minutes your private repository gets each month.' : 'Actions are free on a public repository.'}`
    : 'When your computer is off, a workflow in this folder’s GitHub repository looks for Scout and your phone shows what it found. Locust adds it with a pull request you merge; it reads with the workflow’s own read-only token, and what it finds is sealed for your devices.'
  return <div className="lc-settingline lc-settingline--sub lc-scout__goalline">
    <div className="lc-settingline__text">
      <h3 className="lc-settings__heading">Look while my computer is off</h3>
      <p className="lc-settings__lede">{lede}</p>
      {status?.state === 'busy' && <p className="lc-settings__note" role="status">{status.doing}</p>}
      {status?.state === 'waiting' && <>
        <p className="lc-settings__note" role="status">{`Merge the pull request in ${status.repo} to start. Locust notices once it is merged.`}</p>
        <span className="lc-scout__goalkeys"><button type="button" className="lc-control lc-control--boxed" onClick={() => open(status.pr)}>Open the pull request</button></span>
      </>}
      {status?.state === 'on' && <p className="lc-settings__note" role="status">{status.lastLook === undefined ? 'No away look yet: the first comes once your computer is off.' : `Last away look: ${lastLookWords(status.lastLook)}.`}</p>}
      {status?.state === 'on' && status.update !== undefined && <>
        <p className="lc-settings__note" role="status">{`Merge the update in ${status.repo}. The away look goes on as it was until then.`}</p>
        <span className="lc-scout__goalkeys"><button type="button" className="lc-control lc-control--boxed" onClick={() => open(status.update!)}>Open the pull request</button></span>
      </>}
      {status?.state === 'on' && status.update === undefined && status.outdated === true && <>
        <p className="lc-settings__note">A newer away look is ready: your own /locust comments reach your phone as you write them. It comes in by a pull request you merge.</p>
        <span className="lc-scout__goalkeys"><button type="button" className="lc-control lc-control--boxed" onClick={() => set('update')}>Update it</button></span>
      </>}
      {status?.state === 'off' && status.blocked !== undefined && <p className="lc-settings__note">{status.blocked}</p>}
      {status !== undefined && 'message' in status && status.message !== undefined && <p className="lc-settings__note" role="status">{status.message}</p>}
    </div>
    {switchOf('Look while my computer is off', on, () => { set(!on) }, status === undefined || status.state === 'busy' || (status.state === 'off' && status.blocked !== undefined))}
  </div>
}

/** Entries shown at first, and added by each Show more. */
const SAW_PAGE = 30

/*
 * WHAT SCOUT SAW (0.828, shared/scout-log.ts): its looks of the last fourteen days, newest first, read when opened --
 * what each raised, set aside and why, and what it did not read. "Only what it left out" keeps the looks that set
 * something aside, were not taken, or could not read a watch.
 */
function ScoutSaw({ api }: { readonly api: ScoutApi }) {
  const [log, setLog] = useState<{ readonly entries: readonly ScoutLogEntry[]; readonly held: readonly ScoutHeldAlert[] } | 'failed' | undefined>(undefined)
  const [open, setOpen] = useState(false)
  const [asideOnly, setAsideOnly] = useState(false)
  const [shown, setShown] = useState(SAW_PAGE)
  const read = (): void => {
    if (api.log === undefined) return
    api.log().then((answer) => setLog(answer), () => setLog('failed'))
  }
  const entries = typeof log === 'object' ? [...log.entries].reverse().filter((entry) => !asideOnly || setSomethingAside(entry)) : []
  return <div className="lc-settingline lc-settingline--sub lc-scout__goalline">
    <div className="lc-settingline__text">
      <h3 className="lc-settings__heading">What Scout saw</h3>
      <p className="lc-settings__lede">Each look of the last 14 days, newest first: what it raised, what it set aside and why, and what it did not read. Ask Scout in its conversation why it did not tell you about something, and it answers from this.</p>
      {!open
        ? <span className="lc-scout__goalkeys"><button type="button" className="lc-control lc-control--boxed" disabled={api.log === undefined} onClick={() => { setOpen(true); read() }}>Show what Scout saw</button></span>
        : <>
          <span className="lc-scout__goalkeys">
            <button type="button" className="lc-control lc-control--boxed" onClick={() => { setOpen(false); setShown(SAW_PAGE) }}>Hide</button>
            <button type="button" className="lc-control" onClick={read}>Refresh</button>
            <label className="lc-scoutlog__filter"><input type="checkbox" checked={asideOnly} onChange={(event) => { setAsideOnly(event.target.checked); setShown(SAW_PAGE) }} /> Only what it left out</label>
          </span>
          {log === undefined && <p className="lc-settings__note" role="status">Reading what Scout saw…</p>}
          {log === 'failed' && <p className="lc-settings__note" role="status">What Scout saw could not be read. Try Refresh.</p>}
          {typeof log === 'object' && entries.length === 0 && <p className="lc-settings__note" role="status">{asideOnly ? 'Nothing left out in these 14 days.' : 'No looks yet in these 14 days.'}</p>}
          {entries.length > 0 && <ol className="lc-scoutlog" aria-label="What Scout saw">
            {entries.slice(0, shown).map((entry) => <li key={`${entry.at}-${String(entry.looks)}`} className="lc-scoutlog__look">
              <span className="lc-scoutlog__when">{lookName(entry)}</span>
              <ul className="lc-scoutlog__lines">{lookLines(entry, typeof log === 'object' ? log.held : []).map((line, index) => <li key={index}>{line}</li>)}</ul>
            </li>)}
          </ol>}
          {entries.length > shown && <span className="lc-scout__goalkeys"><button type="button" className="lc-control" onClick={() => setShown((was) => was + SAW_PAGE)}>{`Show ${String(Math.min(SAW_PAGE, entries.length - shown))} more`}</button></span>}
        </>}
    </div>
  </div>
}

/**
 * SCOUT IN SETTINGS (0.756, shared/scout.ts): on or off, what it watches, how often it looks, and whether a card that
 * needs the person now comes as a Windows notification. Its model is its teammate's, changed like any teammate's;
 * this line says which one it reads with and what that costs. Since 0.815 also what to keep watch for, in the
 * person's own words, the folder's check, quiet hours, a look that waits while their own turn runs, and what today
 * used.
 */
export function ScoutSetting({ heading, state, model, route, checkCommand, onChangeModel, team = [], api = typeof window === 'undefined' ? undefined : window.desktop?.scout }: {
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
  /** The teammates who may prepare Scout's fixes (0.828): everyone but Scout. */
  readonly team?: readonly { readonly teammateId: string; readonly name: string }[]
  readonly api?: ScoutApi
}) {
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  /** The goal as it is being written; undefined while it reads as saved. */
  const [goal, setGoal] = useState<string | undefined>(undefined)
  /** Scout's notes as they are being changed; undefined while they read as kept. */
  const [notes, setNotes] = useState<string | undefined>(undefined)
  if (api === undefined) return null
  const on = state?.settings.on === true
  const savedGoal = state?.settings.goal ?? ''
  const quiet = state?.settings.quiet
  const today = state?.today
  const status = [
    ...(state?.lastLookAt === undefined ? [] : [`Last looked ${ago(state.lastLookAt, new Date())}${state.cards.length === 0 ? ', and nothing needed you.' : '.'}`]),
    ...(today === undefined ? [] : [`Today: ${String(today.looks)} look${today.looks === 1 ? '' : 's'}, ${today.asked === 0 ? 'none of them needing its model' : `${String(today.asked)} of them read by ${model ?? 'its model'}`}${state?.settings.fixes?.on === true ? `, ${String(today.fixes ?? 0)} of ${String(state.settings.fixes.perDay)} fix${state.settings.fixes.perDay === 1 ? '' : 'es'} prepared` : ''}.`]),
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
    void act(() => api.set({ watches: { ...state.settings.watches, [key]: !(state.settings.watches[key] ?? true) } }))
  }
  const saveGoal = (): void => {
    if (goal === undefined) return
    const words = goal
    void act(async () => { await api.set({ goal: words }); setGoal(undefined) })
  }
  const savedNotes = (state?.notes ?? []).join('\n')
  const saveNotes = (): void => {
    if (notes === undefined) return
    const text = notes
    void act(async () => { await api.saveNotes(text); setNotes(undefined) })
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
      {/* Scout's notes (0.816): what it keeps from its looks, read, changed or cleared here. */}
      <div className="lc-settingline lc-settingline--sub lc-scout__goalline">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">{`Scout’s notes${(state.notes?.length ?? 0) === 0 ? '' : ` · ${String(state.notes?.length ?? 0)}`}`}</h3>
          <p className="lc-settings__lede">{`What Scout keeps from its looks, one line each, and reads on every look. Change or clear them here. It keeps the newest ${String(MAX_SCOUT_NOTES)} lines, each up to ${String(MAX_SCOUT_NOTE)} characters.`}</p>
          <textarea className="lc-input lc-scout__goal" aria-label="Scout’s notes" disabled={busy} rows={4}
            placeholder="Nothing yet. A look that learns something worth keeping adds a line here."
            value={notes ?? savedNotes}
            onChange={(event) => setNotes(event.target.value)} />
          {notes !== undefined && notes !== savedNotes && <span className="lc-scout__goalkeys">
            <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={saveNotes}>Save</button>
            <button type="button" className="lc-control" disabled={busy} onClick={() => setNotes(undefined)}>Cancel</button>
          </span>}
        </div>
      </div>
      <ScoutSaw api={api} />
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Pull requests and their checks</h3>
          <p className="lc-settings__lede">Yours, the reviews asked of you, and the default branch’s checks, through the GitHub CLI you signed in.</p>
        </div>
        {switchOf('Pull requests and their checks', state.settings.watches.github, () => { watch('github') })}
      </div>
      <div className="lc-settingline lc-settingline--sub">
        <div className="lc-settingline__text">
          <h3 className="lc-settings__heading">Your /locust comments</h3>
          <p className="lc-settings__lede">A comment of yours on this folder's GitHub with a line starting /locust is an ask. Scout brings it to Home with the step drafted; /locust ada names the teammate.</p>
        </div>
        {switchOf('Your /locust comments', state.settings.watches.comments !== false, () => { watch('comments') })}
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
      <FixesLine state={state} team={team} busy={busy} checkCommand={checkCommand} act={act} api={api} switchOf={switchOf} />
      {api.away !== undefined && <AwayLine api={api.away} switchOf={switchOf} lastLookWords={(iso) => ago(iso, new Date())} />}
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
          <h3 className="lc-settings__heading">Morning brief</h3>
          <p className="lc-settings__lede">One card on Home each morning: who waits on you, what finished or failed since yesterday, and how the check stands. The first look from that hour on brings it.</p>
        </div>
        <Select className="lc-control" aria-label="Scout’s morning brief" disabled={busy} value={state.settings.briefAt === undefined ? 'off' : String(state.settings.briefAt)}
          onChange={(next) => { void act(() => api.set({ briefAt: next === 'off' ? null : Number(next) })) }}
          options={[{ value: 'off', label: 'Off' }, ...SCOUT_BRIEF_HOURS.map((hour) => ({ value: String(hour), label: `At ${hourWords(hour)}` }))]} />
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
