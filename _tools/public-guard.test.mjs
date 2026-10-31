import assert from 'node:assert/strict'
import { test } from 'vitest'

import { addedLines, changeProblems, lineProblems } from './public-guard.mjs'

// Every fake below is assembled here, so this file holds none of them whole.
const account = ['h', 'i', 's', 'b', 'o'].join('')
const github = 'gh' + 'p_' + 'Ab3'.repeat(12)
const openai = 's' + 'k-' + 'live'.repeat(6)
const pem = '-----' + 'BEGIN RSA PRIVATE KEY' + '-----'
const inbox = 'someone' + '@' + 'gmail.com'
const phone = '(352) ' + '867-' + '5309'
const terms = ['Elm Street 12', '703 867 5309'] // public-guard: allow (made-up terms)

const diffOf = (file, lines) => [`+++ b/${file}`, ...lines.map((line) => `+${line}`)].join('\n')

test('refuses the account name and a deny term, whatever the line says', () => {
  assert.match(lineProblems(`C:\\Users\\${account}\\work`, terms).join(), /account folder name/)
  assert.match(lineProblems(`at elm street 12, ${'public-guard: allow'}`, terms).join(), /deny term #1/)
  // A phone number in the deny file, written another way.
  assert.match(lineProblems('call 703-867-5309', terms).join(), /deny term #2/) // public-guard: allow
  // The term itself is never repeated.
  assert.ok(!lineProblems('at Elm Street 12', terms).join().includes('Elm'))
})

test('refuses key-shaped text, a private inbox and a real-looking phone number', () => {
  for (const value of [github, openai, pem, inbox, phone]) assert.ok(lineProblems(`x = '${value}'`, undefined).length > 0, value.slice(0, 4))
})

test('lets a marked fixture, a test address and a 555 number through', () => {
  assert.deepEqual(lineProblems(`const key = '${openai}' // public-guard: allow`, undefined), [])
  assert.deepEqual(lineProblems('author: drive@locust.test, noreply@anthropic.com', undefined), [])
  assert.deepEqual(lineProblems('tel (352) 555-0100', undefined), [])
  assert.deepEqual(lineProblems("const retina = 'trayTemplate@2x.png'", undefined), [])
})

test('refuses a private path, and reads only what a change adds', () => {
  const problems = changeProblems(['docs/PLAN-2026-10-06-NEXT.md', 'apps/desktop/src/a.ts'], diffOf('apps/desktop/src/a.ts', ['const ok = 1']), terms)
  assert.equal(problems.length, 1)
  assert.match(problems[0], /docs\/PLAN-2026-10-06-NEXT\.md: a private path/)
  const diff = ['+++ b/a.ts', `-old ${github}`, '+new line'].join('\n')
  assert.deepEqual([...addedLines(diff).get('a.ts')], ['new line'])
  assert.deepEqual(changeProblems(['a.ts'], diff, terms), [])
})

test("lets this repository's own workflows through", () => {
  assert.deepEqual(changeProblems(['.github/workflows/public-guard.yml'], '', undefined), [])
})
