import { describe, expect, it } from 'vitest'

import { deskNotePage, deskPage } from './desk-page.js'
import { phonePage } from './phone-page.js'

/**
 * A BROWSER OPENS LOCUST WITHOUT A CODE TO CARRY (0.838). Colin, 2026-10-28, of app.locust.lol/desk's "Pair this
 * browser first": "w/o this slight friction". The way in says what Locust is and every way in -- signing in with GitHub
 * where the relay offers it, a code, and where to get Locust; the phone page reads a GitHub ticket and Open in your
 * browser's code, and goes on to the full window.
 */
const scriptOf = (html: string): string => html.slice(html.indexOf('<script>') + '<script>'.length, html.lastIndexOf('</script>'))
const parses = (script: string): boolean => {
  try {
    // eslint-disable-next-line no-new-func -- parsed only, never run
    new Function(script)
    return true
  } catch {
    return false
  }
}

/** The page's own readCode, run against an address as a browser would hand it. */
function readCodeOf(hash: string): { readonly code: unknown; readonly cleared: boolean } {
  const script = scriptOf(phonePage({ via: 'relay' }))
  const body = script.slice(script.indexOf('function readCode()'), script.indexOf('function failAll('))
  let cleared = false
  // eslint-disable-next-line no-new-func -- the page's own function, run against a browser's address
  const run = new Function('location', 'history', `${body}; return readCode();`) as (location: unknown, history: unknown) => unknown
  const code = run({ hash, pathname: '/' }, { replaceState: () => { cleared = true } })
  return { code, cleared }
}

const ROOM = 'r'.repeat(22)
const KEY = 'k'.repeat(43)
const SECRET = 's'.repeat(22)
const TICKET = 't'.repeat(43)
const TOKEN = `gho_${'g'.repeat(36)}`

describe('a browser opens Locust without a code to carry', () => {
  it('reads a GitHub ticket, Open in your browser’s code and a plain code, and keeps none in the address', () => {
    expect(readCodeOf(`#g=${ROOM}.${KEY}.${TICKET}.${TOKEN}`)).toEqual({ code: { room: ROOM, pc: KEY, ticket: TICKET, github: TOKEN, desk: true }, cleared: true })
    expect(readCodeOf(`#p=${ROOM}.${KEY}.${SECRET}.d`)).toEqual({ code: { room: ROOM, pc: KEY, secret: SECRET, desk: true }, cleared: true })
    expect(readCodeOf(`#p=${ROOM}.${KEY}.${SECRET}`)).toEqual({ code: { room: ROOM, pc: KEY, secret: SECRET, desk: false }, cleared: true })
    // Anything else is no code, and is still cleared away.
    expect(readCodeOf(`#p=${ROOM}.${KEY}.${SECRET}.x`)).toEqual({ code: null, cleared: true })
    expect(readCodeOf(`#g=${ROOM}.${KEY}.${TICKET}`)).toEqual({ code: null, cleared: true })
    expect(readCodeOf(`#g=${ROOM}.${KEY}.${TICKET}.no<way>`)).toEqual({ code: null, cleared: true })
    expect(readCodeOf('#elsewhere')).toEqual({ code: null, cleared: false })
  })

  it('opens with every way in: GitHub where the relay offers it, a code, and where to get Locust', () => {
    const offered = deskPage({ github: true })
    expect(parses(scriptOf(offered))).toBe(true)
    const script = scriptOf(offered)
    expect(script).toContain('var GITHUB = true;')
    expect(script).toContain("'Sign in with GitHub'")
    expect(script).toContain("go.href = '/v1/github/start'")
    expect(script).toContain('Open in your browser opens it there')
    expect(script).toContain("get.href = 'https://locust.lol'")
    expect(script).not.toContain('Pair this browser first')
    // A relay with no GitHub shows no GitHub button.
    expect(scriptOf(deskPage())).toContain('var GITHUB = false;')
  })

  it('says what GitHub’s sign-in ended in on a page with no script, every word escaped', () => {
    const page = deskNotePage({ title: 'Signed in as <octo>', words: 'Close "this" tab & go', links: [{ label: 'Try again', href: '/desk/' }, { label: 'Laptop', href: `/#g=${ROOM}.${KEY}.${TICKET}.${TOKEN}` }] })
    expect(page).not.toContain('<script')
    expect(page).toContain('<h1>Signed in as &#60;octo&#62;</h1>')
    expect(page).toContain('Close &#34;this&#34; tab &#38; go')
    expect(page).toContain('<a class="go" href="/desk/">Try again</a><a class="go quiet" href="/#g=')
  })
})
