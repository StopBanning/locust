import type { ReactElement } from 'react'

import { Icon } from './Icon.js'

/*
 * WHAT KIND OF FILE, AT A GLANCE (0.835). A changed file's row led with the same grey page for every file, so a list of
 * nine read as nine of one thing. Claude Code's own list marks each by its kind -- Colin, 2026-10-25, of its card:
 * "very aesthetic" -- and so does every editor: a small badge in the language's own colour, a page for a document.
 * The colours are the languages' own, the same in both themes, like the runtimes' marks.
 */
type Kind = { readonly badge: string; readonly tone: string } | { readonly page: true }

const BADGES: Readonly<Record<string, Kind>> = {
  js: { badge: 'JS', tone: 'js' }, mjs: { badge: 'JS', tone: 'js' }, cjs: { badge: 'JS', tone: 'js' }, jsx: { badge: 'JS', tone: 'js' },
  ts: { badge: 'TS', tone: 'ts' }, mts: { badge: 'TS', tone: 'ts' }, cts: { badge: 'TS', tone: 'ts' }, tsx: { badge: 'TS', tone: 'ts' },
  py: { badge: 'PY', tone: 'py' },
  rs: { badge: 'RS', tone: 'rs' },
  go: { badge: 'GO', tone: 'go' },
  java: { badge: 'JV', tone: 'java' }, kt: { badge: 'KT', tone: 'kt' }, swift: { badge: 'SW', tone: 'swift' },
  rb: { badge: 'RB', tone: 'rb' }, php: { badge: 'PH', tone: 'php' }, cs: { badge: 'C#', tone: 'cs' },
  c: { badge: 'C', tone: 'c' }, h: { badge: 'H', tone: 'c' }, cpp: { badge: 'C+', tone: 'c' }, hpp: { badge: 'H+', tone: 'c' },
  html: { badge: '<>', tone: 'html' }, htm: { badge: '<>', tone: 'html' }, vue: { badge: 'V', tone: 'vue' }, svelte: { badge: 'S', tone: 'svelte' },
  css: { badge: '#', tone: 'css' }, scss: { badge: '#', tone: 'css' }, sass: { badge: '#', tone: 'css' },
  json: { badge: '{}', tone: 'data' }, jsonl: { badge: '{}', tone: 'data' }, yml: { badge: 'YM', tone: 'data' }, yaml: { badge: 'YM', tone: 'data' }, toml: { badge: 'TM', tone: 'data' },
  sh: { badge: '$', tone: 'shell' }, bash: { badge: '$', tone: 'shell' }, ps1: { badge: '$', tone: 'shell' }, bat: { badge: '$', tone: 'shell' }, cmd: { badge: '$', tone: 'shell' },
  sql: { badge: 'SQ', tone: 'data' },
  md: { page: true }, mdx: { page: true }, txt: { page: true }, rst: { page: true }, csv: { page: true }, log: { page: true }
}

const IMAGES = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'bmp', 'avif'])

/** The kind a path's name says, by its extension; undefined for one this list does not know. */
export function fileKind(path: string): Kind | 'image' | undefined {
  const name = path.split(/[\\/]/).pop() ?? path
  const dot = name.lastIndexOf('.')
  if (dot <= 0) return undefined
  const extension = name.slice(dot + 1).toLowerCase()
  if (IMAGES.has(extension)) return 'image'
  return BADGES[extension]
}

export function FileTypeIcon({ path }: { readonly path: string }): ReactElement {
  const kind = fileKind(path)
  if (kind === 'image') {
    return (
      <svg className="lc-filetype is-image" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
        <rect x="1.5" y="2.5" width="11" height="9" rx="1.5" />
        <path className="lc-filetype__lines" d="M3.5 10l2.6-3 1.9 2.1 1.2-1.3 1.8 2.2" />
        <circle className="lc-filetype__dot" cx="9.6" cy="5.2" r="1" />
      </svg>
    )
  }
  if (kind !== undefined && 'page' in kind) {
    return (
      <svg className="lc-filetype is-page" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
        <path d="M3 1.5h5.2L11 4.3V12a.5.5 0 0 1-.5.5h-7.5A.5.5 0 0 1 2.5 12V2a.5.5 0 0 1 .5-.5z" />
        <path className="lc-filetype__lines" d="M4.6 7h4.8M4.6 9.2h4.8" />
      </svg>
    )
  }
  // The letters are drawn, not written: a copied or read-out row says the file's name, never "TS notes.ts".
  if (kind !== undefined) return <span className={`lc-filetype is-${kind.tone}`} data-badge={kind.badge} aria-hidden="true" />
  return <span className="lc-filetype is-plain" aria-hidden="true"><Icon name="file" size={14} /></span>
}

/** The file's own name, and its folder's, for telling two files of one name apart. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).filter((part) => part.length > 0).pop() ?? path
}

export function folderName(path: string): string | undefined {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0)
  return parts.length > 1 ? parts[parts.length - 2] : undefined
}

/** A status worth saying beside a changed file: a new, deleted or renamed one. A changed one is every row. */
export function statusWord(status: string): string | undefined {
  return status === 'ADDED' ? 'new' : status === 'DELETED' ? 'deleted' : status === 'RENAMED' ? 'renamed' : undefined
}
