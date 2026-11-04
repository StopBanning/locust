import { describe, expect, it } from 'vitest'

import { themeFromArguments } from '../shared/appearance.js'
import { firstTheme, readTheme, resolvedTheme, windowGround } from './appearance.js'

/**
 * A THEME IS THE PERSON'S CHOICE (0.789). Locust forced the window dark; now
 * Settings > Appearance offers Dark, Light and Match computer. Dark stays the
 * default, so nobody's Locust changes under them, and the window's first
 * frame is drawn in the chosen ground so a light Locust never flashes dark.
 */
describe('the saved theme', () => {
  it('is Dark when nothing was saved, or what was saved cannot be read', () => {
    expect(readTheme('/nowhere/appearance.json', () => { throw new Error('ENOENT') })).toBe('dark')
    expect(readTheme('/f', () => 'not json')).toBe('dark')
    expect(readTheme('/f', () => JSON.stringify({ theme: 'sepia' }))).toBe('dark')
  })

  it('is what the person chose', () => {
    expect(readTheme('/f', () => JSON.stringify({ theme: 'light' }))).toBe('light')
    expect(readTheme('/f', () => JSON.stringify({ theme: 'system' }))).toBe('system')
  })
})

describe('the theme a new copy starts on (0.865)', () => {
  it("is the computer's on a Linux profile that has never run", () => {
    expect(firstTheme('linux', true)).toBe('system')
  })

  it('leaves a Linux profile that has run, and every Windows and Mac profile, to what was saved', () => {
    expect(firstTheme('linux', false)).toBeUndefined()
    expect(firstTheme('win32', true)).toBeUndefined()
    expect(firstTheme('darwin', true)).toBeUndefined()
  })
})

describe('what a choice comes to', () => {
  it("follows the computer under Match computer, and is itself otherwise", () => {
    expect(resolvedTheme('system', true)).toBe('dark')
    expect(resolvedTheme('system', false)).toBe('light')
    expect(resolvedTheme('light', true)).toBe('light')
    expect(resolvedTheme('dark', false)).toBe('dark')
  })

  it("paints the window's first frame in that theme's ground", () => {
    expect(windowGround('dark')).toBe('#090a0c')
    expect(windowGround('light')).toBe('#f3f2ee')
  })

  it('reaches the page on its command line, and reads as dark when it says nothing readable', () => {
    expect(themeFromArguments(['electron', '--locust-theme=light'])).toBe('light')
    expect(themeFromArguments(['electron', '--locust-theme=dark'])).toBe('dark')
    expect(themeFromArguments(['electron', '--locust-theme=chartreuse'])).toBe('dark')
    expect(themeFromArguments(['electron'])).toBe('dark')
  })
})
