/**
 * The free model a new Kilo run starts on.
 *
 * The same id as `KILO_AUTO_FREE_MODEL` in the runtime adapters. It lives
 * here so the window does not import the run adapter. A test checks the two
 * stay the same string.
 */
export const KILO_AUTO_FREE_MODEL = 'kilo/kilo-auto/free'

/**
 * What a Kilo row says about cost.
 *
 * Auto Free's data use is Kilo's own warning:
 * https://kilo.ai/docs/getting-started/using-kilo-for-free
 * "Auto Free may route your requests to providers that log prompts and outputs
 * and use them to improve their services. Do not submit personal or confidential
 * data when using Auto Free."
 * That page states the warning for Auto Free. It is not copied onto every
 * gateway model, and not onto other free-labeled models the page does not
 * describe that way. A model is called free only when its id ends in free
 * the way Kilo labels one (`/free`, `:free`, `-free`).
 */
export function kiloModelDescription(id: string): string {
  if (id === KILO_AUTO_FREE_MODEL) {
    return 'Free · no sign-in. Auto Free may route requests to providers that log prompts and outputs and use them to improve their services. Do not submit personal or confidential data.'
  }
  if (/(?:^|[-/:])free$/i.test(id)) return 'Free · no sign-in'
  return 'Through Kilo. A paid model needs a sign-in.'
}

/** A new Kilo conversation sits on Auto Free once the catalogue lists it. */
export function kiloStartModel(runtime: string, model: string, listed: readonly { readonly runtime: string; readonly id: string }[]): string | undefined {
  if (runtime !== 'kilo' || model !== 'account-default') return undefined
  return listed.some((entry) => entry.runtime === 'kilo' && entry.id === KILO_AUTO_FREE_MODEL) ? KILO_AUTO_FREE_MODEL : undefined
}
