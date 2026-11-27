import { describe, expect, it } from 'vitest'

import { describeApproval, protocolAnswerFor, questionsOf } from './approval-channel.js'

/**
 * A Codex question is ANSWERED. It was being authorized, and the answer was
 * thrown away.
 *
 * Astra settled the protocol on 2026-09-09 from the schema codex-cli 0.153.0
 * generates for itself, plus the version-matched upstream handler. Two facts
 * came out of it, and Locust had both wrong:
 *
 *   `item/tool/requestUserInput` carries a `questions` ARRAY -- each with an
 *   id, header, text, flags and nullable options. Locust read singular
 *   `params.question` / `prompt` / `message`, matched none of them, and drew a
 *   card that said "Answer a question" with no question on it.
 *
 *   Its reply is `{ answers: { <questionId>: { answers: [...] } } }`, keyed by
 *   question id, with a chosen option identified by its LITERAL LABEL --
 *   options carry no id and no index. Locust sent `{ decision: 'accept' }` for
 *   all three buttons. The server cannot deserialize that as an answer: it
 *   logs the failure, substitutes an EMPTY answer map, and tells the model the
 *   person said nothing.
 *
 * And one correction to what I had claimed: "Always allow this session" never
 * granted anything for a question. There is no session-grant field for one. It
 * failed deserialization like the other two, so its only effect was the same
 * empty answer, under a label promising something the protocol cannot mean.
 *
 * NOT VERIFIED HERE, and not claimed anywhere: no live pending question has
 * been answered, no malformed answer sent to a running server, no cancellation
 * race exercised. Those need a real turn and real quota. This is "built to the
 * published schema and unit-tested", which is weaker than "seen working".
 */

const request = (params: unknown): Parameters<typeof describeApproval>[0] =>
  ({ id: 73, method: 'item/tool/requestUserInput', params }) as Parameters<typeof describeApproval>[0]

describe('reading the questions the runtime actually sent', () => {
  it('reads the array, with ids, options and flags', () => {
    const described = describeApproval(
      request({
        questions: [
          {
            id: 'database',
            header: 'Storage',
            question: 'Which database should the fixture use?',
            isOther: true,
            isSecret: false,
            options: [
              { label: 'SQLite', description: 'No server to run' },
              { label: 'Postgres', description: null }
            ]
          }
        ]
      })
    )
    expect(described?.kind).toBe('question')
    // The card gets the question TEXT, which it never used to have.
    expect(described?.detail).toBe('Which database should the fixture use?')
    const [question] = described?.questions ?? []
    expect(question?.id).toBe('database')
    expect(question?.header).toBe('Storage')
    expect(question?.isOther).toBe(true)
    expect(question?.options.map((option) => option.label)).toEqual(['SQLite', 'Postgres'])
  })

  it('says how many when a request carries several', () => {
    const described = describeApproval(
      request({
        questions: [
          { id: 'a', question: 'First?', options: [] },
          { id: 'b', question: 'Second?', options: [] }
        ]
      })
    )
    expect(described?.summary).toBe('Answer 2 questions')
    expect(described?.questions).toHaveLength(2)
  })

  it('still reads a singular question, if one ever arrives', () => {
    // Costs nothing to honour, and this build should not be the reason a
    // question goes unshown a second time.
    expect(describeApproval(request({ question: 'Ready?' }))?.detail).toBe('Ready?')
  })

  it('drops a question with no id, because an answer would have nowhere to go', () => {
    const questions = questionsOf({
      questions: [
        { question: 'no id here', options: [] },
        { id: 'good', question: 'this one is answerable', options: [] }
      ]
    })
    expect(questions.map((question) => question.id)).toEqual(['good'])
  })

  it('drops an option with no label, because a label IS its identity', () => {
    // The server matches returned strings against the labels it sent. An
    // option with no label cannot be chosen, so drawing it would offer
    // something that could never be communicated back.
    const [question] = questionsOf({
      questions: [{ id: 'q', question: 'Pick', options: [{ description: 'nameless' }, { label: 'Real' }] }]
    })
    expect(question?.options.map((option) => option.label)).toEqual(['Real'])
  })

  it('survives a malformed request without inventing a question', () => {
    expect(questionsOf({})).toEqual([])
    expect(questionsOf({ questions: 'not an array' })).toEqual([])
    expect(questionsOf({ questions: [null, 7, 'x'] })).toEqual([])
  })
})

describe('what goes back on the wire', () => {
  it('sends answers keyed by question id, not a decision', () => {
    /*
     * THE fix. Compare against what shipped: `{ decision: 'accept' }`, which
     * the server cannot read as an answer at all.
     */
    expect(
      protocolAnswerFor({
        approvalId: 'ap_1',
        answers: { database: ['SQLite'], notes: ['Keep the existing migration names.'] }
      })
    ).toEqual({
      answers: {
        database: { answers: ['SQLite'] },
        notes: { answers: ['Keep the existing migration names.'] }
      }
    })
  })

  it('sends an option by its literal label, never an index', () => {
    const sent = protocolAnswerFor({ approvalId: 'ap_1', answers: { q: ['Postgres'] } }) as {
      answers: Record<string, { answers: string[] }>
    }
    expect(sent.answers.q?.answers).toEqual(['Postgres'])
    // "3" for the third option would answer with the string "3".
    expect(sent.answers.q?.answers).not.toContain('3')
  })

  it('keeps an unanswered question present and empty rather than absent', () => {
    // "Asked and not answered" and "never asked" are different things, and the
    // wire can carry both.
    expect(protocolAnswerFor({ approvalId: 'ap_1', answers: { q: [] } })).toEqual({ answers: { q: { answers: [] } } })
  })

  it('still sends a decision for an authorization', () => {
    // The control: this change must not turn a command approval into an answer.
    expect(protocolAnswerFor({ approvalId: 'ap_1', decision: 'approve-once' })).toEqual({ decision: 'accept' })
    expect(protocolAnswerFor({ approvalId: 'ap_1', decision: 'approve-always' })).toEqual({
      decision: 'acceptForSession'
    })
    expect(protocolAnswerFor({ approvalId: 'ap_1', decision: 'deny' })).toEqual({ decision: 'decline' })
  })
})
