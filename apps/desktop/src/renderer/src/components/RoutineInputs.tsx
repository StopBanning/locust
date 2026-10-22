import { useRef, useState } from 'react'
import type { ReactElement } from 'react'
import type { RoutineInput, RoutineInputKind, RoutineValues } from '../../../shared/routine-inputs.js'
import { MAX_INPUTS, MAX_INPUT_VALUE, resolveValues } from '../../../shared/routine-inputs.js'
import type { PublicRoutine } from '../../../shared/ipc.js'
import { useModal } from '../useModal.js'
import { Select } from './Select.js'

/** W7: declarations are edited with the steps; defaults are plain text, never folder paths. */
export function RoutineInputEditor({ inputs, onChange }: { readonly inputs: readonly RoutineInput[]; readonly onChange: (inputs: readonly RoutineInput[]) => void }): ReactElement {
  const change = (at: number, value: RoutineInput): void => onChange(inputs.map((held, index) => index === at ? value : held))
  return <div className="lc-dialog__section lc-routineinputs">
    <span className="lc-fieldlabel lc-mono">Inputs</span>
    <p className="lc-dialog__note">Write {'{{key}}'} in a step where an answer belongs. Automatic runs use defaults; required answers without defaults must be entered by hand.</p>
    {inputs.map((input, at) => <fieldset className="lc-routineinputs__entry" key={at}>
      <legend>Input {at + 1}</legend>
      <label>Key<input className="lc-input" aria-label={`Input ${at + 1} key`} maxLength={32} value={input.key} onChange={(event) => change(at, { ...input, key: event.target.value })} /></label>
      <label>Label<input className="lc-input" aria-label={`Input ${at + 1} label`} maxLength={80} value={input.label} onChange={(event) => change(at, { ...input, label: event.target.value })} /></label>
      <label>Kind<Select className="lc-input" aria-label={`Input ${at + 1} kind`} value={input.kind} onChange={(next) => {
        const kind = next as RoutineInputKind
        const { default: _default, choices: _choices, ...base } = input
        change(at, { ...base, kind, ...(kind === 'choice' ? { choices: ['First', 'Second'] } : {}), ...(kind === 'folder' || input.kind === 'choice' || kind === 'choice' || input.default === undefined ? {} : { default: input.default }) })
      }} options={[{ value: 'text', label: 'Text' }, { value: 'long-text', label: 'Long text' }, { value: 'choice', label: 'Choice' }, { value: 'folder', label: 'Folder' }]} /></label>
      <label className="lc-routineinputs__check"><input type="checkbox" checked={input.required} onChange={(event) => change(at, { ...input, required: event.target.checked })} /> Required</label>
      {input.kind === 'choice' && <label>Choices, one per line<textarea className="lc-input" rows={4} aria-label={`Input ${at + 1} choices`} value={(input.choices ?? []).join('\n')} onChange={(event) => change(at, { ...input, choices: event.target.value.split('\n') })} /></label>}
      {input.kind !== 'folder' && <label>Default{input.kind === 'choice'
        ? <Select className="lc-input" aria-label={`Input ${at + 1} default`} value={input.default ?? ''} onChange={(next) => change(at, { ...input, default: next })} options={[{ value: '', label: 'No default' }, ...(input.choices ?? []).map((choice) => ({ value: choice, label: choice }))]} />
        : input.kind === 'long-text' ? <textarea className="lc-input" rows={3} aria-label={`Input ${at + 1} default`} maxLength={MAX_INPUT_VALUE} value={input.default ?? ''} onChange={(event) => change(at, { ...input, default: event.target.value })} />
        : <input className="lc-input" aria-label={`Input ${at + 1} default`} maxLength={MAX_INPUT_VALUE} value={input.default ?? ''} onChange={(event) => change(at, { ...input, default: event.target.value })} />}</label>}
      {input.kind === 'folder' && <p className="lc-dialog__note">Chosen with the folder picker when you run; no default.</p>}
      <button type="button" className="lc-ghostbutton" onClick={() => onChange(inputs.filter((_, index) => index !== at))}>Remove input {at + 1}</button>
    </fieldset>)}
    <button type="button" className="lc-ghostbutton" disabled={inputs.length >= MAX_INPUTS} onClick={() => {
      let number = inputs.length + 1
      while (inputs.some((input) => input.key === `input_${number}`)) number += 1
      onChange([...inputs, { key: `input_${number}`, label: '', kind: 'text', required: true }])
    }}>Add input</button>
  </div>
}

export function RoutineRunDialog({ routine, onRun, onCancel }: { readonly routine: PublicRoutine; readonly onRun: (values: RoutineValues) => Promise<void>; readonly onCancel: () => void }): ReactElement {
  const inputs = routine.inputs ?? []
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(inputs.map((input) => [input.key, input.default ?? ''])))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)
  const settled = resolveValues(inputs, values)
  return <div className="lc-scrim"><div ref={box} className="lc-dialog" role="dialog" aria-modal="true" aria-label="Run routine">
    <div className="lc-dialog__head"><span className="lc-dialog__title">Run {routine.name}</span><button className="lc-dialog__close" aria-label="Close" onClick={onCancel}>×</button></div>
    <div className="lc-dialog__body lc-routineinputs">
      {inputs.map((input) => <label key={input.key}>{input.label}{input.required ? ' (required)' : ''}
        {input.kind === 'folder' ? <><output>{values[input.key] || 'No folder chosen'}</output><button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => {
          void window.desktop?.chooseRoutineFolder().then((answer) => {
            if (answer.ok) { setError(undefined); setValues((held) => ({ ...held, [input.key]: answer.data.path })) }
            else if (answer.error.code !== 'CANCELLED') setError(answer.error.message)
          }).catch(() => setError('The folder picker could not be opened. Choose the folder again before running.'))
        }}>Choose {input.label}</button></>
          : input.kind === 'choice' ? <Select className="lc-input" aria-label={`Value for ${input.label}`} value={values[input.key]} onChange={(next) => setValues({ ...values, [input.key]: next })} options={[{ value: '', label: 'Choose an answer' }, ...(input.choices ?? []).map((choice) => ({ value: choice, label: choice }))]} />
          : input.kind === 'long-text' ? <textarea className="lc-input" rows={3} aria-label={`Value for ${input.label}`} maxLength={MAX_INPUT_VALUE} value={values[input.key]} onChange={(event) => setValues({ ...values, [input.key]: event.target.value })} />
          : <input className="lc-input" aria-label={`Value for ${input.label}`} maxLength={MAX_INPUT_VALUE} value={values[input.key]} onChange={(event) => setValues({ ...values, [input.key]: event.target.value })} />}
      </label>)}
      {error !== undefined && <p className="lc-dialog__error">{error}</p>}
      {!settled.ok && <p className="lc-dialog__note">{settled.message}</p>}
    </div>
    <div className="lc-dialog__foot"><button className="lc-ghostbutton" onClick={onCancel}>Cancel</button><button className="lc-primarybutton" disabled={busy || !settled.ok} onClick={() => {
      setBusy(true); setError(undefined)
      void onRun(values).then(onCancel).catch((failure: unknown) => { setBusy(false); setError(failure instanceof Error ? failure.message : 'The routine start could not be confirmed. Check its conversation before starting it again.') })
    }}>{busy ? 'Starting…' : 'Run routine'}</button></div>
  </div></div>
}
