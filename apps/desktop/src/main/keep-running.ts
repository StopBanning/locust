import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'

/**
 * Keep Locust running in the background when the window is closed.
 *
 * Off unless the person turned it on. The file is the whole preference:
 * missing, unreadable, or anything but `true` is off.
 */

export const BACKGROUND_FILE = 'background.json'

export interface KeepRunningState {
  readonly keepRunning: boolean
}

export function keepRunningFrom(value: unknown): KeepRunningState {
  if (typeof value !== 'object' || value === null) return { keepRunning: false }
  return { keepRunning: (value as { readonly keepRunning?: unknown }).keepRunning === true }
}

export function readKeepRunning(file: string): boolean {
  try {
    return keepRunningFrom(JSON.parse(readFileSync(file, 'utf8'))).keepRunning
  } catch {
    return false
  }
}

/**
 * Keep this computer awake while it is plugged in (0.838), so routines and
 * Scout's looks run on time. Off unless the person turned it on, read the
 * same way: anything but `true` is off.
 */
export const AWAKE_FILE = 'awake.json'

export function readStayAwake(file: string): boolean {
  try {
    const value: unknown = JSON.parse(readFileSync(file, 'utf8'))
    return typeof value === 'object' && value !== null && (value as { readonly stayAwake?: unknown }).stayAwake === true
  } catch {
    return false
  }
}

export function writeStayAwake(file: string, stayAwake: boolean): void {
  mkdirSync(dirname(file), { recursive: true })
  const temporary = `${file}.tmp`
  writeFileSync(temporary, `${JSON.stringify({ stayAwake })}\n`, 'utf8')
  renameSync(temporary, file)
}

export function writeKeepRunning(file: string, keepRunning: boolean): void {
  mkdirSync(dirname(file), { recursive: true })
  // Whole or absent (0.587): written beside, then renamed over, as the other stores do.
  const temporary = `${file}.tmp`
  writeFileSync(temporary, `${JSON.stringify({ keepRunning })}\n`, 'utf8')
  renameSync(temporary, file)
}
