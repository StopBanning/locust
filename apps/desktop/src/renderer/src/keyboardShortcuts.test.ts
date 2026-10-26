import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

import { SHORTCUTS, shortcutGroups } from './keyboardShortcuts.js'

const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('the shortcuts sheet matches the handlers', () => {
  it('groups them for a person, and every row is a key the code actually handles', () => {
    expect(shortcutGroups().map((group) => group.name)).toEqual(['Conversations', 'The message box', 'Panes', 'Menus'])
    const sources = new Map<string, string>()
    for (const row of SHORTCUTS) {
      const source = sources.get(row.file) ?? read(row.file)
      sources.set(row.file, source)
      expect(source, row.keys).toContain(row.mark)
    }
  })
})
