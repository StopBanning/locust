import { scrubSecrets } from './secrets.js'

/**
 * What would leave this machine, said only where it can be known.
 *
 * The approval card answers what would happen, where, and whether it can be
 * undone. It never said what DATA the action sends — which the design pass
 * names as the row it is missing, and which is the one question a person
 * cannot work out for themselves. A file write they can reason about; whether
 * a shell command phones somewhere they cannot, because the command may be a
 * script whose contents are not on screen.
 *
 * So the rule here is asymmetric, deliberately:
 *
 *   - A file change sends NOTHING, and that is provable. The bytes are written
 *     to a path on this disk. Say so plainly.
 *   - A command that names a network tool CAN reach the network. Say that it
 *     can, and say that Locust cannot see what it would send — because it
 *     cannot.
 *   - Any other command is UNKNOWN. Never say "nothing leaves this machine"
 *     about a command, however harmless it looks: `./deploy.sh` is two words
 *     and can do anything.
 *
 * The dangerous direction is the false negative — telling someone nothing is
 * sent when something is. So the negative is only ever claimed for the one
 * kind where it is a fact, and the positive is a capability ("can reach"),
 * never a prediction ("will send").
 */

/**
 * Programs that reach the network as their ordinary purpose. Matched as whole
 * words so `curling` and a file called `wget-notes.md` do not trip it.
 *
 * Under-matching here is safe by construction: an unmatched command falls to
 * "Locust cannot tell", which is the honest answer anyway. Over-matching would
 * be noise, not danger.
 */
const NETWORK_TOOLS = [
  'curl',
  'wget',
  'ssh',
  'scp',
  'sftp',
  'rsync',
  'nc',
  'netcat',
  'telnet',
  'ftp',
  'npm',
  'pnpm',
  'yarn',
  'pip',
  'pip3',
  'cargo',
  'gem',
  'go',
  'brew',
  'apt',
  'docker',
  'kubectl',
  'aws',
  'gcloud',
  'az',
  'gh',
  'git'
]

const TOOL_PATTERN = new RegExp(`(^|[\\s|&;(<>])(${NETWORK_TOOLS.join('|')})([\\s]|$)`, 'i')
/** A bare URL in the command is as good as naming a tool. */
const URL_PATTERN = /\bhttps?:\/\//i

/** Whether this command text names something that reaches the network. */
export function commandReachesNetwork(command: string): boolean {
  return TOOL_PATTERN.test(command) || URL_PATTERN.test(command)
}

/**
 * The `Data sent` row's text, or undefined when the row should not be drawn.
 *
 * `undefined` for a question: answering sends nothing anywhere, and a row
 * saying so on a card that is only asking would be noise on the one surface
 * that must stay short.
 */
export function dataSentLine(kind: string, command: string): string | undefined {
  if (kind === 'file-change') {
    return 'Nothing. The change is written to this machine and sent nowhere.'
  }
  // A connector is the one kind whose whole point is to leave this machine.
  // The input on the card IS what goes; the service it goes to is named in
  // the summary. No sandbox here reaches the far end of a connector, which
  // is why the mode never governed one and why this card exists.
  if (kind === 'connector') {
    return 'The input above, to the service the connector reaches. It acts there, not on this machine.'
  }
  if (kind !== 'command') return undefined
  return commandReachesNetwork(command)
    ? 'This command can reach the network. Locust cannot see what it would send — read it above.'
    : 'Unknown. Locust cannot tell what a command sends; a script can do anything the sandbox allows.'
}

/**
 * ONE ROW FOR A COMMAND LOCUST CANNOT READ (0.735).
 *
 * An ordinary command's card said "Unknown" twice -- under Data sent and
 * again under Reversible, each ending "anything the sandbox allows" -- on the
 * card a person sees most. When neither row knows anything, they are one row
 * saying both; whenever either knows something (a network tool, a route's own
 * words), both rows are drawn as before.
 */
export function commandEffectsUnknown(request: { readonly kind: string; readonly detail: string; readonly dataSentSays?: string; readonly reversibleSays?: string }): boolean {
  return request.kind === 'command' && request.dataSentSays === undefined && request.reversibleSays === undefined && !commandReachesNetwork(request.detail)
}

/** What a connector does cannot be undone from here: said on every card that asks for one (0.749, a call's card too). */
export const CONNECTOR_NOT_UNDOABLE = 'Unknown — a connector acts on the service it reaches, and Locust cannot undo what happens there.'

/**
 * THE EXACT ACTION, ON A SMALL SCREEN (0.837 in the chat apps, 0.840 on the phone). A card's summary is the kind of
 * action -- "Change 1 file", "Run a command" -- and its exact file or command is the detail, so a card answered away
 * from the window said neither. The detail's first line goes with it, cut short, anything shaped like a key taken
 * out; undefined when the summary already says it.
 */
export function exactLine(request: { readonly summary: string; readonly detail: string }): string | undefined {
  const exact = typeof request.detail === 'string' ? request.detail.split('\n').find((line) => line.trim().length > 0)?.trim() : undefined
  if (exact === undefined || request.summary.includes(exact)) return undefined
  return scrubSecrets(exact.length > 200 ? `${exact.slice(0, 200)}…` : exact).text
}

export const COMMAND_EFFECTS_UNKNOWN = 'Unknown. Locust cannot tell what a command sends or undo what it does; it can do anything the sandbox allows.'

/**
 * A FILE OUTSIDE THE PROJECT FOLDER (QA-2026-09-29 round 2, R15).
 *
 * Absolute and not under the folder, or relative and climbing out of it.
 * Compared the way Windows compares paths: separators either way, case aside.
 */
export function outsideFolder(file: string, folder: string): boolean {
  if (folder.length === 0) return false
  const normal = (value: string): string => value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
  const path = normal(file)
  if (!/^(?:[a-z]:)?\//.test(path)) return path.split('/').includes('..')
  const root = normal(folder)
  return path !== root && !path.startsWith(`${root}/`)
}

export const OUTSIDE_NOT_UNDOABLE = 'Not from here — it is outside your project folder, so the project’s version control does not hold it.'
