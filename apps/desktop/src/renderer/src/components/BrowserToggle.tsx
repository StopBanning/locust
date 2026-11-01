import { WINDOW_ONLY_SAYS } from '../../../shared/desk-calls.js'
import { Icon } from './Icon.js'

/**
 * The browser beside the conversation, as an icon in the header's row (0.797): Claude Code's header is a row of
 * icons with their names on hover, and Locust's had grown into a row of labelled buttons (Colin, 2026-10-19: "lets
 * lean into something closer to what claude code has, buttons are getting a little too much").
 */
export function BrowserToggle({
  open,
  refusal,
  onToggle
}: {
  readonly open: boolean
  readonly refusal?: string
  readonly onToggle: () => void
}) {
  return (
    <button
      type="button"
      className={`lc-headicon${open ? ' is-active' : ''}`}
      aria-label={refusal === undefined ? 'Browser' : refusal === WINDOW_ONLY_SAYS ? 'Browser, on your computer' : 'Browser, set by your organization'}
      aria-pressed={open}
      disabled={refusal !== undefined}
      title={refusal ?? 'Browser'}
      onClick={onToggle}
    >
      <Icon name="globe" size={15} />
    </button>
  )
}
