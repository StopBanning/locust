import { existsSync } from 'node:fs'
import { posix, win32 } from 'node:path'
import { spawn } from 'node:child_process'

import { spawnShape } from '@teammate/runtime-adapters'

export type EditorId = 'vscode' | 'cursor'

export interface InstalledEditor {
  readonly id: EditorId
  readonly command: string
}

/**
 * Editors found on PATH. Nothing is guessed from a user's install folder:
 * if `code` or `cursor` is not on PATH, that editor is not offered.
 */
export function editorsOnPath(
  platform: string,
  env: NodeJS.ProcessEnv,
  exists: (file: string) => boolean = existsSync
): readonly InstalledEditor[] {
  const names = (id: EditorId): readonly string[] => {
    if (platform === 'win32') return id === 'vscode' ? ['code.cmd', 'Code.cmd', 'code.exe', 'Code.exe'] : ['cursor.cmd', 'Cursor.cmd', 'cursor.exe', 'Cursor.exe']
    return id === 'vscode' ? ['code'] : ['cursor']
  }
  const found: InstalledEditor[] = []
  for (const id of ['vscode', 'cursor'] as const) {
    const command = names(id)
      .map((name) => onPath(name, platform, env, exists))
      .find((file) => file !== undefined)
    if (command !== undefined) found.push({ id, command })
  }
  return found
}

function onPath(name: string, platform: string, env: NodeJS.ProcessEnv, exists: (file: string) => boolean): string | undefined {
  const path = platform === 'win32' ? win32 : posix
  return (env.PATH ?? env.Path ?? '')
    .split(path.delimiter)
    .map((folder) => path.join(folder, name))
    .find((file) => path.isAbsolute(file) && exists(file))
}

/**
 * How to start an editor on a file.
 *
 * A Windows `.cmd` shim cannot be spawned directly, and Node's `shell: true` joins the arguments with spaces and
 * escapes nothing: a teammate's file named `a&calc&.txt` -- `&` is a legal file-name character -- ran `calc` from
 * "Open in VS Code" (0.836 review). The shim runs through cmd.exe the way the runtimes' launchers do (H8,
 * `spawnShape`): every part quoted, every cmd metacharacter escaped with a caret, the line handed over verbatim.
 * cmd.exe is System32's, never a bare name a working folder could plant (H1).
 */
export function editorSpawn(
  command: string,
  file: string,
  platform: string,
  env: NodeJS.ProcessEnv
): { readonly file: string; readonly args: readonly string[]; readonly windowsVerbatimArguments: boolean } {
  if (platform !== 'win32' || !/\.(cmd|bat)$/i.test(command)) return { file: command, args: [file], windowsVerbatimArguments: false }
  const cmd = win32.join(env.SystemRoot ?? env.SYSTEMROOT ?? 'C:\\Windows', 'System32', 'cmd.exe')
  const shape = spawnShape(cmd, ['/d', '/s', '/c', command, file])
  return { file: cmd, args: shape.args, windowsVerbatimArguments: shape.windowsVerbatimArguments === true }
}

/** Start an editor on a file and do not wait for it. */
export function launchEditor(command: string, file: string, platform: string, env: NodeJS.ProcessEnv): void {
  const launch = editorSpawn(command, file, platform, env)
  const child = spawn(launch.file, [...launch.args], {
    detached: true,
    stdio: 'ignore',
    windowsHide: true,
    windowsVerbatimArguments: launch.windowsVerbatimArguments
  })
  child.unref()
}
