import { useRef, type ReactElement } from 'react'
import { takesSkills } from '../../../shared/skill-runtimes.js'
import { MAX_TEAMMATE_INSTRUCTIONS } from '../../../shared/ipc.js'
import type { TeammateRoute } from '../../../shared/ipc.js'
import { MAX_SKILL_BODY, MAX_SKILL_DESCRIPTION, MAX_SKILL_NAME, repeatsOf, TUNE_DRAFT_COST, TUNE_SKILL_INCOMPLETE, tuneSkillOf, type TuneDrafts, type TuneSkill } from '../../../shared/tune.js'
import { useModal } from '../useModal.js'

export interface TuneCard {
  readonly id: number
  readonly teammateId: string
  readonly route: TeammateRoute
  readonly name: string
  readonly model: string
  readonly free: boolean
  readonly prompt: string
  readonly current: string
  readonly drafts: TuneDrafts
  readonly drafting: boolean
  readonly draftRunId?: string
  readonly starting?: boolean
  readonly problem?: string
  /** A skill set aside by Use instructions instead (0.811), back with Write a skill instead. */
  readonly skillAside?: TuneSkill
}

/** Whether the card can run: a skill drafted for What it knows must read whole first (0.811). */
export function tuneSkillReady(card: TuneCard): boolean {
  return card.drafts.knows.skill === undefined || tuneSkillOf(card.drafts.knows.skill) !== undefined
}

/** A name as Claude Code takes one, typed: lower case, a space or underscore becomes a hyphen, anything else is left out. */
const skillNameTyped = (value: string): string => value.toLowerCase().replace(/[\s_]+/g, '-').replace(/[^a-z0-9-]/g, '').slice(0, MAX_SKILL_NAME)

/** TUNE THIS TEAMMATE (0.809): review the edits before three trials, and never save instructions here. */
export function TuneTeammate({ card, onChange, onRun, onCancel }: {
  readonly card: TuneCard
  readonly onChange: (edit: (current: TuneCard) => TuneCard) => void
  readonly onRun: () => void
  readonly onCancel: () => void
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)
  // While the drafts are being written they are placeholders, so nothing repeats yet.
  const repeats: ReturnType<typeof repeatsOf> = card.drafting ? { distinct: 3 } : repeatsOf(card.current, card.drafts)
  const skill = card.drafts.knows.skill
  const skillReady = tuneSkillReady(card)
  const editSkill = (change: Partial<TuneSkill>): void =>
    onChange((current) => current.drafts.knows.skill === undefined ? current : { ...current, drafts: { ...current.drafts, knows: { ...current.drafts.knows, skill: { ...current.drafts.knows.skill, ...change } } } })
  // Either way round, for a teammate whose agent takes skills (0.811; Codex and OpenCode 0.813): the skill drafted, or set aside, or one the person writes.
  const canSkill = takesSkills(card.route.runtime) && !card.drafting
  const writeSkill = (): void =>
    onChange(({ skillAside, ...current }) => ({ ...current, drafts: { ...current.drafts, knows: { ...current.drafts.knows, skill: skillAside ?? { name: '', description: '', body: '' } } } }))
  const useInstructions = (): void =>
    onChange((current) => {
      const { skill: aside, ...knows } = current.drafts.knows
      return { ...current, ...(aside === undefined ? {} : { skillAside: aside }), drafts: { ...current.drafts, knows } }
    })
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-tune" role="dialog" aria-modal="true" aria-label={`Tune ${card.name}`}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Tune this teammate</span>
          <span className="lc-dialog__sub">{card.name}</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" disabled={card.starting} onClick={onCancel}>×</button>
        </div>
        <div className="lc-dialog__body lc-tune__body">
          <label className="lc-tune__ask">Ask to run again
            <textarea className="lc-input" aria-label="Ask to run again" rows={2} maxLength={8000} value={card.prompt} disabled={card.starting}
              onChange={(event) => { const prompt = event.target.value; onChange((current) => ({ ...current, prompt })) }} />
          </label>
          <p className="lc-dialog__note">{TUNE_DRAFT_COST} {card.drafting ? 'Drafting the two edits…' : 'Review and edit all three before running. Keeping Current is a valid outcome.'}</p>
          <div className="lc-tune__columns">
            {(['current', 'how', 'knows'] as const).map((key) => {
              const title = key === 'current' ? 'Current' : key === 'how' ? 'How it works' : 'What it knows'
              const why = key === 'current' ? 'The teammate’s own standing instructions. Empty is allowed.' : card.drafts[key].why
              // WHAT IT KNOWS, AS A SKILL (0.811): a Claude Code teammate's draft is a skill beside Current's instructions.
              if (key === 'knows' && skill !== undefined) {
                return <div className="lc-tune__column" key={key} role="group" aria-label="What it knows skill">
                  <span className="lc-tune__title">{title}</span>
                  <span className="lc-dialog__note lc-tune__why">{why}</span>
                  {/* One block in the column's third row, so the three columns' rows still line up. */}
                  <div className="lc-tune__skill">
                    <span className="lc-dialog__note">A skill beside Current’s instructions: Claude Code loads it only when the work matches when to use it.</span>
                    <label className="lc-tune__skillfield">
                      <span className="lc-tune__skilllabel">Name</span>
                      <input className="lc-input lc-mono" aria-label="What it knows skill name" value={skill.name} maxLength={MAX_SKILL_NAME} disabled={card.starting}
                        onChange={(event) => editSkill({ name: skillNameTyped(event.target.value) })} />
                    </label>
                    <label className="lc-tune__skillfield">
                      <span className="lc-tune__skilllabel">When to use it</span>
                      <textarea className="lc-input lc-tune__skillwhen" aria-label="What it knows skill: when to use it" rows={1} maxLength={MAX_SKILL_DESCRIPTION} value={skill.description} disabled={card.starting}
                        onChange={(event) => editSkill({ description: event.target.value.replace(/[\r\n]+/g, ' ') })} />
                    </label>
                    <label className="lc-tune__skillfield lc-tune__skillfield--grow">
                      <span className="lc-tune__skilllabel">What it says</span>
                      <textarea className="lc-input lc-tune__instructions" aria-label="What it knows skill: what it says" rows={5} maxLength={MAX_SKILL_BODY} value={skill.body} disabled={card.starting}
                        onChange={(event) => editSkill({ body: event.target.value })} />
                    </label>
                    {!skillReady && <span className="lc-dialog__note lc-tone-red" role="status">{TUNE_SKILL_INCOMPLETE}</span>}
                  </div>
                  <span className="lc-tune__foot">
                    <span className="lc-dialog__note lc-mono">{skill.body.length} / {MAX_SKILL_BODY}</span>
                    <button type="button" className="lc-linkbutton" disabled={card.starting} onClick={useInstructions}>Use instructions instead</button>
                  </span>
                </div>
              }
              const text = key === 'current' ? card.current : card.drafts[key].instructions
              const toggle = key === 'knows' && canSkill
              // A button cannot sit in a label: with the switch beside it, the column is a group and the box says its name.
              const Column = toggle ? 'div' : 'label'
              return <Column className="lc-tune__column" key={key} {...(toggle ? { role: 'group', 'aria-label': title } : {})}>
                <span className="lc-tune__title">{title}</span>
                <span className="lc-dialog__note lc-tune__why">{why}</span>
                <textarea className="lc-input lc-tune__instructions" aria-label={`${title} instructions`} rows={8} maxLength={MAX_TEAMMATE_INSTRUCTIONS}
                  value={text} disabled={card.starting || (key !== 'current' && card.drafting)}
                  onChange={(event) => { const text = event.target.value; onChange((current) => key === 'current' ? { ...current, current: text } : { ...current, drafts: { ...current.drafts, [key]: { ...current.drafts[key], instructions: text } } }) }} />
                <span className="lc-tune__foot">
                  <span className="lc-dialog__note lc-mono">{text.length} / {MAX_TEAMMATE_INSTRUCTIONS}</span>
                  {key !== 'current' && repeats[key] !== undefined && <span className="lc-dialog__note lc-tune__same">Same as {repeats[key]}</span>}
                  {toggle && <button type="button" className="lc-linkbutton" disabled={card.starting} onClick={writeSkill}>Write a skill instead</button>}
                </span>
              </Column>
            })}
          </div>
          <p className="lc-tune__cost">Three runs of {card.model}{card.free ? ' — free.' : ' — counts against your plan three times.'} Each works in its own copy where Compare supports one; you choose what to keep.</p>
          {repeats.distinct < 3 && <p className="lc-tune__cost">{repeats.distinct === 1
            ? 'All three setups are the same, so the runs only show how much one run changes by chance. Edit a draft to try something different.'
            : 'Two of the three setups are the same, so those two runs also show how much one run changes by chance.'}</p>}
          {card.problem !== undefined && <p className="lc-dialog__error" role="alert">{card.problem}</p>}
        </div>
        <div className="lc-dialog__foot">
          <button type="button" className="lc-ghostbutton" disabled={card.starting} onClick={onCancel}>Cancel</button>
          <button type="button" className="lc-primarybutton" disabled={card.drafting || card.starting || card.prompt.trim().length === 0 || !skillReady} onClick={onRun}>
            {card.starting ? 'Starting…' : 'Run all three'}
          </button>
        </div>
      </div>
    </div>
  )
}
