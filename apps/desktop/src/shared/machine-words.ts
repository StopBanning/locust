/**
 * HOW THIS COMPUTER NAMES THINGS (the first Mac run, 2026-10-30).
 *
 * A Mac showed "the tray" and "sign in to Windows", words from the only machine Locust had run on. A closed Locust
 * keeps running in Windows' tray and in a Mac's menu bar; it starts when a person signs in to Windows, and when they
 * log in to a Mac. Anything that is not a Mac keeps the words it always had.
 */
export interface MachineWords {
  /** Where a closed Locust keeps running. */
  readonly tray: string
  /** Starting with the computer, as its own settings say it. */
  readonly signIn: string
  /** What the person calls this computer: "this PC", "this Mac" (F8, the first Mac run). */
  readonly computer: string
  /** The verb alone, for "Start Locust when you sign in" (F19). */
  readonly logIn: string
}

export const machineWords = (platform: string | undefined): MachineWords =>
  platform === 'darwin'
    ? { tray: 'menu bar', signIn: 'log in to this Mac', computer: 'Mac', logIn: 'log in' }
    : { tray: 'tray', signIn: 'sign in to Windows', computer: 'PC', logIn: 'sign in' }
