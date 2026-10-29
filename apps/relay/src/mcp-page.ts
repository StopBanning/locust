import type { McpPage } from './mcp.js'

/**
 * THE PAGE AN APP SENDS THE PERSON TO (0.841, mcp.ts). It answers nothing itself: it says what to look for on the
 * computer, shows the six digits the card in Locust shows, and waits. Allowed there, it sends the person back to the
 * app with the code; denied, with the app's own word for no.
 */
const escape = (text: string): string => text.replace(/[&<>"']/g, (character) => `&#${String(character.charCodeAt(0))};`)

const STYLE = `
:root { color-scheme: light dark; --ground: #f6f6f4; --ink: #1c1c1a; --soft: #6a6a64; --line: #deded8; --accent: #1f6f43; }
@media (prefers-color-scheme: dark) { :root { --ground: #141413; --ink: #ededea; --soft: #9d9d96; --line: #2c2c29; --accent: #6fcf97; } }
* { box-sizing: border-box; }
body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: var(--ground); color: var(--ink);
  font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; padding: 24px 16px; }
main { width: 100%; max-width: 440px; }
h1 { font-size: 20px; margin: 0 0 12px; text-wrap: balance; }
p { margin: 0 0 12px; color: var(--soft); }
.match { font: 600 32px/1.2 ui-monospace, Consolas, monospace; letter-spacing: 0.12em; color: var(--ink); margin: 20px 0;
  padding: 16px; border: 1px solid var(--line); border-radius: 8px; text-align: center; }
.said { color: var(--ink); }
`

function page(title: string, body: string, script = ''): string {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">`
    + `<meta name="robots" content="noindex"><title>${escape(title)}</title><style>${STYLE}</style></head>`
    + `<body><main>${body}</main>${script === '' ? '' : `<script>${script}</script>`}</body></html>`
}

export function mcpPage(shown: Exclude<McpPage, { readonly page: 'redirect' }>): string {
  if (shown.page === 'waiting') {
    const name = escape(shown.client)
    const digits = `${shown.match.slice(0, 3)} ${shown.match.slice(3)}`
    // Polled every two seconds; the answer says where to go once the person answered the card.
    const script = `(function(){var ask=${JSON.stringify(shown.ask)};var said=document.getElementById('said');`
      + `function look(){fetch('wait?ask='+encodeURIComponent(ask),{cache:'no-store'}).then(function(r){return r.json()}).then(function(a){`
      + `if(a.state==='waiting'){setTimeout(look,2000);return}`
      + `if(a.to){said.textContent=a.state==='allowed'?'Allowed. Going back to the app...':'Denied. Going back to the app...';location.replace(a.to);return}`
      + `said.textContent=a.state==='expired'?'Nobody answered on the computer in time. Connect again from the app.':'This sign-in is over. Connect again from the app.'`
      + `}).catch(function(){setTimeout(look,4000)})}look()})()`
    return page('Connect to Locust',
      `<h1>${name} wants to use your Locust team</h1>`
      + `<p>Locust on your computer is showing a card for it now. Check the card shows these numbers, then choose Allow or Deny there.</p>`
      + `<div class="match" aria-label="The numbers on the card">${digits}</div>`
      + `<p id="said" class="said" role="status">Waiting for your answer on the computer...</p>`
      + `<p>Nothing is decided on this page. If you did not start this, choose Deny in Locust.</p>`, script)
  }
  if (shown.page === 'offline') {
    return page('Locust is not open', `<h1>Locust is not open on your computer</h1><p>The app can connect only while Locust is running there. Open Locust, then connect again from the app.</p>`)
  }
  if (shown.page === 'busy') {
    return page('Try again shortly', `<h1>Another sign-in is waiting</h1><p>Locust is already asking about one on your computer, or has been asked too often this hour. Answer that card, then connect again from the app.</p>`)
  }
  if (shown.page === 'off') {
    return page('Not taking apps', `<h1>This Locust does not take apps from anywhere</h1><p>In Locust, open Settings, then General, and under Let your other AI apps use Locust, turn on Use from anywhere. Then connect again from the app.</p>`)
  }
  return page('Cannot connect', `<h1>This sign-in cannot go on</h1><p>${escape(shown.page === 'bad' ? shown.words : 'Connect again from the app.')}</p>`)
}

export const mcpPageHeaders = (secure: boolean): Record<string, string> => ({
  'content-type': 'text/html; charset=utf-8',
  'cache-control': 'no-store',
  'x-robots-tag': 'noindex',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  ...(secure ? { 'strict-transport-security': 'max-age=31536000' } : {}),
  'content-security-policy': `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`
})
