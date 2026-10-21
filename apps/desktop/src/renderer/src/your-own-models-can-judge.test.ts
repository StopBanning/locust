import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

/**
 * YOUR OWN MODELS JUDGE TOO (0.810). A model the person added with their own key was never offered as a judge, so
 * three Claude judges on the API could not be asked (the credits drive, 2026-10-21). One that only chats is left out:
 * its runs get no tools, and a long brief reaches a judge as files to read.
 */
const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
const choices = source.slice(source.indexOf('const judgeChoicesFor ='), source.indexOf('const others = picked.filter'))

describe('the models a judge can be', () => {
  it('offers the person’s own models that take tools, under Your models and by their own names', () => {
    expect(choices).toContain('models.filter((one) => one.own === true && one.chatOnly !== true)')
    expect(choices).toContain("own === undefined ? runtimeNameOf(runtime) : 'Your models'")
    // A name that is only the model's id reads spelled out, as its column does (0.810).
    expect(choices).toContain('shownRouteLabel({ model: own.id, label: own.displayName }) ?? own.displayName')
  })

  it('still offers a free model first, and the catalogue before any model of the person’s own', () => {
    expect(choices.indexOf('freeAnswered')).toBeLessThan(choices.indexOf('one.own === true'))
    expect(choices.indexOf('const catalogue')).toBeLessThan(choices.indexOf('one.own === true'))
  })
})
