/**
 * The person's theme (0.789, main/appearance.ts): Dark, Light, or Match the computer.
 */
export type ThemeChoice = 'dark' | 'light' | 'system'

/**
 * The look (0.855, main/mac-glass.ts): Standard, or Liquid glass -- the whole window on macOS 26's glass, the sidebar
 * on a floating panel of it, the controls on glass pills. Offered only on a Mac that has the glass.
 */
export type LookChoice = 'standard' | 'liquid'

/** What the window is drawn in now: the choice, and what Match the computer comes to. */
export interface AppearanceState {
  readonly theme: ThemeChoice
  readonly resolved: 'dark' | 'light'
  /** The look chosen. A window draws Liquid glass only while `liquidOffered` is true too. */
  readonly look: LookChoice
  /** This computer has macOS's glass and this build can reach it. */
  readonly liquidOffered: boolean
}

export const APPEARANCE_GET_CHANNEL = 'appearance:get'
export const APPEARANCE_SET_CHANNEL = 'appearance:set'
export const APPEARANCE_SET_LOOK_CHANNEL = 'appearance:set-look'
/**
 * Where the sidebar's floating panel sits, measured by the page from the window's top left (0.855): the glass behind
 * the page is placed there. `null` when there is no panel to draw (the sidebar is hidden).
 */
export const GLASS_PANEL_CHANNEL = 'appearance:glass-panel'
export interface GlassPanel {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly radius: number
}
/** Sent to every window when the theme changes -- by the person, or by the computer under Match the computer. */
export const APPEARANCE_CHANGED_CHANNEL = 'appearance:changed'
/**
 * Handed to each window's preload on its command line, so the page is in the right theme from its first frame
 * rather than one IPC round trip after it.
 */
export const THEME_ARGUMENT = '--locust-theme='
/** The same for the look: `liquid` only when the window starts drawn in Liquid glass. */
export const LOOK_ARGUMENT = '--locust-look='

/** The theme a window was started in, from its own command line; dark when it says nothing readable. */
export function themeFromArguments(argv: readonly string[]): 'dark' | 'light' {
  const given = argv.find((arg) => arg.startsWith(THEME_ARGUMENT))?.slice(THEME_ARGUMENT.length)
  return given === 'light' ? 'light' : 'dark'
}

/** Whether a window started drawn in Liquid glass, from its own command line. */
export function liquidFromArguments(argv: readonly string[]): boolean {
  return argv.find((arg) => arg.startsWith(LOOK_ARGUMENT))?.slice(LOOK_ARGUMENT.length) === 'liquid'
}

/** Whether a window is drawn in Liquid glass now: chosen, and offered on this computer. */
export function drawsLiquid(state: Pick<AppearanceState, 'look' | 'liquidOffered'>): boolean {
  return state.look === 'liquid' && state.liquidOffered
}
