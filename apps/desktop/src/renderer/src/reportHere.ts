import type { FeedbackReport } from '../../shared/ipc.js'
import { feedbackUrl, reportFileText, supportMailtoUrl, type ReportFacts } from '../../shared/report-problem.js'
import { saveInDownloads } from './saveHere.js'

/**
 * SEND FEEDBACK FROM LOCUST IN A BROWSER (0.853). The computer opens its own browser, mail app or save dialog, out of
 * a browser's sight, so a browser is refused those calls. It builds the same report here instead, from the computer's
 * own facts (its version and system), and sends it this browser's way: the GitHub issue in a new tab, the email in
 * this device's mail app, the file in its downloads. Each runs inside the press, as a browser lets a tab open.
 */
export interface ReportPlace {
  readonly open: (url: string) => void
  readonly mail: (url: string) => void
  readonly download: (name: string, text: string) => void
}

export const browserPlace: ReportPlace = {
  open: (url) => { window.open(url, '_blank', 'noopener,noreferrer') },
  mail: (url) => { window.location.href = url },
  download: saveInDownloads
}

/** The file's name, as the computer's save dialog offers it: "Locust report 2026-11-01 21.40.txt". */
export function reportFileName(now: Date): string {
  return `Locust report ${now.toISOString().slice(0, 16).replace('T', ' ').replace(':', '.')}.txt`
}

/** The report sent this browser's way. For a file, the name it was downloaded as. */
export function reportHere(how: 'issue' | 'email' | 'file', facts: ReportFacts, report: FeedbackReport, place: ReportPlace = browserPlace, now = new Date()): string | undefined {
  if (how === 'issue') {
    place.open(feedbackUrl(facts, report))
    return undefined
  }
  if (how === 'email') {
    place.mail(supportMailtoUrl(facts, report))
    return undefined
  }
  const name = reportFileName(now)
  place.download(name, reportFileText(facts, report))
  return name
}
