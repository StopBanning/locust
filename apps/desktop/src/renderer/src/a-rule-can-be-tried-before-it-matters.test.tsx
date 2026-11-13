import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { COMPOUND_REFUSAL, whatIf, whatIfSentence } from '../../shared/approval-rules.js'
import type { ApprovalRule } from '../../shared/approval-rules.js'
import { TryARule } from './components/SavedApprovalRules.js'

/*
 * WHAT WOULD HAPPEN IF (0.874; the 13 Nov rival scan). A command typed under the saved rules is tried in every place
 * they speak to, and each place says what a card would do and the rule that decides. Nothing runs.
 */
const rule = (over: Partial<ApprovalRule> & Pick<ApprovalRule, 'ruleId' | 'effect' | 'pattern'>): ApprovalRule => ({
  kind: 'command',
  createdAt: '2026-11-13T00:00:00.000Z',
  ...over
})

const RULES: readonly ApprovalRule[] = [
  rule({ ruleId: 'r1', effect: 'allow', pattern: 'npm run test:*', teammateId: 'tm_wren', folder: '/work/shop' }),
  rule({ ruleId: 'r2', effect: 'deny', pattern: 'rm:*' }),
  rule({ ruleId: 'r3', effect: 'allow', pattern: 'ls' })
]

describe('what would happen if', () => {
  it('asks the rules in every place they speak to, and names the rule that decides', () => {
    const lines = whatIf('npm run test -- --watch', RULES)
    expect(lines.map((line) => [line.teammateId, line.folder, line.verdict.decision])).toEqual([
      [undefined, undefined, 'ask'],
      ['tm_wren', '/work/shop', 'allow']
    ])
    expect(whatIfSentence(lines[0]!)).toBe('Any teammate, anywhere: it asks you first. No saved rule speaks to it.')
    expect(whatIfSentence(lines[1]!, 'Wren')).toBe('Wren in shop: it runs without asking, because of the rule “Wren may run commands starting "npm run test" in shop without asking.”')
  })

  it('says a refusal and its rule, and a deny wins over an allow everywhere', () => {
    const lines = whatIf('rm -rf build', RULES)
    expect(lines.every((line) => line.verdict.decision === 'deny')).toBe(true)
    expect(whatIfSentence(lines[0]!)).toBe('Any teammate, anywhere: it is refused, because of the rule “Any teammate may not run commands starting "rm".”')
    // A deny reads a compound command part by part.
    expect(whatIf('ls; rm -rf build', RULES)[0]!.verdict.decision).toBe('deny')
  })

  it('says why a compound command still asks though one of its parts is allowed', () => {
    const [anywhere] = whatIf('ls | wc -l', RULES)
    expect(anywhere!.verdict).toEqual({ decision: 'ask', why: COMPOUND_REFUSAL })
    expect(whatIfSentence(anywhere!)).toBe(`Any teammate, anywhere: it asks you first. ${COMPOUND_REFUSAL}`)
  })

  it('tries nothing for an empty box, and only command rules make places', () => {
    expect(whatIf('   ', RULES)).toEqual([])
    const withEdit = [...RULES, rule({ ruleId: 'r4', effect: 'allow', kind: 'edit', pattern: 'src/**', teammateId: 'tm_ada' })]
    expect(whatIf('ls', withEdit).map((line) => line.teammateId)).toEqual([undefined, 'tm_wren'])
  })

  it('draws the box under the rules with each place’s answer, and says nothing runs', () => {
    const html = renderToStaticMarkup(<TryARule rules={RULES} names={{ tm_wren: 'Wren' }} command="rm -rf build" onCommand={() => undefined} />)
    expect(html).toContain('What would happen if a teammate asked to run:')
    expect(html).toContain('Wren in shop: it is refused, because of the rule')
    expect(html).toContain('lc-ruletry__deny')
    expect(html).toContain('Nothing runs here. It asks the saved rules the same question a card would.')
    expect(renderToStaticMarkup(<TryARule rules={RULES} names={{}} command="" onCommand={() => undefined} />)).not.toContain('lc-ruletry__answers')
  })
})
