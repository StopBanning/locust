import { isMissionRuntime } from '../../shared/runtimes.js'
import { modelDisplayName, routeChrome } from './routeName.js'

/**
 * What hovering a teammate tells you.
 *
 * Colin, 2026-09-07: "make sure that if we hover over the teammates we can see
 * their role and name" -- "and the model they are using". The tooltip said
 * only "Message Wren", which is the least interesting of the four facts, and
 * in the compact avatar rail it was the ONLY one available: the name, role and
 * route are all drawn as text, and the rail hides text.
 *
 * So a rail avatar was a coloured square with no way to find out whose it was
 * without widening the window.
 *
 * Order is name, role, route -- what identifies them, then what they do, then
 * what they run on. A teammate that has never run has no route, and the line
 * simply ends: a tooltip claiming a model that nothing has resolved yet would
 * be inventing the one fact people open this app to compare.
 */
export function teammateTooltip(teammate: {
  readonly name: string
  readonly role: string
  readonly route?: { readonly runtime: string; readonly model: string }
}): string {
  // Leads with the ACTION, then who they are. The hover has to answer both
  // "what happens if I click this" and "whose avatar is this" -- the second
  // only became urgent with the compact rail, where the name is not drawn at
  // all, and the first was the only thing it used to say.
  const parts = [`Message ${teammate.name}`, teammate.role]
  if (teammate.route !== undefined) {
    // A runtime id this build does not know reads as itself rather than
    // being forced through a lookup that would refuse it.
    // The route in the chip's words (0.848): it said the id, "OpenCode / opencode/nemotron-3.5-lightning-free".
    const runtime = teammate.route.runtime
    parts.push(isMissionRuntime(runtime) ? routeChrome(runtime, teammate.route.model, modelDisplayName(runtime, teammate.route.model)) : `${runtime} / ${teammate.route.model}`)
  }
  return parts.join(' · ')
}
