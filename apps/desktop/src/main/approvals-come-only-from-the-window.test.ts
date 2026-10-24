import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

/**
 * A2.16: APPROVALS NEVER RIDE ON A TEAMMATE'S MESSAGE.
 *
 * CHECKED 2026-09-24: an approval -- a Codex action, a Claude Code connector
 * permission, an Antigravity question -- is answered in exactly one place,
 * the MISSION_APPROVAL_DECIDE handler, which takes only the app's own window
 * (`fromOwnWindow`): a person's click. A run the relay started raises its
 * cards like any other and nothing auto-answers them. Nothing a teammate
 * writes -- a share, a relayed brief, a returned answer -- reaches `decide`.
 *
 * This keeps it that way: every call of `.decide(` in the main process is in
 * the modules that hold approvals, or in that one handler. A new caller -- a
 * relay path, a block parser -- fails here and has to say why it may.
 *
 * 0.775, YOUR PHONE: the person's own tap on their own phone is the one other
 * place, and it says why it may -- the phone host checks the key the person
 * scanned before it reads anything (phone-host.ts), and its `decide` goes down
 * the window's own path, `decideApproval`, with every guard the window's has.
 *
 * 0.831, TELEGRAM: the person's own press, or reply, in their own paired chat is the other. It says why it may too --
 * the link checks the chat and the person who wrote against the ones that paired before it reads a button
 * (telegram.ts, `pairedFrom`) -- and its `decide` is that same `decideApproval`. It offers Allow once and Deny,
 * never Always.
 *
 * 0.832, DISCORD AND SLACK: the three chat apps share one link (chat-link.ts), so that check is made in one place for
 * all of them, and no app's own file answers a card at all.
 */
const MAIN = new URL('.', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const HOLDERS = new Set(['approval-channel.ts', 'permission-host.ts', 'antigravity-mission.ts', 'codex-mission.ts'])

describe('who may answer an approval', () => {
  it('is only the approval holders and the window handler', () => {
    const callers = readdirSync(MAIN)
      .filter((name) => name.endsWith('.ts') && !name.endsWith('.test.ts'))
      .filter((name) => /\.decide\(/.test(readFileSync(join(MAIN, name), 'utf8')))
    expect(callers.filter((name) => !HOLDERS.has(name) && name !== 'index.ts' && name !== 'phone-host.ts' && name !== 'chat-link.ts')).toEqual([])
    // The chat apps' two calls, a button and a reply in words, each only after the chat and its person were checked.
    const chat = readFileSync(join(MAIN, 'chat-link.ts'), 'utf8')
    expect(chat.match(/\.decide\(/g)).toHaveLength(2)
    const button = chat.indexOf('const handlePress = async')
    expect(chat.indexOf('pairedFrom(press.from, press.place)', button)).toBeGreaterThan(button)
    expect(chat.indexOf('pairedFrom(press.from, press.place)', button)).toBeLessThan(chat.indexOf('options.decide(', button))
    const message = chat.indexOf('const handleMessage = async')
    expect(chat.indexOf('if (!pairedFrom(incoming.from, incoming.place)) return', message)).toBeGreaterThan(message)
    expect(chat.indexOf('if (!pairedFrom(incoming.from, incoming.place)) return', message)).toBeLessThan(chat.indexOf('options.decide(', message))
    for (const name of ['chat-link.ts', 'telegram.ts', 'discord.ts', 'slack.ts']) expect(readFileSync(join(MAIN, name), 'utf8'), name).not.toContain("'approve-always'")
    // The phone's one call, and only after its key was checked against the person's.
    const phone = readFileSync(join(MAIN, 'phone-host.ts'), 'utf8')
    expect(phone.match(/\.decide\(/g)).toHaveLength(1)
    expect(phone.indexOf('timingSafeEqual(')).toBeGreaterThan(-1)
    expect(phone.indexOf('timingSafeEqual(')).toBeLessThan(phone.indexOf('calls.decide('))
    // In index.ts, one funnel -- `answerApproval` -- and nothing else calls decide.
    const index = readFileSync(join(MAIN, 'index.ts'), 'utf8').split('\n')
    const decides = index.map((line, at) => ({ line, at })).filter(({ line }) => /\.decide\(/.test(line))
    expect(decides).toHaveLength(1)
    const funnel = index.findIndex((line) => line.includes('const answerApproval = async'))
    expect(funnel).toBeGreaterThan(-1)
    expect(decides[0]!.at - funnel).toBeLessThan(3)
    /*
     * 0.521: the funnel is called from three places, each a decision of the
     * person's -- their click (the window handler), their click on a card's
     * "don't ask again" (the from-card handler, also window-only), and a rule
     * they saved. 0.616: the third is the one decision path
     * (answerByDecision, shared/who-decides.ts), which also answers by the
     * person's own Always on an earlier card of the run: two calls, both in
     * it. A caller anywhere else fails here.
     */
    const calls = index.map((line, at) => ({ line, at })).filter(({ line }) => /answerApproval\(/.test(line))
    expect(calls).toHaveLength(4)
    const enclosing = (at: number): string => {
      for (let line = at; line >= 0; line -= 1) {
        const opened = /ipcMain\.handle\((\w+)|const (answerByDecision|decideApproval) = /.exec(index[line] ?? '')
        if (opened !== null) return opened[1] ?? opened[2] ?? ''
      }
      return ''
    }
    expect(calls.map(({ at }) => enclosing(at)).sort()).toEqual(['APPROVAL_RULE_FROM_CARD_CHANNEL', 'answerByDecision', 'answerByDecision', 'decideApproval'])
    // The editor's private API owns only new turns minted under its per-session capability. The MCP bridge
    // offers no such tool. Its once-only answer uses the same guarded path as the window and paired phone.
    const decideCalls = index.map((line, at) => ({ line, at })).filter(({ line }) => /decideApproval\(/.test(line))
    expect(decideCalls.map(({ at }) => (/ipcMain\.handle\(MISSION_APPROVAL_DECIDE_CHANNEL/.test(index[at - 2] ?? '') ? 'window' : /decide: async \(approvalId, approve\)/.test(index[at - 1] ?? '') ? 'phone' : /answer: async \(approvalId, approve\)/.test(index[at - 1] ?? '') ? 'editor' : /decide: async \(answer\) =>/.test(index[at - 1] ?? '') ? 'chat apps' : `line ${String(at + 1)}`))).toEqual(['window', 'editor', 'phone', 'chat apps'])
    for (const channel of ['MISSION_APPROVAL_DECIDE_CHANNEL', 'APPROVAL_RULE_FROM_CARD_CHANNEL']) {
      const handler = index.findIndex((line) => line.includes(`ipcMain.handle(${channel}`))
      expect(index[handler + 1], channel).toContain('fromOwnWindow(event)')
    }
    // And it answers only from the person's saved rules and their own Always, through the one evaluator.
    const byDecision = index.findIndex((line) => line.includes('const answerByDecision = '))
    expect(index.slice(byDecision, byDecision + 8).join('\n')).toContain('approvalRules.list()')
    expect(index.slice(byDecision, byDecision + 8).join('\n')).toContain('decide(request, ')
  })
})

/*
 * M31: the window drops an approval card when its run ends. Pinned here,
 * beside the rule for who may answer one, because the card and the answer are
 * one contract: a card no run can take must not be offered.
 */
describe('an approval card in the window', () => {
  it('is dropped when its run ends: App applies approvalsOfLiveRuns on every change of runs', () => {
    const app = readFileSync(join(MAIN, '..', 'renderer', 'src', 'App.tsx'), 'utf8')
    expect(app).toMatch(/useEffect\(\(\) => \{\s*setApprovals\(\(current\) => approvalsOfLiveRuns\(current, runs\)\)\s*\}, \[runs\]\)/)
  })
})
