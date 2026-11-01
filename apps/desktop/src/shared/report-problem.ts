import type { FeedbackReport } from './ipc.js'
import { SUPPORT_ADDRESS } from './support.js'

export { SUPPORT_ADDRESS }

/**
 * WHERE FEEDBACK GOES -- one address, named here, in the host.
 *
 * Settings' Report a problem showed the log and said "this is the file to
 * send", and said nowhere where to send it (the beta handover, 2026-09-23).
 * A tester who got stuck left without a trace, and there is no telemetry, by
 * design, so what testers send is the only signal there is.
 *
 * Colin: "for bug reporting we can use what claude code does" -- its Send
 * feedback box: describe the issue, a line saying exactly what goes with it,
 * Cancel and Send. Claude Code sends to its makers' own service; Locust has
 * none. So Send builds the report and opens it on GitHub, filled in, and the
 * person sends it there (it needs a GitHub account). The destination is this
 * one constant: whether it stays GitHub issues or becomes Discord or GitHub
 * Discussions is Colin's call, and it changes this line only.
 *
 * What goes: the person's own words, the Locust version and the Windows
 * build, and -- when it was sent from a conversation -- that conversation's
 * text, the latest of it if it is long. Never the log, never a path, never
 * a file: the person attaches the log themselves, and the report says how.
 *
 * The renderer names no address here (see OPEN_LINK_CHANNEL in index.ts):
 * it hands over the words, and the host builds the address itself.
 */
// The code repository since 10/08 (Colin): its bug and idea templates and SECURITY.md live there; locust-releases
// is the downloads page. Blank issues stay on there, so the filled-in body still opens.
export const REPORT_DESTINATION = 'https://github.com/StopBanning/locust/issues/new'

/**
 * THE PRIVATE WAY (0.593, the PRD's R23). A GitHub issue is public and needs
 * a GitHub account; a report that carries a conversation, or comes from
 * someone without an account, needs an address. One constant
 * (shared/support.ts, shown in Settings > Help too), forwarded to the
 * maintainer; a placeholder there fails a test, so the box can never offer
 * an address nobody reads.
 */

/** Kept well under the length a browser and GitHub take in one address. */
export const MAX_REPORT_URL = 7_500

/**
 * A mailto body is cut by the mail client, not the browser: Outlook and the
 * Windows Mail app take about 2,000 characters of a mailto, and silently drop
 * the rest. So the email carries the words and the facts, and the
 * conversation only while it fits; "Save this report as a file" carries it
 * whole.
 */
export const MAX_MAILTO_URL = 1_900

/** The person's own words, at most this long. */
export const MAX_DESCRIPTION = 2_000

export interface ReportFacts {
  /** The running Locust's own version. */
  readonly version: string
  /** `os.release()`: "10.0.<build>" on Windows 10 and 11 alike. */
  readonly release: string
  /** `process.arch`. */
  readonly arch: string
  /** `process.platform`; absent reads as Windows, which it was until the first macOS build. */
  readonly platform?: string
}

/** The system in words, on each platform Locust runs on (the first macOS build, 2026-09-29). */
export function systemName(facts: Pick<ReportFacts, 'release' | 'arch' | 'platform'>): string {
  if (facts.platform === 'darwin') return `macOS (Darwin ${facts.release.trim()}, ${facts.arch})`
  if (facts.platform !== undefined && facts.platform !== 'win32') return `${facts.platform} ${facts.release.trim()} (${facts.arch})`
  return windowsName(facts.release, facts.arch)
}

/** Windows 10 and 11 both say "10.0"; 11 is build 22000 and later. */
export function windowsName(release: string, arch: string): string {
  const build = /^10\.0\.(\d+)/.exec(release.trim())?.[1]
  if (build === undefined) return `Windows ${release.trim()} (${arch})`
  return `${Number(build) >= 22000 ? 'Windows 11' : 'Windows 10'}, build ${build} (${arch})`
}

const FENCE = '`'.repeat(4)
const NEWLINE = String.fromCharCode(10)

function body(facts: ReportFacts, description: string, conversation: string | undefined, trimmed: boolean): string {
  const lines = [description, '', '---', `Locust ${facts.version} on ${systemName(facts)}`]
  if (conversation !== undefined && conversation.length > 0) {
    lines.push(
      '',
      '<details><summary>The conversation it was sent from' + (trimmed ? ' (its latest part)' : '') + '</summary>',
      '',
      FENCE + 'text',
      conversation,
      FENCE,
      '',
      '</details>'
    )
  }
  lines.push('', 'Nothing else was attached. To add the log: Settings > Report a problem > Show the log, then drag the file into this box.')
  return lines.join(NEWLINE)
}

function urlWith(text: string): string {
  const url = new URL(REPORT_DESTINATION)
  url.searchParams.set('body', text)
  return url.toString()
}

/** The report as a file: everything, uncut, and where to send it. */
export function reportFileText(facts: ReportFacts, report: FeedbackReport): string {
  const description = report.description.trim().slice(0, MAX_DESCRIPTION)
  const conversation = report.conversation?.trim()
  return [
    `A report for Locust's makers. Send it to ${SUPPORT_ADDRESS}, or open it as an issue at ${REPORT_DESTINATION} (public).`,
    '',
    body(facts, description, conversation !== undefined && conversation.length > 0 ? conversation : undefined, false)
  ].join(NEWLINE)
}

/** "Locust 0.593.0: <the first words>" */
export function reportSubject(facts: Pick<ReportFacts, 'version'>, description: string): string {
  const words = description.trim().replace(/\s+/g, ' ')
  const first = words.length > 60 ? `${words.slice(0, 57).trimEnd()}...` : words
  return `Locust ${facts.version}: ${first.length > 0 ? first : 'a problem'}`
}

function mailtoWith(subject: string, text: string): string {
  return `mailto:${SUPPORT_ADDRESS}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(text)}`
}

/**
 * The email, filled in. The conversation goes only while the whole address
 * fits MAX_MAILTO_URL -- its END first, as the issue does -- and when none of
 * it fits, the body says the file carries it.
 */
export function supportMailtoUrl(facts: ReportFacts, report: FeedbackReport): string {
  const description = report.description.trim().slice(0, MAX_DESCRIPTION)
  const subject = reportSubject(facts, description)
  const conversation = report.conversation?.trim()
  const whole = mailtoWith(subject, body(facts, description, conversation !== undefined && conversation.length > 0 ? conversation : undefined, false))
  if (conversation === undefined || conversation.length === 0 || whole.length <= MAX_MAILTO_URL) return whole
  let keep = conversation.length
  while (keep > 40) {
    keep = Math.floor(keep * 0.8)
    const url = mailtoWith(subject, body(facts, description, `...${conversation.slice(conversation.length - keep)}`, true))
    if (url.length <= MAX_MAILTO_URL) return url
  }
  return mailtoWith(subject, [body(facts, description, undefined, false), '', 'The conversation did not fit in an email; "Save this report as a file" in Locust carries it whole.'].join(NEWLINE))
}

/**
 * The new-report page, filled in. A long conversation keeps its END -- where
 * the trouble usually is -- cut to fit MAX_REPORT_URL once encoded.
 */
export function feedbackUrl(facts: ReportFacts, report: FeedbackReport): string {
  const description = report.description.trim().slice(0, MAX_DESCRIPTION)
  const conversation = report.conversation?.trim()
  const whole = urlWith(body(facts, description, conversation, false))
  if (conversation === undefined || whole.length <= MAX_REPORT_URL) return whole
  // Keep less of the start until it fits; the ending stays whole.
  let keep = conversation.length
  let url = whole
  while (keep > 0) {
    keep = Math.floor(keep * 0.8)
    url = urlWith(body(facts, description, `...${conversation.slice(conversation.length - keep)}`, true))
    if (url.length <= MAX_REPORT_URL) return url
  }
  return urlWith(body(facts, description, undefined, false))
}
