export type FileMenuId = 'attach' | 'vscode' | 'cursor' | 'default' | 'reveal' | 'copy-path' | 'copy-relative' | 'copy-name'

export interface FileMenuChoice {
  readonly id: FileMenuId
  readonly label: string
}

/** Extensions the default app would run. An editor may still open them as text. */
const SCRIPTS = new Set(['bat', 'cmd', 'ps1', 'exe', 'com', 'scr', 'msi', 'vbs', 'js', 'mjs', 'wsf', 'wsh'])

export function fileBaseName(path: string): string {
  const base = path.replace(/\\/g, '/').split('/').pop() ?? path
  return base.length === 0 ? path : base
}

export function fileExtension(path: string): string {
  const base = fileBaseName(path)
  const dot = base.lastIndexOf('.')
  if (dot <= 0) return ''
  return base.slice(dot + 1).toLowerCase()
}

export function isScriptPath(path: string): boolean {
  return SCRIPTS.has(fileExtension(path))
}

function splitPath(value: string): string[] {
  const out: string[] = []
  for (const part of value.replace(/\\/g, '/').split('/')) {
    if (part === '' || part === '.') continue
    if (part === '..') {
      out.pop()
      continue
    }
    out.push(part)
  }
  return out
}

/**
 * A path a screen shows, as a file in the folder when it is written relative
 * to that folder. An absolute path is left as given. The host only reveals
 * and opens an absolute path.
 */
export function rootedPath(path: string, folder: string | undefined): string {
  const slash = path.trim().replace(/\\/g, '/')
  if (folder === undefined || folder.trim().length === 0) return path
  if (slash.startsWith('/') || /^[a-zA-Z]:\//.test(slash)) return path
  const base = folder.trim().replace(/[\\/]+$/, '')
  return `${base}/${slash.replace(/^\.\//, '')}`
}

/** Whether `candidate` is `folder` or somewhere under it, after `.` and `..` are collapsed. Case-blind on Windows. */
export function pathInsideFolder(candidate: string, folder: string | undefined, platform: string): boolean {
  if (folder === undefined || folder.trim().length === 0 || candidate.trim().length === 0) return false
  const fold = (part: string): string => (platform === 'win32' ? part.toLowerCase() : part)
  const inner = splitPath(candidate)
  const outer = splitPath(folder)
  if (outer.length === 0 || inner.length < outer.length) return false
  return outer.every((part, index) => fold(inner[index] ?? '') === fold(part))
}

/** The path under the folder, or the path as given when it is not inside. */
export function relativeToFolder(candidate: string, folder: string | undefined, platform: string): string {
  if (folder === undefined || !pathInsideFolder(candidate, folder, platform)) return candidate
  const rest = splitPath(candidate).slice(splitPath(folder).length).join('/')
  return rest.length === 0 ? fileBaseName(candidate) : rest
}

export function looksLikeFilePath(text: string): boolean {
  const value = text.trim()
  if (value.length === 0 || value.length > 400 || /\s/.test(value) || value.includes('://')) return false
  return value.includes('/') || value.includes('\\')
}

/**
 * What a right-click on a path offers.
 *
 * Inside the conversation's folder: attach, the editors that are actually
 * installed, the default app (never for a script), the file manager, and the
 * three copies. Outside that folder: the three copies only.
 */
export function fileMenuChoices(input: {
  readonly path: string
  readonly folder: string | undefined
  readonly platform: string
  readonly editors: readonly ('vscode' | 'cursor')[]
}): readonly FileMenuChoice[] {
  const copies: readonly FileMenuChoice[] = [
    { id: 'copy-path', label: 'Copy path' },
    { id: 'copy-relative', label: 'Copy relative path' },
    { id: 'copy-name', label: 'Copy file name' }
  ]
  if (!pathInsideFolder(input.path, input.folder, input.platform)) return copies
  const open: FileMenuChoice[] = [{ id: 'attach', label: 'Add to my next message' }]
  if (input.editors.includes('vscode')) open.push({ id: 'vscode', label: 'Open in VS Code' })
  if (input.editors.includes('cursor')) open.push({ id: 'cursor', label: 'Open in Cursor' })
  if (!isScriptPath(input.path)) open.push({ id: 'default', label: 'Open in the default app' })
  const reveal = input.platform === 'darwin' ? 'Show in Finder' : input.platform === 'linux' ? 'Show in the file manager' : 'Show in Explorer'
  open.push({ id: 'reveal', label: reveal })
  return [...open, ...copies]
}
