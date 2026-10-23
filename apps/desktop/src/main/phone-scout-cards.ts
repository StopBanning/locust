import { PHONE_MAX_SCOUT_CARDS, type PhoneScoutCard } from '../shared/phone.js'
import type { ScoutCard } from '../shared/scout.js'

/**
 * SCOUT'S CARDS ON YOUR PHONE (0.829). Since 0.827 a Scout card that can't wait buzzes the phone, and the tap opened a
 * page that had no Scout on it. The phone now lists Scout's cards, newest first: Locust's sentence and lines, a link
 * to GitHub when the card is about something there, and Dismiss. What a card quotes (a waiting card's command), what
 * Scout's model said and a draft for a teammate stay in the window.
 */
export function phoneScoutCards(cards: readonly ScoutCard[]): readonly PhoneScoutCard[] {
  return [...cards]
    .sort((a, b) => b.at.localeCompare(a.at))
    .slice(0, PHONE_MAX_SCOUT_CARDS)
    .map((card) => {
      const url = card.open?.kind === 'url' && /^https:\/\/github\.com\/[\w./#?=&%-]+$/.test(card.open.url) ? card.open.url : undefined
      return {
        cardId: card.cardId,
        said: card.said.length > 400 ? `${card.said.slice(0, 399)}…` : card.said,
        ...(card.lines === undefined || card.lines.length === 0 ? {} : { lines: card.lines.slice(0, 8) }),
        now: card.urgency === 'now',
        at: card.at,
        ...(url === undefined ? {} : { url })
      }
    })
}
