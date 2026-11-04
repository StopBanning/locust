import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'

import type { GlassGround, LookChoice, ThemeChoice } from '../shared/appearance.js'

/**
 * LIGHT, DARK, OR THE COMPUTER'S (0.789).
 *
 * Locust was drawn dark and forced the window dark (`nativeTheme.themeSource`), so a person who works in a light
 * room, or whose whole desktop is light, had one bright thing they could not change. Now the choice is theirs, in
 * Settings > General: Dark, which stays the default so nobody's Locust changes under them; Light; or Match the
 * computer, which follows the system and changes when it does.
 *
 * Kept in its own small file and read before the window is made: the window's first frame, drawn before the page
 * has loaded, is painted in the theme's own ground, so a light Locust never flashes dark on the way in.
 */
export const THEME_CHOICES: readonly ThemeChoice[] = ['dark', 'light', 'system']

export function readTheme(file: string, read: (file: string) => string = (path) => readFileSync(path, 'utf8')): ThemeChoice {
  try {
    const saved = (JSON.parse(read(file)) as { theme?: unknown }).theme
    return THEME_CHOICES.includes(saved as ThemeChoice) ? (saved as ThemeChoice) : 'dark'
  } catch {
    return 'dark'
  }
}

/**
 * THE THEME A NEW LINUX INSTALL STARTS ON (0.865): the computer's. Linux desktops say light or dark through GTK and
 * Qt, and the AppImage catalog asks every app to follow that unless the person picks otherwise. Only a profile that
 * has never run (no saved theme, no teammates file) starts this way, so nobody's Locust changes under them; Windows
 * and Mac keep Dark, the look Locust was drawn in.
 */
export function firstTheme(platform: NodeJS.Platform, profileIsNew: boolean): ThemeChoice | undefined {
  return platform === 'linux' && profileIsNew ? 'system' : undefined
}

export const LOOK_CHOICES: readonly LookChoice[] = ['standard', 'liquid']

/** The look (Settings > General > Look, 0.855), kept in the same file: Standard unless Liquid glass was chosen. */
export function readLook(file: string, read: (file: string) => string = (path) => readFileSync(path, 'utf8')): LookChoice {
  try {
    const saved = (JSON.parse(read(file)) as { look?: unknown }).look
    return LOOK_CHOICES.includes(saved as LookChoice) ? (saved as LookChoice) : 'standard'
  } catch {
    return 'standard'
  }
}

export const GLASS_GROUNDS: readonly GlassGround[] = ['desktop', 'rainbow']

/** What Liquid glass shows through it (0.859), kept in the same file: the desktop unless Rainbow was chosen. */
export function readGlassGround(file: string, read: (file: string) => string = (path) => readFileSync(path, 'utf8')): GlassGround {
  try {
    const saved = (JSON.parse(read(file)) as { glassGround?: unknown }).glassGround
    return GLASS_GROUNDS.includes(saved as GlassGround) ? (saved as GlassGround) : 'desktop'
  } catch {
    return 'desktop'
  }
}

/** Every choice, written together, so saving one never drops another. */
export function writeAppearance(file: string, theme: ThemeChoice, look: LookChoice, glassGround: GlassGround = 'desktop'): void {
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, `${JSON.stringify({ theme, look, glassGround }, null, 2)}\n`, 'utf8')
}

/** The theme a choice comes to now: the computer's, for Match the computer. */
export function resolvedTheme(theme: ThemeChoice, systemIsDark: boolean): 'dark' | 'light' {
  return theme === 'system' ? (systemIsDark ? 'dark' : 'light') : theme
}

/** The window's ground before the page paints: --lc-bg-app of each theme (tokens.css). */
export function windowGround(theme: 'dark' | 'light'): string {
  return theme === 'light' ? '#f3f2ee' : '#090a0c'
}
