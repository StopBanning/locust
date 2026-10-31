import { POPULAR_SKILLS } from './popular-skills.js'
import { runtimeInstallFacts } from './runtime-install.js'

/**
 * Every address this app is allowed to open in a browser, named here.
 *
 * The host denies `window.open` outright and cancels any navigation away
 * from the app's own URL, because `shell.openExternal` hands a URL to the
 * operating system where none of the packaged build's egress rules apply --
 * anything running in the renderer could otherwise have posted the mission
 * ledger to a host of its choosing, one browser launch at a time.
 *
 * The note beside that policy in `index.ts` said "Nothing in this shell
 * links out, so nothing is opened", and asked that a feature which needs a
 * link name the exact URL in the host. That stopped being true and nobody
 * did: the first-run panel grew `<a target="_blank">` links for the two
 * runtimes that are not npm packages, and one for Node.js itself. Every one
 * of them was dead, silently, in every build that shipped them -- reported
 * by the first outside tester on 0.55.0, who had no Node and whose only
 * offered way out was the link that did nothing.
 *
 * So: this list, checked in the host. The renderer asks for a URL and the
 * host opens it only if it is one of these exactly. A renderer that has been
 * turned against the person can reach these three addresses and nowhere
 * else, which is the same guarantee the deny-everything policy gave.
 *
 * The runtime addresses are read from `runtime-install.ts` rather than
 * copied, so a vendor URL that changes there cannot leave a dead link here.
 */
const RUNTIME_LINKS: readonly string[] = ['cursor', 'antigravity', 'codex', 'claude', 'copilot', 'opencode', 'gemini', 'muse', 'kilo']
  .map((id) => runtimeInstallFacts(id))
  .flatMap((facts) => (facts !== undefined && facts.install.kind === 'vendor' ? [facts.install.url] : []))

/** Node.js itself: what four of the five runtimes install through. */
const NODE_LINK = 'https://nodejs.org'

/**
 * The pet gallery's home, and where its terms send a report about a pet
 * (0.563): "Rights remain with their respective owners", contact
 * admin@openpets.dev. A fixed address, like every other here.
 */
export const OPENPETS_LINK = 'https://openpets.dev'
export const OPENPETS_REPORT_LINK = 'mailto:admin@openpets.dev?subject=A%20pet%20on%20openpets.dev'

/** Every connection Locust itself makes, listed (0.618, the PRD's R22): docs/NETWORK.md in the public copy. */
export const NETWORK_DOC_LINK = 'https://github.com/StopBanning/locust/blob/main/docs/NETWORK.md'

export const OUTBOUND_LINKS: readonly string[] = [...new Set([...RUNTIME_LINKS, NODE_LINK, OPENPETS_LINK, OPENPETS_REPORT_LINK, NETWORK_DOC_LINK, ...POPULAR_SKILLS.map((pack) => `https://github.com/${pack.source}`)])]

/** Whether the host may open this, asked of the host's own list. */
export function isOutboundLink(url: unknown): url is string {
  return typeof url === 'string' && OUTBOUND_LINKS.includes(url)
}

/**
 * A plain web address, which is a DIFFERENT and weaker guarantee than the
 * list above, and the difference is the point.
 *
 * The list is what the APP links to: three addresses, fixed, chosen here. This
 * is what a MODEL wrote in a reply, and there is no allowlist that can cover
 * that -- the whole value of a link in a reply is that it goes somewhere
 * nobody anticipated.
 *
 * Colin asked for it twice, 2026-09-14: "do we have clickable links yet" and
 * "source links are hoverable but not clickable". The old answer was a
 * deliberate no, and the note explaining it said "what it asked for is what a
 * prompt injection would ask for". That was the right instinct about the wrong
 * risk. A reply is rendered as TEXT, never as markup, so a model cannot
 * execute anything here; what it can do is write a label that disagrees with
 * its target and hope for a click. The answer to that is not to break every
 * honest citation -- it is to show where the link actually goes, which is
 * what `linkHost` is for.
 *
 * Narrow on purpose:
 *   - http and https ONLY. No `file:`, no `vscode:`, no custom scheme -- the
 *     original worry was a reply becoming a way to reach the machine, and
 *     that worry survives intact.
 *   - It must parse as a URL with a real host.
 *   - The person still has to click it, and the browser it opens in is
 *     theirs, outside this app entirely.
 */
export function isWebLink(url: unknown): url is string {
  if (typeof url !== "string" || url.length > 2048) return false;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  return (parsed.protocol === "https:" || parsed.protocol === "http:") && parsed.hostname.length > 0;
}

/**
 * The host, for showing beside a link whose label is the model's own words.
 *
 * A citation reading "his essay" that goes to somewhere unrelated is the only
 * real hazard in making these clickable, and it is entirely solved by saying
 * where it goes before the person decides. Undefined when it is not a web
 * link, in which case nothing should be clickable anyway.
 */
export function linkHost(url: string): string | undefined {
  if (!isWebLink(url)) return undefined;
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return undefined;
  }
}
