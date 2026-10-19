import { Icon } from './Icon.js'

export function TerminalToggle({ open, refusal, onToggle }: { readonly open: boolean; readonly refusal?: string; readonly onToggle: () => void }) {
  return <button type="button" className={`lc-button${open ? ' is-active' : ''}`} aria-label="Terminal" aria-pressed={open}
    aria-keyshortcuts="Control+`" disabled={refusal !== undefined} title={refusal ?? 'Terminal (Ctrl+`)'} onClick={onToggle}>
    <Icon name="code" size={13} />{refusal === undefined ? 'Terminal' : 'Terminal · Set by your organization'}
  </button>
}
