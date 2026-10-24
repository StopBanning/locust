/**
 * A SECRET IN WHAT A TEAMMATE REMEMBERS (0.394).
 *
 * A kept memory is pasted into every teammate's brief, on every runtime and
 * every provider -- free and third-party routes included -- and written to
 * the folder's .locust/memory.md. Until now the only thing standing between
 * a key and all of that was one sentence in the brief ("never secrets,
 * credentials"), which asks the model to behave. This refuses it at the door.
 *
 * The shapes are the ones a key announces itself by, chosen for precision
 * over reach: a false refusal costs a memory, and says why; a missed key
 * would be quietly repeated forever. The list follows agentmemory's
 * privacy.ts (Apache-2.0), reimplemented here from its idea, reviewed
 * read-only on 2026-09-27.
 */
const SHAPES: readonly { readonly kind: string; readonly pattern: RegExp }[] = [
  { kind: 'an Anthropic API key', pattern: /\bsk-ant-[A-Za-z0-9_-]{20,}/ },
  { kind: 'an OpenAI API key', pattern: /\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}/ },
  { kind: 'an AWS access key', pattern: /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/ },
  { kind: 'a Google API key', pattern: /\bAIza[0-9A-Za-z_-]{35}\b/ },
  { kind: 'a GitHub token', pattern: /\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{22,})/ },
  { kind: 'a GitLab token', pattern: /\bglpat-[A-Za-z0-9_-]{20,}/ },
  { kind: 'a Slack token', pattern: /\bxox[abprs]-[A-Za-z0-9-]{10,}/ },
  // 0.832, teammates in chat apps: the tokens a person pastes for them, and which a teammate must never repeat.
  { kind: 'a Slack app token', pattern: /\bxapp-\d-[A-Za-z0-9-]{10,}/ },
  { kind: 'a Discord bot token', pattern: /\b[MNO][A-Za-z0-9_-]{23,27}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,40}\b/ },
  { kind: 'a Telegram bot token', pattern: /\b\d{5,15}:AA[A-Za-z0-9_-]{30,40}\b/ },
  { kind: 'an npm token', pattern: /\bnpm_[A-Za-z0-9]{36}\b/ },
  { kind: 'a signed token (JWT)', pattern: /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/ },
  { kind: 'a private key', pattern: /-----BEGIN [A-Z ]*PRIVATE KEY-----/ },
  { kind: 'a bearer token', pattern: /\bBearer\s+[A-Za-z0-9._~+/-]{20,}/ },
  // A value written straight after its name: `password=hunter2hunter2`,
  // `api_key: 9f2c...`. Eight characters or more, so a line that only NAMES
  // the setting ("set the password in .env") is not taken for one.
  { kind: 'a password or key', pattern: /\b(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token)\s*[:=]\s*["']?[^\s"'<>]{8,}/i },
  /*
   * QA-2026-09-29 round 2, R2: what the shapes above let through, found by
   * writing secrets the way people say them.
   */
  // `postgres://admin:hunter2@db.internal/app` -- a login inside an address.
  { kind: 'a login inside an address', pattern: /\b[a-z][a-z0-9+.-]*:\/\/[^\s:/@]+:[^\s@/]+@/i },
  // An AWS secret access key: 40 characters of base64 near the word that names it.
  { kind: 'an AWS secret key', pattern: /\b(?:secret|aws)\b[^\n]{0,60}?(?<![A-Za-z0-9/+])(?=[A-Za-z0-9/+]*[0-9/+])(?=[A-Za-z0-9/+]*[a-z])(?=[A-Za-z0-9/+]*[A-Z])[A-Za-z0-9/+]{40}(?![A-Za-z0-9/+=])/i },
  // A passphrase said, whatever it is: "the ssh key passphrase is correct horse battery staple".
  { kind: 'a passphrase', pattern: /\bpassphrase\b[^.\n]{0,30}?\b(?:is|was)\s*[:=]?\s*\S{3,}|\bpassphrase\s*[:=]\s*\S{3,}/i }
]

/**
 * Said in words, a password is only one when its value looks like one: "the
 * password is hunter2hunter2" and "the admin login is admin / P@ssw0rd-2026!"
 * are refused, "the password is stored in the vault" is not (R2). A value
 * that looks like one holds a digit or a symbol and is six characters or more.
 */
const LOOKS_LIKE_A_PASSWORD = (value: string): boolean => value.length >= 6 && /[0-9!@#$%^&*_+=?~-]/.test(value) && !/^\.[A-Za-z]+$/.test(value)
function passwordSaid(text: string): boolean {
  for (const said of text.matchAll(/\b(?:password|passwd|pwd|pin)\b[^.\n]{0,30}?\b(?:is|was)\s+["'`]?([^\s"'`]+)/gi)) {
    if (LOOKS_LIKE_A_PASSWORD((said[1] ?? '').replace(/[.,;)]+$/, ''))) return true
  }
  for (const said of text.matchAll(/\b(?:login|logins|credentials?|username)\b[^\n]{0,40}?\S+\s*[/:]\s*([^\s"'`]+)/gi)) {
    if (LOOKS_LIKE_A_PASSWORD((said[1] ?? '').replace(/[.,;)]+$/, ''))) return true
  }
  return false
}

/** The Luhn check, and the card networks' opening digits, on a bare run of digits. */
function looksLikeACard(digits: string): boolean {
  let sum = 0
  for (let at = 0; at < digits.length; at += 1) {
    let digit = Number(digits[digits.length - 1 - at])
    if (at % 2 === 1) digit = digit * 2 > 9 ? digit * 2 - 9 : digit * 2
    sum += digit
  }
  // Luhn alone passes one long number in ten -- an order id, a timestamp --
  // so the number must also open the way a card network's do.
  const network = /^(?:4|5[1-5]|2[2-7]|3[47]|6(?:011|5))/.test(digits)
  return sum % 10 === 0 && network && !/^(\d)\1+$/.test(digits)
}

const CARD_RUN = /(?<![0-9])[0-9](?:[ -]?[0-9]){12,18}(?![0-9])/g

/**
 * A payment card number: 13 to 19 digits, spaced or not, that pass the Luhn
 * check every card number carries (R10). Personal details stay welcome --
 * Colin, 2026-09-28, scoped this refusal to "passwords and api keys/stuff
 * like that" -- so a phone number is kept; a card number is a key to money.
 */
function cardNumberIn(text: string): boolean {
  for (const run of text.matchAll(CARD_RUN)) {
    if (looksLikeACard(run[0].replace(/[^0-9]/g, ''))) return true
  }
  return false
}

/** What kind of secret the text holds, if it holds one. */
export function secretIn(text: string): string | undefined {
  const shape = SHAPES.find((candidate) => candidate.pattern.test(text))?.kind
  if (shape !== undefined) return shape
  if (passwordSaid(text)) return 'a password'
  if (cardNumberIn(text)) return 'a card number'
  return undefined
}

/**
 * WHAT A SAVED RECORD SAYS INSTEAD OF A KEY. "Save the record" writes command
 * output, diffs and the person's own words verbatim, to be sent to someone;
 * this is the scrub between the ledger and that file.
 *
 * One list, each line named for where it came from. `secretIn` (above) is the
 * door of memory and `redactSecrets` (packages/runtime-adapters/src/codex-events.ts)
 * is what evidence is scrubbed with; `shared/` does not import the adapters
 * package, so the patterns are copied here. Where the two disagree the wider
 * one is kept: a record is sent on, and a missed key is repeated for good. A
 * record may lose a harmless string that looks like a key; the raw record
 * keeps it.
 */
export const SECRET_SHAPED_TEXT_REMOVED = '[secret-shaped text removed]'

const SCRUB_SHAPES: readonly RegExp[] = [
  // redactSecrets: a whole private key, header to footer; secretIn: the header alone (a diff cut short).
  /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----(?:[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----)?/g,
  // redactSecrets (8+) and secretIn (10+): a signed token (JWT).
  /\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g,
  // redactSecrets (12+) and secretIn (20+): Anthropic and OpenAI keys (sk-ant-, sk-proj-, sk-svcacct-) and pk- keys.
  /\b(?:sk|pk)-[A-Za-z0-9_-]{12,}/g,
  // secretIn and redactSecrets: an AWS access key.
  /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g,
  // secretIn and redactSecrets: a Google API key.
  /\bAIza[0-9A-Za-z_-]{35}\b/g,
  // redactSecrets (20+; secretIn asks 30+, which a short token slips under) and secretIn: a GitHub token.
  /\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{22,})/g,
  // secretIn: a GitLab token.
  /\bglpat-[A-Za-z0-9_-]{20,}/g,
  // secretIn and redactSecrets: a Slack token.
  /\bxox[abprs]-[A-Za-z0-9-]{10,}/g,
  // secretIn (0.832): the tokens of the chat apps' own -- Slack's app token, a Discord bot's, a Telegram bot's.
  /\bxapp-\d-[A-Za-z0-9-]{10,}/g,
  /\b[MNO][A-Za-z0-9_-]{23,27}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,40}\b/g,
  /\b\d{5,15}:AA[A-Za-z0-9_-]{30,40}\b/g,
  // secretIn: an npm token.
  /\bnpm_[A-Za-z0-9]{36}\b/g,
  // redactSecrets: a Hugging Face token.
  /\bhf_[A-Za-z0-9]{30,}\b/g,
  // redactSecrets (8+, any case): a bearer token.
  /\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi,
  // secretIn: a login inside an address, `postgres://admin:hunter2@db.internal/app`.
  /\b[a-z][a-z0-9+.-]*:\/\/[^\s:/@]+:[^\s@/]+@/gi,
  // secretIn: an AWS secret access key, 40 characters of base64 near the word that names it.
  /\b(?:secret|aws)\b[^\n]{0,60}?(?<![A-Za-z0-9/+])(?=[A-Za-z0-9/+]*[0-9/+])(?=[A-Za-z0-9/+]*[a-z])(?=[A-Za-z0-9/+]*[A-Z])[A-Za-z0-9/+]{40}(?![A-Za-z0-9/+=])/gi,
  // secretIn's names plus redactSecrets' (refresh and id tokens, client secret, authorization, cookie,
  // credential): a value of eight characters or more written straight after the name. The optional
  // quote before the colon is redactSecrets' (0.489), for the JSON shape `"accessToken": "..."`.
  /\b(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|authorization|cookie|credential)["']?\s*[:=]\s*["']?[^\s"'<>,;}]{8,}/gi,
  // secretIn: a passphrase said, whatever it is.
  /\bpassphrase\b[^.\n]{0,30}?\b(?:is|was)\s*[:=]?\s*\S{3,}|\bpassphrase\s*[:=]\s*\S{3,}/gi
]

/** secretIn's spoken passwords and logins. Only the value is replaced, and only when it looks like one. */
const SPOKEN_PASSWORDS: readonly RegExp[] = [
  /\b(?:password|passwd|pwd|pin)\b[^.\n]{0,30}?\b(?:is|was)\s+["'`]?([^\s"'`]+)/gi,
  /\b(?:login|logins|credentials?|username)\b[^\n]{0,40}?\S+\s*[/:]\s*([^\s"'`]+)/gi
]

/**
 * The text with every secret-shaped piece replaced by `[secret-shaped text
 * removed]`, and how many pieces that was. Pure: the same text gives the same
 * answer, and scrubbed text scrubs to itself with nothing replaced.
 */
export function scrubSecrets(text: string): { readonly text: string; readonly replaced: number } {
  let replaced = 0
  let clean = text
  for (const shape of SCRUB_SHAPES) {
    clean = clean.replace(shape, () => {
      replaced += 1
      return SECRET_SHAPED_TEXT_REMOVED
    })
  }
  for (const spoken of SPOKEN_PASSWORDS) {
    clean = clean.replace(spoken, (match: string, value: string) => {
      const bare = value.replace(/[.,;)]+$/, '')
      // A value an earlier shape already replaced is not a second secret.
      if (bare.startsWith('[secret-shaped') || !LOOKS_LIKE_A_PASSWORD(bare)) return match
      replaced += 1
      return `${match.slice(0, match.length - value.length)}${SECRET_SHAPED_TEXT_REMOVED}${value.slice(bare.length)}`
    })
  }
  clean = clean.replace(CARD_RUN, (run: string) => {
    if (!looksLikeACard(run.replace(/[^0-9]/g, ''))) return run
    replaced += 1
    return SECRET_SHAPED_TEXT_REMOVED
  })
  return { text: clean, replaced }
}

/**
 * ABOUT YOU, TOO (0.433). The person's note is read by every teammate on
 * every provider, like a memory, and 0.423 left it unguarded. Colin,
 * 2026-09-28, on what memory should refuse: "honestly agents keep personal
 * details about the user to help understand them... maybe just passwords
 * and api keys/stuff like that." So personal details stay welcome; keys do
 * not. `from` names the teammate whose suggested line it was.
 */
export function aboutYouSecretRefusal(kind: string, from?: string): string {
  return from === undefined
    ? `That holds ${kind}, and About you is read by every teammate on every provider, so it was not saved. Keep keys in the runtime's own sign-in or an environment variable.`
    : `A line ${from} suggested for About you held ${kind}, so it was not put to you: About you is read by every teammate on every provider.`
}

/** The refusal, said the way the memory notice says every other one. */
export function secretRefusal(kind: string): string {
  return `It held ${kind}, and a memory is given to every teammate on every provider, so it was not kept. Keep keys in the runtime's own sign-in or an environment variable.`
}
