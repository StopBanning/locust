import { accountName } from '../agentAccounts.js'
import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'
import { useModal } from '../useModal.js'

import { BOT_SHAPES, botFor, samePet, screenSuits, seedAvatar, shuffledAvatar } from '../../../shared/avatar.js'
import type { AvatarSpec, BotFace, BotShape, BotSpec, PetRef } from '../../../shared/avatar.js'
import { useTerminalFaces } from '../botLook.js'
import { usePetList, usePetLook } from '../pets.js'
import { isPetPick } from '../../../shared/pet-picks.js'
import type { MissionMode, PublicPet, PublicTeammate, TeammateHue, TeammateRole, PublicConnector, PublicModel, PublicRuntimeStatus, TeammateRoute } from '../../../shared/ipc.js'
import { MAX_TEAMMATE_INSTRUCTIONS, ROLE_DESCRIPTIONS } from '../../../shared/ipc.js'
import { defaultEffort, modelFamily, modeRunsOn, modesFor, modeSummary } from '../status.js'
import { policyModeAllowed, policyMonthlyLimit } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'
import { effortFooter } from '../effortLevels.js'
import { effortScale, joinEffort, splitEffort } from '../effortScale.js'
import { EffortSlider } from './EffortSlider.js'
import { routeLabel } from './GroupSettingsDialog.js'
import { RoutePicker } from './RoutePicker.js'
import type { RouteChoice } from './RoutePicker.js'
import { TeammateBot } from './TeammateBot.js'
import { PetCredit, PetPickTiles, PetRemovalControls, usePetRemoval } from './PetPicks.js'
import { branchNameFor } from '../../../shared/worktree-name.js'
import { dollars, isMonthlyLimit } from '../../../shared/spend.js'
import type { Spend } from '../../../shared/spend.js'
import { agentNamed } from '../../../shared/runtimes.js'
import { CHATGPT_VOICES, isChatGptVoice, voiceName } from '../../../shared/voice-call.js'
import { Select } from './Select.js'
import { VoicePlay } from './VoicePlay.js'
import { Icon } from './Icon.js'
import { tuneLessonLine, type PublicTeammateSkill, type PublicTuneLesson } from '../../../shared/tune.js'

/** What the dialog hands back. A limit is `null` when an edit removes one. */
export interface TeammateDraft {
  readonly name: string
  readonly hue: TeammateHue
  readonly role: TeammateRole
  readonly roleTitle?: string
  readonly worktree?: boolean
  readonly avatar: AvatarSpec
  readonly route?: TeammateRoute
  readonly monthlyLimitUsd?: number | null
  /** Its own standing instructions (0.706); `null` when an edit clears them. */
  readonly instructions?: string | null
  /** Its voice in a call (0.746): one of ChatGPT's nine; `null` when an edit goes back to Talk's own. */
  readonly voice?: string | null
}

/**
 * The limit as typed: nothing is no limit; "$12.50", "12.5" and "1,000" are
 * amounts, rounded to the cent; anything else is `invalid`, and the dialog
 * says so rather than saving a limit the person did not mean.
 */
/**
 * What the limit field says under itself: how the limit works, and -- for a
 * teammate who has spent money this month -- how much, so a person setting
 * a limit sees what it is set against.
 */
export function limitHint(name: string, spent: Spend | undefined, editing: boolean): string {
  const how = `Checked before each run: once this month's priced runs reach it, ${name} starts nothing until you raise it or the month ends. A run already going finishes. Plans and free models are not priced, so they never count. Nor do models of your own, which report no price; ${name}'s card counts those runs.`
  if (!editing || spent?.usd === undefined) return how
  return `${dollars(spent.usd)} spent this month so far. ${how}`
}

export function parsedLimit(text: string): number | undefined | 'invalid' {
  const cleaned = text.trim().replace(/^\$/, '').replace(/,/g, '').trim()
  if (cleaned.length === 0) return undefined
  if (!/^\d+(\.\d*)?$|^\.\d+$/.test(cleaned)) return 'invalid'
  const amount = Math.round(Number(cleaned) * 100) / 100
  return isMonthlyLimit(amount) ? amount : 'invalid'
}

const HUES: readonly { readonly hue: TeammateHue; readonly label: string }[] = [
  { hue: 'lime', label: 'Lime' },
  { hue: 'blue', label: 'Blue' },
  { hue: 'violet', label: 'Violet' },
  { hue: 'clay', label: 'Clay' },
  { hue: 'teal', label: 'Teal' },
  { hue: 'butter', label: 'Butter' },
  { hue: 'rose', label: 'Rose' },
  { hue: 'slate', label: 'Slate' },
  { hue: 'pearl', label: 'Pearl' },
  { hue: 'indigo', label: 'Indigo' },
  { hue: 'coral', label: 'Coral' }
]

/**
 * The colour a new teammate starts on: the first one nobody on the team wears
 * yet, then round again once every colour is taken.
 *
 * It was always lime, so a team built by accepting the defaults was a row of
 * one colour, and the sidebar's faces -- the thing that says whose each
 * conversation is -- could not tell them apart (the design review, #7). The
 * person can still pick any colour; this is only where the picker starts.
 */
/**
 * The name the empty box suggests: the first of these not already on the team
 * (0.408). It was always "Wren" -- and with the Build software starter team
 * made, Wren is the one name the box would refuse (fresh-eyes check).
 */
export const EXAMPLE_NAMES = ['Wren', 'Robin', 'Sable', 'Juno', 'Atlas', 'Pip', 'Quill', 'Iris', 'Moss', 'Rook', 'Penny', 'Fern'] as const
export function suggestedName(taken: readonly string[]): string {
  const lower = new Set(taken.map((name) => name.trim().toLowerCase()))
  return EXAMPLE_NAMES.find((name) => !lower.has(name.toLowerCase())) ?? 'Wren'
}

export function freshHue(taken: readonly TeammateHue[]): TeammateHue {
  const unused = HUES.find((option) => !taken.includes(option.hue))
  return unused?.hue ?? HUES[taken.length % HUES.length]!.hue
}

/** What each shape is called in the Look grid; Locust's own two say so. */
const SHAPE_NAMES: Readonly<Record<BotShape, string>> = {
  clover: 'Clover',
  flower: 'Flower',
  triangle: 'Triangle',
  square: 'Square',
  blob: 'Blob',
  ghost: 'Ghost',
  circle: 'Circle',
  drop: 'Drop',
  star: 'Star',
  droid: 'Droid',
  mech: 'Mech',
  alien: 'Alien',
  hexagon: 'Hexagon',
  cat: 'Cat',
  cloud: 'Cloud',
  pill: 'Pill',
  pebble: 'Pebble',
  puddle: 'Puddle',
  hopper: 'Hopper, a Locust',
  swarm: 'Swarm, a Locust',
  critter: 'Critter',
  spark: 'Spark',
  prompt: 'Prompt'
}

const ROLES: readonly { readonly role: TeammateRole; readonly description: string }[] = (
  ['Code & Migrations', 'Research & Briefs', 'Ops & Scheduling', 'Docs & QA', 'Data & Reporting', 'Chief of Staff', 'Custom'] as const
).map((role) => ({ role, description: ROLE_DESCRIPTIONS[role] }))

/**
 * The face is generated, then owned: the dialog seeds a look the moment it
 * opens (from a throwaway id, so two dialogs opened in a row start from
 * different faces), the person can shuffle it or recolour it, and whatever is
 * on the preview when they create is what gets persisted with the record.
 * Never derived from the name -- a rename must not change a face.
 */
/**
 * What the next mission may do, in the words the mode menu uses.
 *
 * A `switch` with no default, so the union is exhausted and a mode added
 * later cannot fall through. It used to end in a bare `return 'Ask ...'`,
 * and `auto` -- added after this was written -- landed there: a composer
 * reading "may edit anything on this machine" opened a dialog promising
 * "every write refused". Caught 2026-09-09 in the folder drive's capture,
 * which is the THIRD time this dialog has claimed a permission the run did
 * not have, and the first time in the direction that understates it.
 */
/* `modeSummary` now lives in `status.ts` with every other mode phrasing. */

/**
 * A connector by a name a person reads (0.514). A first-hour pass on 0.512
 * met `plugin:small-business:zoho-projects` in the editor. The id stays on
 * hover; the button says "Zoho Projects".
 */
export function connectorLabel(name: string): string {
  const bare = name.replace(/^claude\.ai /, '')
  const plugin = /^plugin:[^:]+:(.+)$/.exec(bare)
  if (plugin === null) return bare
  return (plugin[1] ?? bare)
    .split(/[-_]+/)
    .filter((word) => word.length > 0)
    .map((word) => `${word.charAt(0).toUpperCase()}${word.slice(1)}`)
    .join(' ')
}

/** Where a pet came from, said under the pets and on each one's tile. */
export function petOrigin(pet: PublicPet): string {
  switch (pet.source) {
    case 'bundled':
      return 'comes with Locust'
    case 'gallery':
      return 'from openpets.dev'
    case 'codex':
      return 'from Codex'
  }
}

/** The look without its pet: a bot again. */
function withoutPet(avatar: AvatarSpec): AvatarSpec {
  const { pet: _pet, ...bot } = avatar
  return bot
}

export function NewTeammateDialog({
  onCancel,
  onCreate,
  spentThisMonth,
  error,
  initial,
  mode,
  takenHues = [],
  takenNames = [],
  onChooseFolder,
  folderNotice,
  connectors,
  onSetConnectors,
  composerRoute,
  picker,
  platform,
  lessons,
  onRemoveLesson,
  skills,
  onRemoveSkill
}: {
  readonly onCancel: () => void
  /** Resolves when the save has landed; the button is held until then (L22). */
  readonly onCreate: (input: TeammateDraft) => void | Promise<unknown>
  /** What this teammate has spent this month, for the limit field to say (editing only). */
  readonly spentThisMonth?: Spend
  readonly error: string | undefined
  /** Set to edit an existing teammate: the same dialog, filled in, saving instead of creating. */
  readonly initial?: PublicTeammate
  /** The mode the next mission would actually run in, so the card cannot promise another. */
  readonly mode: MissionMode
  /** The colours the team already wears, so a new teammate starts on one it does not. */
  readonly takenHues?: readonly TeammateHue[]
  /** The other teammates' names, so a name already taken is said before Save (A2.18). */
  readonly takenNames?: readonly string[]
  /**
   * Ask the host for this teammate's own folder, or clear it.
   *
   * Takes effect at once rather than on Save, because the path is the HOST's
   * to name -- the renderer is handed a teammate back, never a path it could
   * have typed. Absent while creating: a teammate with no id yet has nothing
   * to write the folder onto.
   */
  readonly onChooseFolder?: (clear: boolean) => void
  /** Why the last folder request did nothing. Absent when it worked, or was cancelled. */
  readonly folderNotice?: string
  /**
   * Every connector the person's Claude Code reports, for narrowing. Absent
   * while it is being read, or where narrowing is not offered.
   */
  readonly connectors?: readonly PublicConnector[]
  /** The whole list of ticked names; empty means every connector. Takes effect at once. */
  readonly onSetConnectors?: (names: readonly string[]) => void
  /**
   * What the chat box is set to: the model a teammate with none of its own
   * runs on, and so what the Model row says until one is picked.
   */
  readonly composerRoute?: TeammateRoute
  /**
   * What the chat's own model picker lists, so the Model row can offer the
   * same picker. Absent, the row states the model and offers nothing.
   */
  readonly picker?: {
    readonly runtimes: readonly PublicRuntimeStatus[]
    readonly models: readonly PublicModel[]
    readonly resolvedModels: ReadonlyMap<string, string>
    readonly recentRoutes: readonly string[]
    readonly limitedRuntimes: ReadonlyMap<string, string>
  }
  /** Which modes a runtime can run turns on it (Cursor cannot be held read-only on Windows). */
  readonly platform?: string
  /** What earlier tunes of this teammate noted (0.810), each removable here. */
  readonly lessons?: readonly PublicTuneLesson[]
  readonly onRemoveLesson?: (lessonId: string) => void | Promise<unknown>
  /** The skills it kept from tunes (0.811), each removable here. */
  readonly skills?: readonly PublicTeammateSkill[]
  /** Resolves with why the skill stayed, or nothing when it went. */
  readonly onRemoveSkill?: (name: string) => Promise<string | undefined>
}): ReactElement {
  const editing = initial !== undefined
  const policy = useOrganizationPolicy()
  const policyLimit = policy?.policy.monthlyLimitUsd
  const [name, setName] = useState(initial?.name ?? '')
  // A save in flight (L22): the button is held until it lands.
  const [saving, setSaving] = useState(false)
  // Removing a kept skill (0.811) takes effect at once, as Remove does for a tune lesson; one at a time.
  const [removingSkill, setRemovingSkill] = useState<string>()
  const [skillProblem, setSkillProblem] = useState<string>()
  const pressed = useRef(false)
  const [hue, setHue] = useState<TeammateHue>(initial?.hue ?? freshHue(takenHues))
  const [avatar, setAvatar] = useState<AvatarSpec>(
    () => initial?.avatar ?? seedAvatar(`draft_${Date.now()}_${Math.random()}`)
  )
  /*
   * A NEW TEAMMATE STARTS NEUTRAL (0.530). It started as Code & Migrations --
   * "Repo work, refactors, test runs", and starters about "the riskiest file in
   * this repo" -- for everyone. Sol's 0.528 pass, as an operations lead with a
   * folder of documents, got a programmer. Research & Briefs reads whatever the
   * folder holds; Code is one click away for the people who want it.
   */
  const [role, setRole] = useState<TeammateRole>(initial?.role ?? 'Research & Briefs')
  const [roleTitle, setRoleTitle] = useState(initial?.roleTitle ?? '')
  const [worktree, setWorktree] = useState(initial?.worktree === true)
  /*
   * A MONTHLY LIMIT, in dollars (0.353; Paperclip's idea worth taking). A
   * teammate that replies and runs routines on its own can spend while
   * nobody is watching; the host checks this before every run it starts.
   * Typed as text so "12.50" can be typed through "12." on the way.
   */
  const [limitText, setLimitText] = useState(() => policyMonthlyLimit(policy, initial?.monthlyLimitUsd)?.toFixed(2) ?? '')
  const limit = policyLimit === 0 ? 0 : parsedLimit(limitText)
  const overPolicyLimit = policyLimit !== undefined && (typeof limit !== 'number' || limit > policyLimit)
  // THIS TEAMMATE'S OWN INSTRUCTIONS (0.706, from the Paperclip scrub): what a group's are, for one teammate.
  const [instructions, setInstructions] = useState(initial?.instructions ?? '')
  // ITS VOICE IN A CALL (0.746): Talk pressed in its conversation speaks as it, in this voice.
  const [voice, setVoice] = useState(initial?.voice ?? '')
  /*
   * THE TEAMMATE'S OWN MODEL, chosen here (0.311). Colin, 2026-09-24: "do we
   * have the ability to switch a teammates model? like not when youre in the
   * chat but the actual designated teammate". It could only change by
   * sending them a message on another model; rooms and routines use it, so
   * to move a teammate in a room you had to talk to them alone first. Only a
   * model actually picked is saved -- opening the dialog changes nothing.
   */
  const [picked, setPicked] = useState<TeammateRoute>()
  const [picking, setPicking] = useState(false)
  const ownRoute = initial?.route
  const shownRoute = picked ?? ownRoute ?? composerRoute
  const pick = (choice: RouteChoice): void => {
    const was = ownRoute ?? composerRoute
    const wanted = was?.mode ?? mode
    // A mode the new runtime cannot run is not one it can be kept in.
    const kept = modeRunsOn(wanted, choice.runtime, platform) && policyModeAllowed(policy, wanted) ? wanted : modesFor(choice.runtime, platform).find((value) => policyModeAllowed(policy, value)) ?? 'ask'
    // The level goes with the model that reported it; another model starts on its own default.
    const effort = was !== undefined && was.runtime === choice.runtime && was.model === choice.model ? was.effort : undefined
    setPicked({ runtime: choice.runtime, ...(choice.account === undefined ? {} : { account: choice.account }), model: choice.model, mode: kept, ...(effort === undefined ? {} : { effort }) })
    setPicking(false)
  }
  /*
   * THE TEAMMATE'S OWN EFFORT, beside their model. Colin, 2026-09-24: "for
   * model picker in edit teammate we need to be able to choose effort too".
   * The route always carried a level, but the dialog never showed it: a new
   * model silently started on its default, and the only way to change it was
   * a chat with them. The same control as the composer's, on the levels THIS
   * model reports; a level set here is saved like a picked model.
   */
  const family = shownRoute === undefined || picker === undefined ? undefined : modelFamily(picker.models, shownRoute.runtime, shownRoute.model)
  const supportedEfforts = family?.supportedEfforts ?? []
  const effortOfId = Object.entries(family?.variants ?? {}).find(([, id]) => id === shownRoute?.model)?.[0]
  const shownEffort = shownRoute?.effort ?? effortOfId ?? defaultEffort(supportedEfforts, family?.defaultEffort)
  const { bases: effortBases, hasFast: effortHasFast } = effortScale(supportedEfforts)
  const { base: effortBase, fast: effortIsFast } = splitEffort(shownEffort ?? effortBases[0] ?? '')
  const chooseEffort = (level: string | undefined): void => {
    if (level !== undefined && shownRoute !== undefined) setPicked({ ...shownRoute, effort: level })
  }
  const nameRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    nameRef.current?.focus()
  }, [])

  const trimmed = name.trim()
  // A2.18: a name another teammate has would make both unreachable by name.
  const taken = takenNames.find((other) => other.trim().toLowerCase() === trimmed.toLowerCase())
  const agentMatch = trimmed.length === 0 ? undefined : agentNamed(trimmed)
  const canCreate = trimmed.length > 0 && taken === undefined && limit !== 'invalid' && !overPolicyLimit
  const look = botFor(avatar)
  /*
   * THE FACE: EYES, A MOUTH, OR A SCREEN (0.562). Colin: "should we just
   * remove it alright or have it toggleable in the teammate editor" -- the
   * editor. A screen is offered while Terminal faces is on (Settings >
   * Appearance) and, until the person picks, follows what suits the shape
   * (`screenSuits`); the two mascots' faces, Prompt's and Spark's, are always their screens.
   */
  const terminal = useTerminalFaces()
  const alwaysScreen = look.shape === 'prompt' || look.shape === 'spark'
  const wearsScreen = alwaysScreen || (terminal && (look.screen ?? screenSuits(look.shape)))
  const faceChoice: BotFace | 'screen' = wearsScreen ? 'screen' : look.face
  /** The bot with this shape and the face as chosen so far (a screen choice travels with it). */
  const withShape = (current: AvatarSpec, shape: BotShape): BotSpec => {
    const held = botFor(current)
    return { shape, face: held.face, ...(held.screen === undefined ? {} : { screen: held.screen }) }
  }

  /*
   * PETS, IN THE SAME LIST OF LOOKS (0.563). Colin, 2026-10-03: "theyre just
   * going to be added to the list of potential choices for teammates". Under
   * the bots: the pets Locust offers (PetPicks, 0.564 -- the gallery went),
   * the person's own Codex pets, and a pet this teammate already wears that
   * is neither, picked as a bot is picked. A pet keeps its own face, so the Eyes / Mouth / Screen
   * choice steps aside while one is worn, and the colour swatches become
   * plain colours: the colour still marks the teammate, it does not tint the
   * pet. A pet whose drawings were measured for a screen (2026-10-05,
   * petScreens.ts: Codex Buddy), or that becomes a puppet with one
   * (petPuppets.ts: Cabin, Macintosh, Bitty and the rest), has a choice of its
   * own in its place: as its maker drew it, or the screen -- offered as a
   * bot's screen is, while Terminal faces is on.
   */
  const { pets, removed: removedPets } = usePetList()
  const wornPet: PetRef | undefined = avatar.pet
  const wornLook = usePetLook(wornPet)
  const wornEntry = wornPet === undefined ? undefined : pets?.find((pet) => samePet(pet, wornPet))
  const [petNotice, setPetNotice] = useState<string>()
  const petRemoval = usePetRemoval(setPetNotice)
  // Beside the picks: the person's own Codex pets, and a pet worn that is neither (kept, so it shows as chosen).
  const offered = (pet: PetRef): boolean => pet.source === 'gallery' && isPetPick(pet.id)
  const otherPets: readonly PublicPet[] = [
    ...(pets ?? []).filter((pet) => pet.source === 'codex' || (samePet(wornPet, pet) && !offered(pet))),
    // Worn, not offered, and the list not read yet: its tile now, named once the list arrives.
    ...(wornPet !== undefined && !offered(wornPet) && wornPet.source !== 'codex' && pets?.some((pet) => samePet(pet, wornPet)) !== true
      ? [{ source: wornPet.source, id: wornPet.id, displayName: wornPet.id, description: '', rows: 9 as const }]
      : [])
  ]
  const wearPet = (pet: PublicPet): void => {
    setPetNotice(undefined)
    // "Removed 3 pets" beside "2 removed" once one came back read wrong (Sonnet's 0.569 pass).
    petRemoval.dismiss()
    setAvatar((current) => ({ ...current, pet: { source: pet.source, id: pet.id } }))
    // Picking a pet is the quickest way to a new teammate: an empty name takes the pet's own.
    if (!editing && name.trim().length === 0) {
      const own = pet.displayName.slice(0, 40).trim()
      if (own.length > 0 && !takenNames.some((other) => other.trim().toLowerCase() === own.toLowerCase())) setName(own)
    }
  }
  const who = trimmed.length === 0 ? 'This teammate' : trimmed
  const petCaption =
    wornPet === undefined
      ? undefined
      : wornLook?.status === 'missing'
        ? `${who}\u2019s pet could not be found, so ${who} is showing a bot. ${wornLook.reason}`
        : wornEntry === undefined
          ? undefined
          : `${wornEntry.displayName}, ${petOrigin(wornEntry)}`

  // Focus in (the name field, above), Tab held inside, Escape closes -- from
  // anywhere now, not only while focus happened to be in the dialog.
  const box = useRef<HTMLDivElement>(null)
  // Escape closes the model picker first, then the dialog.
  useModal(box, () => {
    if (picking) {
      setPicking(false)
      return
    }
    onCancel()
  })

  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog" role="dialog" aria-modal="true" aria-label={editing ? 'Edit teammate' : 'New teammate'}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">{editing ? 'Edit teammate' : 'New teammate'}</span>
          <span className="lc-dialog__sub lc-mono">lives on this machine</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onCancel}>
            <Icon name="close" size={13} />
          </button>
        </div>

        <div className="lc-dialog__body">
          {/* First, not at the foot below the fold (a practice tester on 0.306 found it there). */}
          <p className="lc-dialog__note lc-mono">
            A teammate is a name, a face and a place to keep conversations. It grants no new access.
          </p>
          <div className="lc-dialog__identity">
            {/* The preview works, so the person sees the behaviour a live
                teammate has -- all of it, as the face in a conversation does. */}
            <TeammateBot hue={hue} avatar={avatar} size={64} activity="working" presence="working" motion="full" />
            <div className="lc-dialog__fields">
              <label className="lc-fieldlabel lc-mono" htmlFor="lc-teammate-name">
                Name
              </label>
              <input
                id="lc-teammate-name"
                ref={nameRef}
                className="lc-input"
                value={name}
                maxLength={40}
                onChange={(event) => setName(event.target.value)}
                placeholder={suggestedName(takenNames)}
                autoComplete="off"
                {...(taken === undefined ? {} : { 'aria-invalid': true, 'aria-describedby': 'lc-teammate-name-taken' })}
              />
              {taken !== undefined && (
                <p id="lc-teammate-name-taken" className="lc-dialog__error">
                  Another teammate is already called {taken}.
                </p>
              )}
              {agentMatch !== undefined && (
                <p className="lc-dialog__note">
                  {agentMatch} is also an AI agent&apos;s name. Your other teammates will be told this {agentMatch} is a teammate.
                </p>
              )}
              <div className="lc-hues" role="radiogroup" aria-label="Avatar colour">
                {HUES.map((option) => (
                  <button
                    key={option.hue}
                    type="button"
                    role="radio"
                    aria-checked={hue === option.hue}
                    aria-label={option.label}
                    className={`lc-hue${hue === option.hue ? ' is-selected' : ''}`}
                    onClick={() => setHue(option.hue)}
                  >
                    {wornPet === undefined ? (
                      <TeammateBot hue={option.hue} avatar={avatar} size={24} />
                    ) : (
                      <span className="lc-hue__chip" style={{ background: `var(--lc-hue-${option.hue})` }} />
                    )}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {/*
            * THE MODEL FIRST, under the name (0.514). A first-hour pass on
            * 0.512: Create teammate was on screen while the model sat below
            * look, role, Own branch and connectors -- so a person could make a
            * teammate on a paid model without seeing which. What it runs on is
            * the choice that matters; how it looks can wait.
            */}
          {/*
            The reference shows a default route and approval mode here. Both are
            stated as what they are today rather than as settings this dialog
            can change: route selection lands with the route layer, and approval
            modes need a write-capable sandbox to mean anything.
          */}
          <div className="lc-teammatemodel">
            <span className="lc-fieldlabel lc-mono">Model</span>
            <div className="lc-teammatemodel__row">
              <span className="lc-teammatemodel__name">
                {shownRoute?.account !== undefined && <span>{accountName(shownRoute.account)}{' \u00b7 '}</span>}
                {shownRoute === undefined ? 'The chat box\u2019s, when they start' : routeLabel(shownRoute)}
                {picked === undefined && ownRoute === undefined && shownRoute !== undefined && (
                  <span className="lc-teammatemodel__whose"> · the chat box&apos;s, until you pick one</span>
                )}
              </span>
              {picker !== undefined && (
                <button type="button" className="lc-button" aria-expanded={picking} onClick={() => setPicking((open) => !open)}>
                  {picking ? 'Cancel' : 'Change'}
                </button>
              )}
            </div>
            {picking && picker !== undefined && (
              <div className="lc-teammatemodel__picker">
                <RoutePicker
                  runtimes={picker.runtimes}
                  limitedRuntimes={picker.limitedRuntimes}
                  models={picker.models}
                  resolvedModels={picker.resolvedModels}
                  recentRoutes={picker.recentRoutes}
                  active={shownRoute === undefined ? { runtime: 'claude', model: 'account-default' } : { runtime: shownRoute.runtime, model: shownRoute.model, ...(shownRoute.account === undefined ? {} : { account: shownRoute.account }) }}
                  onSelect={pick}
                  onClose={() => setPicking(false)}
                />
              </div>
            )}
            {!picking && shownRoute !== undefined && effortBases.length > 0 && (
              <div className="lc-effortpanel lc-teammatemodel__effort" role="group" aria-label="Reasoning effort">
                <EffortSlider
                  bases={effortBases}
                  index={Math.max(0, effortBases.indexOf(effortBase))}
                  fast={effortIsFast}
                  hasFast={effortHasFast}
                  footer={effortFooter(shownRoute.runtime)}
                  onPick={(base) => chooseEffort(joinEffort(base, effortIsFast, supportedEfforts))}
                  onFast={(next) => chooseEffort(joinEffort(effortBase, next, supportedEfforts))}
                />
              </div>
            )}
            <span className="lc-teammatemodel__hint">
              Their messages, rooms and routines run on it. Picking another model in a chat with them changes it too.
            </span>
          </div>

          {/*
            * CHOOSE A LOOK. Colin, 2026-09-22, on the bots: "we could just
            * have a choose your avatar option or both, whatever you decide".
            * Both: Shuffle rolls a look, and the grid picks one outright --
            * every shape in the teammate's own colour, still; the preview
            * above is the one that moves.
            */}
          <div className="lc-dialog__section">
            <div className="lc-lookhead">
              <span className="lc-fieldlabel lc-mono">Look</span>
              {wornPet === undefined && (
                <div className="lc-lookface" role="radiogroup" aria-label="Face">
                  {(['eyes', 'mouth', 'screen'] as const).map((face) => {
                    const off = face === 'screen' ? !terminal && !alwaysScreen : alwaysScreen
                    return (
                      <button
                        key={face}
                        type="button"
                        role="radio"
                        aria-checked={faceChoice === face}
                        className={faceChoice === face ? 'is-selected' : undefined}
                        disabled={off}
                        title={
                          off
                            ? face === 'screen'
                              ? 'Turn Terminal faces on in Settings > Appearance to give a teammate a screen.'
                              : "Prompt's face is its screen."
                            : undefined
                        }
                        onClick={() =>
                          setAvatar((current) => {
                            const held = botFor(current)
                            return {
                              ...current,
                              bot: face === 'screen' ? { shape: held.shape, face: held.face, screen: true } : { shape: held.shape, face, screen: false }
                            }
                          })
                        }
                      >
                        {face === 'eyes' ? 'Eyes' : face === 'mouth' ? 'Mouth' : 'Screen'}
                      </button>
                    )
                  })}
                </div>
              )}
              {wornPet !== undefined && wornLook?.status === 'ready' && (wornLook.atlas.screen !== undefined || wornLook.atlas.puppet !== undefined) && (
                <div className="lc-lookface" role="radiogroup" aria-label="Face">
                  {(['drawn', 'screen'] as const).map((face) => {
                    const off = face === 'screen' && !terminal
                    const chosen = face === 'screen' ? terminal && wornPet.screen !== false : !terminal || wornPet.screen === false
                    return (
                      <button
                        key={face}
                        type="button"
                        role="radio"
                        aria-checked={chosen}
                        className={chosen ? 'is-selected' : undefined}
                        disabled={off}
                        title={
                          off
                            ? 'Turn Terminal faces on in Settings > Appearance to give a teammate a screen.'
                            : face === 'drawn'
                              ? 'As its maker drew it.'
                              : 'A screen whose eyes say what it is doing.'
                        }
                        onClick={() =>
                          setAvatar((current) =>
                            current.pet === undefined ? current : { ...current, pet: { source: current.pet.source, id: current.pet.id, screen: face === 'screen' } }
                          )
                        }
                      >
                        {face === 'drawn' ? 'As drawn' : 'Screen'}
                      </button>
                    )
                  })}
                </div>
              )}
              <button
                type="button"
                className="lc-ghostbutton lc-shuffle"
                onClick={() => setAvatar((current) => shuffledAvatar(current))}
              >
                Shuffle look
              </button>
            </div>
            <div className="lc-lookgrid" role="radiogroup" aria-label="Look">
              {BOT_SHAPES.map((shape) => (
                <button
                  key={shape}
                  type="button"
                  role="radio"
                  aria-checked={wornPet === undefined && look.shape === shape}
                  aria-label={SHAPE_NAMES[shape]}
                  title={SHAPE_NAMES[shape]}
                  data-shape={shape}
                  className={`lc-look${wornPet === undefined && look.shape === shape ? ' is-selected' : ''}`}
                  onClick={() => setAvatar((current) => ({ ...withoutPet(current), bot: withShape(current, shape) }))}
                >
                  <TeammateBot hue={hue} avatar={{ ...withoutPet(avatar), bot: withShape(avatar, shape) }} size={30} />
                </button>
              ))}
            </div>

            <div className="lc-pets" role="group" aria-label="Pets">
              <div className="lc-pets__head">
                <span className="lc-fieldlabel lc-mono">Pets</span>
                <PetRemovalControls removal={petRemoval} />
              </div>
              <div className="lc-lookgrid" role="radiogroup" aria-label="Pets">
                <PetPickTiles selected={wornPet} installed={pets} onWear={wearPet} onNotice={setPetNotice} removed={removedPets} removal={petRemoval} />
                {petRemoval.ticked === undefined && otherPets.map((pet) => {
                  const chosen = samePet(wornPet, pet)
                  return (
                    <button
                      key={`${pet.source}/${pet.id}`}
                      type="button"
                      role="radio"
                      aria-checked={chosen}
                      aria-label={pet.displayName}
                      title={`${pet.displayName}, ${petOrigin(pet)}`}
                      data-pet={pet.id}
                      data-source={pet.source}
                      className={`lc-look lc-pettile${chosen ? ' is-selected' : ''}`}
                      onClick={() => wearPet(pet)}
                    >
                      <TeammateBot hue={hue} avatar={{ ...avatar, pet: { source: pet.source, id: pet.id } }} size={34} />
                    </button>
                  )
                })}
              </div>
              {petCaption !== undefined && <p className="lc-pets__caption">{petCaption}</p>}
              {petNotice !== undefined && <p className="lc-pets__caption lc-tone-amber">{petNotice}</p>}
              {petRemoval.report !== undefined && <p className={`lc-pets__caption${petRemoval.report.warn ? ' lc-tone-amber' : ''}`}>{petRemoval.report.text}</p>}
              {petRemoval.ticked === undefined && removedPets.length > 0 && (
                <p className="lc-pets__caption">
                  {`${String(removedPets.length)} removed. `}
                  <button type="button" className="lc-linkbutton lc-pets__action" onClick={() => petRemoval.setShowRemoved(!petRemoval.showRemoved)}>
                    {petRemoval.showRemoved ? 'Hide removed' : 'Show removed'}
                  </button>
                </p>
              )}
              <PetCredit />
            </div>
          </div>

          <div className="lc-dialog__section">
            <span className="lc-fieldlabel lc-mono">Role</span>
            <div className="lc-rolegrid" role="radiogroup" aria-label="Role">
              {ROLES.map((option) => (
                <button
                  key={option.role}
                  type="button"
                  role="radio"
                  aria-checked={role === option.role}
                  className={`lc-rolecard${role === option.role ? ' is-selected' : ''}`}
                  onClick={() => setRole(option.role)}
                >
                  <span className="lc-rolecard__name">{option.role}</span>
                  <span className="lc-rolecard__desc">{option.description}</span>
                </button>
              ))}
            </div>
            {role === 'Custom' && (
              // The words that stand in for a role name everywhere: beside the
              // name in the sidebar, and in the brief every runtime on the
              // roster is given. Short, because it sits in both places.
              <label className="lc-field lc-field--roletitle">
                <span className="lc-fieldlabel lc-mono">What they do</span>
                <input
                  type="text"
                  value={roleTitle}
                  maxLength={60}
                  placeholder="e.g. Release manager, or Reviews every PR for security"
                  onChange={(event) => setRoleTitle(event.target.value)}
                />
                <span className="lc-field__hint">Shown beside their name, and told to their AI agent as their role.</span>
              </label>
            )}
          </div>

          {/*
            * Own branch: the teammate's missions run in its own worktree of the
            * folder's repository, so two teammates editing one repository do
            * not collide (parity row 64). Bringing the branch back is a git
            * operation of the person's; Locust merges nothing.
            */}
          <div className="lc-field lc-field--switch">
            <button
              type="button"
              role="switch"
              aria-checked={worktree}
              aria-label="Own copy"
              className={`lc-memory__switch${worktree ? ' is-on' : ''}`}
              onClick={() => setWorktree(!worktree)}
            >
              <span className="lc-memory__knob" />
            </button>
            <span className="lc-field__text">
              <span className="lc-fieldlabel lc-mono">Own copy</span>
              <span className="lc-field__hint">
                Works in its own copy of the folder, on branch {branchNameFor(name.trim().length === 0 ? 'teammate' : name)}. Needs the folder to be a git repository.
                Merging back is yours to do.
              </span>
            </span>
          </div>

          {/*
            * The folder THIS teammate stands in.
            *
            * Not the project folder switch in Settings: that one reopens
            * Locust, because history, memory and worktrees are all scoped by
            * it. This moves one teammate and closes nothing. Colin,
            * 2026-09-09: "it should only change the folder for that
            * chat/teammate not the entire app."
            *
            * Only when editing. A teammate being created has no id yet, and
            * the host writes the folder onto a record.
            */}
          {editing && onChooseFolder !== undefined && (
            <div className="lc-field lc-field--folder">
              <span className="lc-fieldlabel lc-mono">Works in</span>
              <div className="lc-folderrow">
                <span
                  className={`lc-folderrow__path lc-mono${initial?.folder === undefined ? ' is-default' : ''}`}
                  title={initial?.folder ?? 'The project folder'}
                >
                  {initial?.folder ?? 'The project folder'}
                </span>
                <button type="button" className="lc-button" onClick={() => onChooseFolder(false)}>
                  {initial?.folder === undefined ? 'Choose folder' : 'Change'}
                </button>
                {initial?.folder !== undefined && (
                  <button type="button" className="lc-button" onClick={() => onChooseFolder(true)}>
                    Use the project folder
                  </button>
                )}
              </div>
              <span className="lc-field__hint">
                Its conversations run here instead of the project folder, which is also how it reaches a connector (an MCP server) set up
                for that folder. History and memory stay with the project either way.
              </span>
              {folderNotice !== undefined && <span className="lc-field__hint lc-tone-amber">{folderNotice}</span>}
            </div>
          )}

          {/*
            * Which connectors THIS teammate may use without asking -- and, since
            * 0.798, whether it has the person's connectors at all.
            *
            * Nothing ticked means OFF: Claude Code sends every tool of every
            * connector with each call, about 170k tokens a turn on Colin's
            * account (measured 2026-10-19 on Haiku, Sonnet and Opus), so a
            * teammate carries them only once it has one ticked. Ticked ones work
            * without asking; while any is, the rest are loaded too and a call to
            * one asks first. A Finance Bro gets Robinhood and not Gmail. Every
            * name here was read off `claude mcp list`; none can be typed.
            *
            * Only when editing, like the folder: a teammate being created has
            * nothing to write a list onto.
            */}
          {editing && onSetConnectors !== undefined && connectors !== undefined && connectors.length > 0 && (
            <div className="lc-field lc-field--connectors">
              <span className="lc-fieldlabel lc-mono">Connectors</span>
              <div className="lc-connectorgrid" role="group" aria-label="Connectors this teammate may use">
                {connectors.map((connector) => {
                  const narrowed = initial?.connectors ?? []
                  const on = narrowed.includes(connector.name)
                  const label = connectorLabel(connector.name)
                  return (
                    <button
                      key={connector.name}
                      type="button"
                      role="checkbox"
                      aria-checked={on}
                      className={`lc-connectorpick${on ? ' is-on' : ''}${connector.status === 'connected' ? '' : ' is-unready'}`}
                      title={connector.status === 'connected' ? `${connector.name} — ${connector.location}` : `${connector.name} — ${connector.status === 'needs-auth' ? 'not signed in yet' : 'not responding'}`}
                      onClick={() => {
                        // A tick turns that connector on for this teammate; the last untick turns them off again.
                        onSetConnectors(on ? narrowed.filter((name) => name !== connector.name) : [...narrowed, connector.name])
                      }}
                    >
                      <span>{label}</span>
                    </button>
                  )
                })}
              </div>
              <span className="lc-field__hint">
                {(initial?.connectors ?? []).length === 0
                  ? 'Off. This teammate works without your connectors, which keeps each Claude Code turn about ten times lighter. Tick the ones it may use.'
                  : 'On. The ticked ones work without asking. While any is ticked, Claude Code loads all your connectors, and a call to one not ticked asks you first.'}
              </span>
            </div>
          )}

          <label className="lc-field lc-field--limit">
            <span className="lc-fieldlabel lc-mono">Monthly limit</span>
            {policyLimit !== undefined && <><ManagedPolicyNote /><span className="lc-field__hint">Maximum {policyLimit.toFixed(2)} USD. You can set a lower amount.</span></>}
            <span className="lc-limitinput">
              <span className="lc-limitinput__currency lc-mono" aria-hidden="true">$</span>
              <input
                type="text"
                inputMode="decimal"
                value={limitText}
                disabled={policyLimit === 0}
                placeholder={policyLimit === undefined ? 'No limit' : policyLimit.toFixed(2)}
                aria-invalid={limit === 'invalid' || overPolicyLimit}
                onChange={(event) => setLimitText(event.target.value)}
                onBlur={() => {
                  if (typeof limit === 'number') setLimitText(limit.toFixed(2))
                }}
              />
            </span>
            <span className={`lc-field__hint${limit === 'invalid' ? ' lc-tone-amber' : ''}`}>
              {limit === 'invalid'
                ? 'Enter an amount like 5 or 12.50, or leave it empty for no limit.'
                : limitHint(trimmed.length === 0 ? 'This teammate' : trimmed, spentThisMonth, editing)}
            </span>
          </label>

          <label className="lc-field lc-field--instructions">
            <span className="lc-fieldlabel lc-mono">Instructions</span>
            <textarea
              className="lc-input lc-field__text"
              aria-label="Instructions for this teammate"
              rows={3}
              maxLength={MAX_TEAMMATE_INSTRUCTIONS}
              placeholder="Answer in short bullet points. Ask before adding a dependency."
              value={instructions}
              onChange={(event) => setInstructions(event.target.value)}
            />
            <span className="lc-field__hint">
              {`Optional. Every turn ${trimmed.length === 0 ? 'this teammate' : trimmed} takes is given these, after a group's instructions.`}
            </span>
          </label>

          {editing && lessons !== undefined && lessons.length > 0 && (
            <div className="lc-field lc-field--lessons">
              <span className="lc-fieldlabel lc-mono">Tune lessons</span>
              <ul className="lc-tunelessons" aria-label={`Tune lessons for ${trimmed.length === 0 ? 'this teammate' : trimmed}`}>
                {lessons.map((lesson) => (
                  <li key={lesson.lessonId} className="lc-tunelessons__row">
                    <span className="lc-tunelessons__text">{tuneLessonLine(lesson)}</span>
                    {onRemoveLesson !== undefined && (
                      <button
                        type="button"
                        className="lc-ghostbutton"
                        onClick={() => void onRemoveLesson(lesson.lessonId)}
                      >
                        Remove
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {editing && skills !== undefined && skills.length > 0 && (
            <div className="lc-field lc-field--lessons">
              <span className="lc-fieldlabel lc-mono">Skills</span>
              <ul className="lc-tunelessons" aria-label={`Skills for ${trimmed.length === 0 ? 'this teammate' : trimmed}`}>
                {skills.map((skill) => (
                  <li key={skill.name} className="lc-tunelessons__row">
                    <span className="lc-tunelessons__text">
                      <span className="lc-mono">{skill.name}</span>
                      <span className="lc-tunelessons__when">{skill.description}</span>
                    </span>
                    {onRemoveSkill !== undefined && (
                      <button
                        type="button"
                        className="lc-ghostbutton"
                        disabled={removingSkill !== undefined}
                        onClick={() => {
                          setRemovingSkill(skill.name)
                          setSkillProblem(undefined)
                          void onRemoveSkill(skill.name)
                            .then((why) => setSkillProblem(why))
                            .catch(() => setSkillProblem('That skill could not be removed. The teammate’s skills are unchanged.'))
                            .finally(() => setRemovingSkill(undefined))
                        }}
                      >
                        Remove
                      </button>
                    )}
                  </li>
                ))}
              </ul>
              <span className="lc-field__hint">
                {shownRoute?.runtime === 'claude'
                  ? 'Kept from tunes. A run loads one only when the work matches when to use it.'
                  : 'Kept from tunes. Only a Claude Code run is handed these, so they wait until this teammate runs on Claude Code again.'}
              </span>
              {skillProblem !== undefined && <span className="lc-field__hint lc-tone-red" role="alert">{skillProblem}</span>}
            </div>
          )}

          <label className="lc-field">
            <span className="lc-fieldlabel lc-mono">Voice</span>
            <span className="lc-voicepick">
              <Select className="lc-input" aria-label="Voice for this teammate" value={voice} onChange={setVoice} options={[
                { value: '', label: 'Talk’s own voice, from Settings' },
                ...CHATGPT_VOICES.map((entry) => ({ value: entry.id, label: `${voiceName(entry.id)} — ${entry.about}` }))
              ]} />
              {/* Hear it before choosing it (0.751). */}
              <VoicePlay voice={isChatGptVoice(voice) ? voice : undefined} />
            </span>
            <span className="lc-field__hint">
              {`How ${trimmed.length === 0 ? 'this teammate' : trimmed} sounds when you press Talk in ${trimmed.length === 0 ? 'its' : `${trimmed}’s`} conversation. ChatGPT’s voices, on your ChatGPT plan; the work stays on its own AI agent.`}
            </span>
          </label>

          <div className="lc-dialog__summary">
            {/*
              * The mode the next mission will ACTUALLY run in. This said
              * "Read-only · nothing outside the workspace" as fixed copy,
              * while the composer's default is Accept edits -- so a fresh
              * profile promised read-only in the dialog and then wrote files
              * (QA pass, 2026-09-05). Same defect as the idle teammate's
              * sentence, fixed in 0.18.3; this was its second home.
              */}
            <div className="lc-summarycard">
              <span className="lc-summarycard__text">{modeSummary(mode)}</span>
              <span className="lc-summarycard__label lc-mono">APPROVALS</span>
            </div>
          </div>

          {error !== undefined && <p className="lc-dialog__error">{error}</p>}
        </div>

        <div className="lc-dialog__foot">
          <button type="button" className="lc-ghostbutton" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="lc-primarybutton"
            disabled={!canCreate || saving}
            onClick={() => {
              // L22 (the code review): a double click created the teammate
              // twice. A ref, because the second click lands before the
              // re-render that would disable the button.
              if (pressed.current) return
              pressed.current = true
              setSaving(true)
              void Promise.resolve(
                onCreate({
                  name: trimmed,
                  hue,
                  role,
                  ...(role === 'Custom' && roleTitle.trim().length > 0 ? { roleTitle: roleTitle.trim() } : {}),
                  ...(worktree ? { worktree: true } : {}),
                  avatar,
                  ...(picked === undefined ? {} : { route: picked }),
                  // An edit always says: an amount, or null to lift one. A new
                  // teammate only says when there is an amount.
                  ...(typeof limit === 'number' && limit > 0 ? { monthlyLimitUsd: limit } : limit === 0 ? {} : editing ? { monthlyLimitUsd: null } : {}),
                  // The same rule as the limit: an edit always says (null clears); a new teammate only when there are some.
                  ...(instructions.trim().length > 0 ? { instructions: instructions.trim() } : editing ? { instructions: null } : {}),
                  // The same again: an edit always says (null goes back to Talk's own); a new teammate only when one is picked.
                  ...(voice !== '' ? { voice } : editing ? { voice: null } : {})
                })
              ).finally(() => {
                pressed.current = false
                setSaving(false)
              })
            }}
          >
            {editing ? 'Save changes' : 'Create teammate'}
          </button>
        </div>
      </div>
    </div>
  )
}
