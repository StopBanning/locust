import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { MissionApprovalRequest } from '../../shared/ipc.js'
import { ApprovalCard } from './components/ApprovalCard.js'

/**
 * A COMMAND'S CARD SAYS WHAT IT IS FOR (0.840, shared/command-plain.ts). Under the exact command, a row In short says
 * in a sentence what a familiar command does -- amber when it deletes, forces or throws work away -- and the phone
 * and chat apps carry the command and that sentence with Allow once.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

const asked = (over: Partial<MissionApprovalRequest>): MissionApprovalRequest => ({
  approvalId: 'ap_1',
  runId: 'run_1',
  missionId: 'mission_1',
  kind: 'command',
  summary: 'Run a command',
  detail: 'git push --force origin main',
  cwd: 'C:\\work\\shop',
  requestedAt: '2026-10-28T22:00:00.000Z',
  runtime: 'claude',
  ...over
})
const card = (over: Partial<MissionApprovalRequest> = {}): string =>
  renderToStaticMarkup(<ApprovalCard request={asked(over)} onDecide={() => undefined} onAnswer={() => undefined} busy={false} />)

describe('a command’s card', () => {
  it('says what a forced push does, under the command, in amber', () => {
    const html = card()
    expect(html).toContain('<dt>In short</dt><dd class="lc-approval__plain" data-careful="true">Replaces main on origin with this computer’s version: commits there that are not on this computer are lost.</dd>')
    // Under the exact command, before where it runs.
    expect(html.indexOf('<dt>Exact</dt>')).toBeLessThan(html.indexOf('<dt>In short</dt>'))
    expect(html.indexOf('<dt>In short</dt>')).toBeLessThan(html.indexOf('<dt>Where</dt>'))
  })

  it('says it plainly, without amber, for one that only looks', () => {
    expect(card({ detail: 'git status' })).toContain('<dt>In short</dt><dd class="lc-approval__plain">Shows which files have changed. It only looks.</dd>')
  })

  it('has no such row for a command it cannot read all of, or for a file change', () => {
    expect(card({ detail: 'npm test && ./deploy.sh' })).not.toContain('In short')
    expect(card({ kind: 'file-change', summary: 'Edit a file', detail: 'rm -rf dist' })).not.toContain('In short')
  })

  it('is amber on the card as a reach is', () => {
    expect(read('./shell.css')).toMatch(/\.lc-receipt dd\.lc-approval__plain\[data-careful\] \{\s*color: var\(--lc-amber-text\);/)
  })

  it('goes to the phone with the command itself', () => {
    const page = read('../../main/phone-page.ts')
    expect(page).toContain("item.exact ? el('div', { class: 'exact', text: item.exact }) : null")
    expect(page).toContain("item.plain ? el('div', { class: 'plain', 'data-careful': item.plain.careful === true, text: item.plain.said }) : null")
    const host = read('../../main/index.ts')
    expect(host).toContain("const exact = request.kind === 'question' ? undefined : exactLine(request)")
    expect(host).toContain('plain: { said: scrubSecrets(plain.said).text, careful: plain.careful }')
  })
})
