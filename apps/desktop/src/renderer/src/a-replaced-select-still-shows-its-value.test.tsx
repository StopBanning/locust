import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicRoutine, PublicTeammate, RoutineImportPreview } from '../../shared/ipc.js'
import { DEFAULT_SCOUT_SETTINGS } from '../../shared/scout.js'
import type { ScoutApi, ScoutState } from '../../shared/scout.js'
import { CompareView } from './components/CompareView.js'
import type { CompareColumnView } from './components/CompareView.js'
import { NewTeammateDialog } from './components/NewTeammateDialog.js'
import { RoutineDialog } from './components/RoutineDialog.js'
import { RoutineImportDialog } from './components/RoutineFileDialog.js'
import { RoutineInputEditor, RoutineRunDialog } from './components/RoutineInputs.js'
import { ScoutSetting } from './components/ScoutSetting.js'
import { TalkVoice } from './components/TalkVoice.js'
import { VoiceSettings } from './components/VoiceSettings.js'
import type { ChatGptVoice, VoiceCallApi } from '../../shared/voice-call.js'

/**
 * EACH REPLACED CONTROL STILL SHOWS THE VALUE IT HAD.
 *
 * The operating system's list is gone. The words, the chosen row and the
 * value behind it are the ones the control had before.
 */
const nothing = (): void => undefined
const mate = (teammateId: string, name: string): PublicTeammate =>
  ({ teammateId, name, hue: 'blue', avatar: seedAvatar(teammateId), role: 'Code & Migrations', createdAt: '2026-10-18T00:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/free', mode: 'accept-edits' } }) as PublicTeammate

function valueOf(html: string, label: string): string {
  const at = html.indexOf(`aria-label="${label}"`)
  expect(at, label).toBeGreaterThan(-1)
  const shown = /lc-select__value">([^<]*)</.exec(html.slice(at, at + 700))
  return shown?.[1] ?? ''
}

function chosen(html: string, label: string): string {
  const at = html.indexOf(`aria-label="${label}"`)
  const slice = html.slice(at, at + 4000)
  const marked = /aria-selected="true"[^>]*data-value="([^"]*)"/.exec(slice)
  return marked?.[1] ?? ''
}

describe('a replaced select still shows its value', () => {
  it('shows the routine editor’s fixes, hours, hand-off and inputs', () => {
    const html = renderToStaticMarkup(
      <RoutineDialog
        teammate={mate('tm_atlas', 'Atlas')}
        initialName="Notes"
        initialSteps={['Find it.', 'Write about {{tone}}']}
        initialSchedule={{ kind: 'every', hours: 4 }}
        initialGoalTries={3}
        initialHandOffs={[{}, { teammateId: 'tm_wren' }]}
        initialInputs={[{ key: 'tone', label: 'Tone', kind: 'choice', required: false, choices: ['Brief', 'Detailed'], default: 'Brief' }]}
        team={[mate('tm_atlas', 'Atlas'), mate('tm_wren', 'Wren')]}
        modeName="Edit"
        modeReadsOnly={false}
        truncated={false}
        routeLabel={undefined}
        busy={false}
        error={undefined}
        onSave={nothing}
        onCancel={nothing}
      />
    )
    expect(valueOf(html, 'Fixes at most')).toBe('3')
    expect(chosen(html, 'Fixes at most')).toBe('3')
    expect(valueOf(html, 'Hours between runs')).toBe('4 hours')
    expect(chosen(html, 'Hours between runs')).toBe('4')
    expect(valueOf(html, 'Who takes step 2')).toBe('Wren')
    expect(chosen(html, 'Who takes step 2')).toBe('tm_wren')
    expect(valueOf(html, 'Input 1 kind')).toBe('Choice')
    expect(chosen(html, 'Input 1 kind')).toBe('choice')
    expect(valueOf(html, 'Input 1 default')).toBe('Brief')
    expect(chosen(html, 'Input 1 default')).toBe('Brief')
  })

  it('asks who runs a conversation nobody owns, and starts on nobody', () => {
    const html = renderToStaticMarkup(
      <RoutineDialog
        teammate={undefined}
        chooseFrom={[mate('tm_wren', 'Wren')]}
        initialName="Nightly tidy"
        initialSteps={['Tidy the notes.']}
        initialSchedule={undefined}
        truncated={false}
        routeLabel="OpenCode / Free"
        busy={false}
        error={undefined}
        onSave={nothing}
        onCancel={nothing}
      />
    )
    const at = html.indexOf('id="routine-runner"')
    const slice = html.slice(at, at + 1500)
    expect(slice).toContain('lc-select__value">Pick a teammate<')
    expect(slice).toMatch(/aria-selected="true"[^>]*data-value=""/)
    expect(slice).not.toContain('lc-select__value">Wren<')
  })

  it('fills a choice when a routine is run, and asks who an import is given to', () => {
    const routine = {
      routineId: 'rt_one', name: 'Notes', teammateId: 'tm_one', route: { runtime: 'opencode', model: 'free', mode: 'ask' },
      steps: ['Write {{tone}}'], learnedFrom: [], createdAt: '2026-10-18T00:00:00.000Z', runs: 0,
      inputs: [{ key: 'tone', label: 'Tone', kind: 'choice', required: false, choices: ['Brief', 'Detailed'], default: 'Brief' }]
    } as PublicRoutine
    const run = renderToStaticMarkup(<RoutineRunDialog routine={routine} onRun={async () => undefined} onCancel={nothing} />)
    expect(valueOf(run, 'Value for Tone')).toBe('Brief')
    expect(chosen(run, 'Value for Tone')).toBe('Brief')
    const preview = { token: 'token', name: 'Notes', steps: ['Write'], inputs: [], handOffRoles: [], handOffChecks: [], connectors: [] } as RoutineImportPreview
    const imported = renderToStaticMarkup(<RoutineImportDialog preview={preview} team={[mate('tm_wren', 'Wren')]} onImport={async () => undefined} onCancel={nothing} />)
    expect(valueOf(imported, 'Give routine to')).toBe('Choose a teammate')
    expect(chosen(imported, 'Give routine to')).toBe('')
  })

  it('shows an input’s kind and default on their own', () => {
    const html = renderToStaticMarkup(
      <RoutineInputEditor
        inputs={[{ key: 'tone', label: 'Tone', kind: 'text', required: true, default: 'Daily notes' }]}
        onChange={nothing}
      />
    )
    expect(valueOf(html, 'Input 1 kind')).toBe('Text')
    expect(chosen(html, 'Input 1 kind')).toBe('text')
    expect(html).toContain('value="Daily notes"')
    expect(html).not.toMatch(/aria-label="Input 1 default"[^>]*role="combobox"/)
    // Required sits beside its box (0.820): the label lays them out in a row, not the box alone over the word.
    expect(html).toContain('<label class="lc-routineinputs__check"><input type="checkbox" checked=""/> Required</label>')
    // And a choice's list shows four lines before it scrolls: two choices read whole.
    const choice = renderToStaticMarkup(<RoutineInputEditor inputs={[{ key: 'style', label: 'Style', kind: 'choice', required: true, choices: ['Brief', 'Detailed'] }]} onChange={nothing} />)
    expect(choice).toMatch(/<textarea class="lc-input" rows="4" aria-label="Input 1 choices">/)
  })

  it('shows the voice, the talk voice, and how often Scout looks', () => {
    const teammate = renderToStaticMarkup(<NewTeammateDialog error={undefined} mode="accept-edits" onCancel={nothing} onCreate={nothing} />)
    expect(valueOf(teammate, 'Voice for this teammate')).toBe('Talk’s own voice, from Settings')
    expect(chosen(teammate, 'Voice for this teammate')).toBe('')
    const voice = renderToStaticMarkup(<VoiceSettings />)
    expect(valueOf(voice, 'Voice typing model')).toBe('Fast — 41 MB')
    expect(chosen(voice, 'Voice typing model')).toBe('fast')
    const talk = renderToStaticMarkup(<TalkVoice api={{ voice: async () => 'sol' as const, setVoice: async (next: ChatGptVoice) => next, onEvent: () => () => undefined } as unknown as VoiceCallApi} platform="win32" />)
    expect(talk).toContain('aria-label="Talk’s voice"')
    expect(talk).toContain('disabled=""')
    const scout = renderToStaticMarkup(
      <ScoutSetting
        heading="Scout keeps watch"
        state={{ settings: { ...DEFAULT_SCOUT_SETTINGS, on: true, everyMinutes: 60 }, cards: [], lookIds: [] } as ScoutState}
        api={{ state: async () => undefined, turnOn: async () => undefined, set: async () => undefined, dismiss: async () => undefined, look: async () => undefined, onChanged: () => () => undefined } as unknown as ScoutApi}
      />
    )
    expect(valueOf(scout, 'How often Scout looks')).toBe('Every hour')
    expect(chosen(scout, 'How often Scout looks')).toBe('60')
  })

  it('shows the judge and the model to ask, under the agent’s name', () => {
    const columns: CompareColumnView[] = (['a', 'b'] as const).map((slot) => ({ slot, name: slot, runtime: 'opencode', runtimeName: 'OpenCode', turns: [], running: false, keepable: true, retryable: false, answer: 'Answer', state: 'done' }))
    const html = renderToStaticMarkup(
      <CompareView
        compare={{ compareId: 'cmp_test', prompt: 'Question', createdAt: '2026-10-18T00:00:00.000Z', slots: [] }}
        changeLines={{}}
        prompts={[]}
        columns={columns}
        owner={undefined}
        workspacePath={undefined}
        keeping={false}
        retrying={undefined}
        onKeep={nothing}
        onRetry={nothing}
        onBack={undefined}
        onJudge={nothing}
        onAddModel={nothing}
        judgeChoices={[{ key: 'free', label: 'Free judge' }, { key: 'other', label: 'Another judge', group: 'OpenCode' }]}
        addChoices={[{ key: 'free', label: 'Free judge' }, { key: 'more', label: 'One more', group: 'OpenCode' }]}
      />
    )
    expect(valueOf(html, 'The model that judges')).toBe('Free judge')
    expect(chosen(html, 'The model that judges')).toBe('free')
    expect(html).toContain('role="group" aria-label="OpenCode"')
    expect(valueOf(html, 'The model to ask too')).toBe('Free judge')
    expect(chosen(html, 'The model to ask too')).toBe('free')
  })
})
