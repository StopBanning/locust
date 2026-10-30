import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { ATTACHMENT_DIR, attachmentDestination, excludeWith, GIT_EXCLUDE_LINE } from './attach-outside.js'

// A file the person picked, as the machine running the test spells it: a backslash separates folders only on Windows.
const WINDOWS = process.platform === 'win32'
const DOWNLOADS = WINDOWS ? 'C:\\Users\\c\\Downloads' : '/Users/c/Downloads'

describe('finding a home for a file from outside the workspace', () => {
  it('keeps the name it had', () => {
    expect(attachmentDestination(join(DOWNLOADS, 'report.pdf'), new Set())).toBe(
      `${ATTACHMENT_DIR}/report.pdf`
    )
    expect(attachmentDestination('/home/c/shot.png', new Set())).toBe(`${ATTACHMENT_DIR}/shot.png`)
  })

  it('never overwrites a file already there', () => {
    // THE test. Two `notes.md` from two folders is the ordinary case, and
    // silently overwriting would send the same file twice while showing two
    // tiles -- the app claiming something that did not happen.
    expect(attachmentDestination('/a/notes.md', new Set(['notes.md']))).toBe(`${ATTACHMENT_DIR}/notes-2.md`)
    expect(attachmentDestination('/b/notes.md', new Set(['notes.md', 'notes-2.md']))).toBe(
      `${ATTACHMENT_DIR}/notes-3.md`
    )
  })

  it('handles a name with no extension, and one with several dots', () => {
    expect(attachmentDestination('/a/README', new Set(['README']))).toBe(`${ATTACHMENT_DIR}/README-2`)
    expect(attachmentDestination('/a/archive.tar.gz', new Set(['archive.tar.gz']))).toBe(
      `${ATTACHMENT_DIR}/archive.tar-2.gz`
    )
  })
})

describe('keeping the folder out of git', () => {
  it('adds the line to an exclude file that lacks it', () => {
    expect(excludeWith('')).toBe(`${GIT_EXCLUDE_LINE}\n`)
    expect(excludeWith('# comment\n*.log\n')).toBe(`# comment\n*.log\n${GIT_EXCLUDE_LINE}\n`)
  })

  it('adds a newline first when the file does not end with one', () => {
    expect(excludeWith('*.log')).toBe(`*.log\n${GIT_EXCLUDE_LINE}\n`)
  })

  it('does nothing when the folder is already excluded', () => {
    // Writing every launch would rewrite a file in someone's repository for
    // no reason, and a growing list of identical lines is its own bug.
    expect(excludeWith(`${GIT_EXCLUDE_LINE}\n`)).toBeUndefined()
    expect(excludeWith('.locust\n')).toBeUndefined()
    expect(excludeWith('*.log\n/.locust/\n')).toBeUndefined()
  })
})

describe('a source that is not really a file name', () => {
  /*
   * `attachmentDestination` builds a path this app WRITES to, from
   * `basename(source)` -- and basename does not always return a plain name.
   * Measured before this guard existed:
   *
   *   'C:\a\..'  ->  '..'  ->  .locust/attachments/..  ->  .locust
   *
   * One level above the folder it is supposed to write into. Nothing a person
   * does reaches it -- the OS dialog cannot return such a path -- but a
   * path-builder must not rely on its caller for containment, and
   * `LOCUST_ATTACH_PATHS` already feeds this from elsewhere.
   */
  it('refuses a name that would climb out of the folder', () => {
    expect(() => attachmentDestination('C:\\a\\..', new Set())).toThrow(/not a file name/)
    expect(() => attachmentDestination('..', new Set())).toThrow(/not a file name/)
  })

  it('refuses a name that would land on the folder itself', () => {
    // These would try to write a file over a directory.
    expect(() => attachmentDestination('C:\\a\\.', new Set())).toThrow(/not a file name/)
    expect(() => attachmentDestination('C:\\', new Set())).toThrow(/not a file name/)
  })

  it('refuses an alternate data stream', () => {
    // `file.txt:stream` names a hidden stream on Windows rather than a file,
    // and a picked file never has one.
    expect(() => attachmentDestination('C:\\a\\file.txt:stream', new Set())).toThrow(/not a file name/)
  })

  it('still takes every ordinary name, including awkward ones', () => {
    // The control. A guard that refused real filenames would be worse than
    // the hole it closes.
    for (const name of ['notes.md', 'my report (final).pdf', 'a b  c.txt', '.gitignore', 'ünïcodé.md', 'archive.tar.gz']) {
      expect(attachmentDestination(join(DOWNLOADS, name), new Set())).toBe(`${ATTACHMENT_DIR}/${name}`)
    }
  })

  it('a traversal INSIDE the source path is harmless, because only the name is used', () => {
    // `C:\a\..\..\evil.txt` is just a roundabout way of naming evil.txt, and
    // the file it points at was already chosen by the person.
    // Spelled out, not joined: join would take the `..` away before the code saw it.
    expect(attachmentDestination(WINDOWS ? 'C:\\a\\..\\..\\evil.txt' : '/a/../../evil.txt', new Set())).toBe(`${ATTACHMENT_DIR}/evil.txt`)
  })
})
