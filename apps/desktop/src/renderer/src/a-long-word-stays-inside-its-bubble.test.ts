import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const css = readFileSync(new URL('./shell.css', import.meta.url), 'utf8')

describe('a message you sent', () => {
  // case: Colin, 22 Nov, on a message carrying test names such as
  // asks_a_side_question_in_a_freshly_briefed_acp_session_without_appending_to_the_parent: the name ran past the
  // bubble's right edge and was cut off ("Bug.").
  it('breaks a word longer than its bubble inside the bubble', () => {
    const bubble = /\n\.lc-bubble \{([^}]*)\}/.exec(css)?.[1] ?? ''
    expect(bubble).toMatch(/max-width: 78%/)
    expect(bubble).toMatch(/overflow-wrap: anywhere;/)
  })
})
