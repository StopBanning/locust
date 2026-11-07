import { isAbsolute, resolve } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { parseFileBlocks } from '../shared/handover.js'
import { joinMessageFragments } from '../shared/messageFragments.js'

/**
 * THE FILES A TEAMMATE'S OWN RECORD NAMES, WHEREVER THEY ARE (0.787).
 *
 * Codex, asked to write its review to D:\work\scratch, wrote it there with
 * its own file tool and handed it over by its whole path. Locust drew no
 * button: the host opens a file only inside the folders it knows
 * (reveal-file.ts), so the thread said "Not shown as a file", and the turn's
 * own "created a file" row would not open it either. Colin, 2026-10-18:
 * "claude code and the other apps still let you access the file".
 *
 * So a whole path outside those folders is honoured when a teammate's
 * recorded turn names it: in a file block it wrote, or as the whole target
 * of a step of its own that finished (Codex's apply_patch, Claude Code's
 * Write or Read). Those are the files this app showed the person as the
 * teammate's, and the window still names nothing it was not shown: the host
 * reads these from the ledger, never from the request.
 *
 * What this does not widen: a teammate can already read any file its own
 * permissions allow and say what is in it, so opening a file it named shows
 * the person nothing the teammate could not. A step that was declined or
 * failed names nothing here. A network path is never one: just looking at
 * `\\server\share` makes Windows sign in to that server.
 */

/** A step that did not do what it set out to: its target is not the teammate's. */
const DID_NOT_HAPPEN = /declin|refus|denied|reject|fail|error|cancel/i

/**
 * One spelling per file: resolved, and on Windows without case. Undefined for
 * anything but a whole path on this computer -- on Windows one that starts
 * with a drive, which no share or device path does.
 */
export function namedFileKey(path: string): string | undefined {
  const trimmed = path.trim()
  if (trimmed.length === 0 || trimmed.length > 400 || /[\u0000-\u001f\u007f]/.test(trimmed)) return undefined
  const whole = process.platform === 'win32' ? /^[a-z]:[\\/]/i.test(trimmed) : trimmed.startsWith('/')
  if (!whole || !isAbsolute(trimmed)) return undefined
  const resolved = resolve(trimmed)
  return process.platform === 'win32' ? resolved.toLowerCase() : resolved
}

/** The whole paths one turn's record names as the teammate's: what it handed over, and what its finished steps acted on. */
export function filesNamedIn(events: readonly NormalizedRuntimeEvent[]): readonly string[] {
  const named = new Set<string>()
  const add = (path: string): void => {
    const key = namedFileKey(path)
    if (key !== undefined) named.add(key)
  }
  for (const event of joinMessageFragments(events)) {
    if (event.type === 'message.delta') {
      if (event.payload.text.includes('<locust-file')) for (const file of parseFileBlocks(event.payload.text)) add(file.path)
      continue
    }
    if (event.type !== 'tool.completed') continue
    const { command, status } = event.payload
    if (command === undefined || (status !== undefined && DID_NOT_HAPPEN.test(status))) continue
    // A line that is a path and nothing else. A shell command names paths
    // inside a sentence of its own, and is not read for them.
    for (const line of command.split('\n')) add(line)
  }
  return [...named]
}

/**
 * A HANDED FILE NAMED FROM THE FOLDER IT RAN IN (0.868).
 *
 * Codex, working in now-coding, built in a worktree beside it and handed its
 * report over as `../locust-wt/spotify-artist/REPORT.md`. The thread drew it
 * from the conversation's folder, and the copy button said "Not copied":
 * only a whole path counted as named. Colin, 2026-11-06: "wont let me copy".
 *
 * So a relative path in a file block the teammate wrote is kept as it was
 * written, and the host resolves it only against the folder that turn ran in
 * (namedByATeammate), never against every folder it knows. Only file blocks:
 * a step's target was always a whole path, and a shell command is not read.
 */
export function relativeFilesNamedIn(events: readonly NormalizedRuntimeEvent[]): readonly string[] {
  const named = new Set<string>()
  for (const event of joinMessageFragments(events)) {
    if (event.type !== 'message.delta' || !event.payload.text.includes('<locust-file')) continue
    for (const file of parseFileBlocks(event.payload.text)) {
      const path = file.path.trim()
      if (path.length === 0 || path.length > 400 || /[\u0000-\u001f\u007f]/.test(path)) continue
      // A whole path is filesNamedIn's; a network or drive-relative spelling is never one.
      if (isAbsolute(path) || /^[\\/]/.test(path) || /^[a-z]:/i.test(path)) continue
      named.add(path)
    }
  }
  return [...named]
}

/** The key a relative handed path names once it is resolved against the folder its turn ran in. */
export function relativeNamedKey(folder: string, path: string): string | undefined {
  return namedFileKey(resolve(folder, path))
}
