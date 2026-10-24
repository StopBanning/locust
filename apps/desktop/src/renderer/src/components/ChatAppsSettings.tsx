import { useState, type ReactElement, type ReactNode } from 'react'

import type { ChatAppApi, ChatAppId, ChatAppSettings } from '../../../shared/chat-apps.js'
import { DISCORD_DEVELOPER_PORTAL } from '../../../shared/discord.js'
import { policyFeatureRefusal } from '../../../shared/organization-policy.js'
import { SLACK_MANIFEST_LINK } from '../../../shared/slack.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'
import { ChatAppRow, openLink, PairingCode, useChatApp, useFreshPairing } from './ChatAppRow.js'
import { TelegramSettings } from './TelegramSettings.js'

/*
 * YOUR TEAMMATES IN CHAT APPS (0.831 Telegram; 0.832 Discord and Slack; shared/chat-apps.ts). One row an app, in the
 * order most people have them: off, each is a name and Set up; set up, each says its steps where they are done, then
 * the code or link that pairs the person's own account, then who is paired. A token goes in a masked field and comes
 * back from Locust never again -- only the bot's name does.
 */

/** Who is paired, with another account or Remove beside it. */
function PairedRow({ state, paired, busy, onAnother, onRemove }: {
  readonly state: ChatAppSettings
  readonly paired: string
  readonly busy: boolean
  readonly onAnother: () => void
  readonly onRemove: () => void
}): ReactElement | null {
  if (state.chat === undefined) return null
  return (
    <ul className="lc-phone__devices" aria-label="The paired account">
      <li className="lc-phone__device">
        <span className="lc-phone__who">
          <span className="lc-phone__name">{state.chat.name}{state.chat.username === undefined ? '' : ` (@${state.chat.username})`}</span>
          <span className="lc-phone__state">{state.connected ? paired : `${paired} · not connected now`}</span>
        </span>
        <button type="button" className="lc-button" disabled={busy} onClick={onAnother}>Pair another account</button>
        <button type="button" className="lc-button" disabled={busy} onClick={onRemove}>Remove</button>
      </li>
    </ul>
  )
}

/** Discord: an application of the person's own, its bot added to a server of theirs, and a code sent to it. */
export function DiscordSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.discord, initial, open: opened = false }: {
  readonly api?: ChatAppApi
  /** What to show before Locust has answered: a test draws each state from it. */
  readonly initial?: ChatAppSettings
  readonly open?: boolean
}): ReactElement {
  const { state, busy, failure, run } = useChatApp(api, initial)
  const [open, setOpen] = useState(opened)
  const [token, setToken] = useState('')
  useFreshPairing(api, state, run)
  const bot = state?.bot
  return (
    <ChatAppRow app="discord" name="Discord" state={state} open={open} onToggle={() => setOpen(!open)} failure={failure}>
      {bot === undefined && (
        <div className="lc-chatapp__steps" data-chat-step="token">
          <p className="lc-settings__note">1. In Discord’s Developer Portal, press New Application, name it (Locust will do), and press Create.</p>
          <span><button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(DISCORD_DEVELOPER_PORTAL)}>Open the Developer Portal</button></span>
          <p className="lc-settings__note">2. On its Bot page, press Reset Token (Discord may ask for your password) and copy the token. Paste it here. It is kept encrypted on this computer and sent only to Discord.</p>
          <form className="lc-chatapp__token" onSubmit={(event) => {
            event.preventDefault()
            const given = token
            void run(() => api!.setToken(given)).then(() => setToken(''))
          }}>
            <input className="lc-input" type="password" aria-label="Your bot’s token from Discord" autoComplete="off" spellCheck={false} value={token} disabled={busy}
              onChange={(event) => { setToken(event.target.value) }} placeholder="MTI…" />
            <button type="submit" className="lc-button" disabled={busy || token.trim().length === 0}>Connect</button>
          </form>
        </div>
      )}
      {bot !== undefined && state?.chat === undefined && (
        <div className="lc-chatapp__steps" data-chat-step="pair">
          <p className="lc-settings__note" role="status">{state?.connected === true ? `Connected to ${bot.username}. It reads only its direct messages.` : 'Connecting to Discord…'}</p>
          <p className="lc-settings__note">3. Add the bot to a server of yours. If you have none, make one first: in Discord, the + under your servers, then Create My Own.</p>
          {bot.invite !== undefined && <span><button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(bot.invite!)}>Add the bot to a server</button></span>}
          <p className="lc-settings__note">4. In that server, click the bot in the members list and send it this code in a message. It pairs your account, once, within ten minutes.</p>
          {state?.code !== undefined
            ? <PairingCode code={state.code.code} label="The code to send the bot" />
            : <span><button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.newLink()) }}>Make a pairing code</button></span>}
          <span className="lc-chatapp__buttons">
            {bot.open !== undefined && <button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(bot.open!)}>Open the bot in Discord</button>}
            <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove()) }}>Remove the bot</button>
          </span>
        </div>
      )}
      {bot !== undefined && state?.chat !== undefined && (
        <>
          <PairedRow state={state} paired={`Paired with ${bot.username}`} busy={busy} onAnother={() => { void run(() => api!.newLink()) }} onRemove={() => { void run(() => api!.remove()) }} />
          {state.code !== undefined && (
            <div className="lc-chatapp__steps" data-chat-step="pair-again">
              <p className="lc-settings__note">Send this code to the bot from the account that should take {state.chat.name}’s place, within ten minutes.</p>
              <PairingCode code={state.code.code} label="The code to send the bot" />
            </div>
          )}
          <p className="lc-settings__note">In your messages with the bot, type / to pick a teammate, or write a name and a colon: “Ada: check the build”. Discord’s servers can read what you and your teammates write there. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.</p>
        </>
      )}
    </ChatAppRow>
  )
}

/** Slack: an app of the person's own, made from Locust's manifest in one step, its two tokens, and a code sent to it. */
export function SlackSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.slack, initial, open: opened = false }: {
  readonly api?: ChatAppApi
  readonly initial?: ChatAppSettings
  readonly open?: boolean
}): ReactElement {
  const { state, busy, failure, run } = useChatApp(api, initial)
  const [open, setOpen] = useState(opened)
  const [botToken, setBotToken] = useState('')
  const [appToken, setAppToken] = useState('')
  useFreshPairing(api, state, run)
  const bot = state?.bot
  const where = bot?.team === undefined ? 'your workspace' : bot.team
  return (
    <ChatAppRow app="slack" name="Slack" state={state} open={open} onToggle={() => setOpen(!open)} failure={failure}>
      {bot === undefined && (
        <form className="lc-chatapp__steps" data-chat-step="token" onSubmit={(event) => {
          event.preventDefault()
          const given = { bot: botToken, app: appToken }
          void run(() => api!.setToken(given)).then(() => { setBotToken(''); setAppToken('') })
        }}>
          <p className="lc-settings__note">1. Make the app in Slack. This opens Slack with everything filled in: pick your workspace, press Next, then Create.</p>
          <span><button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(SLACK_MANIFEST_LINK)}>Create the app in Slack</button></span>
          <p className="lc-settings__note">2. In the app’s settings, open Install App, install it to your workspace, and press Allow. Copy the Bot User OAuth Token and paste it here.</p>
          <span className="lc-chatapp__token">
            <input className="lc-input" type="password" aria-label="The bot’s token from Slack" autoComplete="off" spellCheck={false} value={botToken} disabled={busy}
              onChange={(event) => { setBotToken(event.target.value) }} placeholder="xoxb-…" />
          </span>
          <p className="lc-settings__note">3. Then open Basic Information. Under App-Level Tokens, press Generate Token and Scopes, add the scope connections:write, and press Generate. Paste that token here. Both are kept encrypted on this computer and sent only to Slack.</p>
          <span className="lc-chatapp__token">
            <input className="lc-input" type="password" aria-label="The app’s own token from Slack" autoComplete="off" spellCheck={false} value={appToken} disabled={busy}
              onChange={(event) => { setAppToken(event.target.value) }} placeholder="xapp-…" />
            <button type="submit" className="lc-button" disabled={busy || botToken.trim().length === 0 || appToken.trim().length === 0}>Connect</button>
          </span>
        </form>
      )}
      {bot !== undefined && state?.chat === undefined && (
        <div className="lc-chatapp__steps" data-chat-step="pair">
          <p className="lc-settings__note" role="status">{state?.connected === true ? `Connected to the Locust app in ${where}.` : 'Connecting to Slack…'}</p>
          <p className="lc-settings__note">4. In Slack, open Locust under Apps in the sidebar, or search for it, and send it this code. It pairs your account, once, within ten minutes.</p>
          {state?.code !== undefined
            ? <PairingCode code={state.code.code} label="The code to send the Locust app" />
            : <span><button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.newLink()) }}>Make a pairing code</button></span>}
          <span className="lc-chatapp__buttons">
            {bot.open !== undefined && <button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(bot.open!)}>Open Locust in Slack</button>}
            <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove()) }}>Remove the app</button>
          </span>
        </div>
      )}
      {bot !== undefined && state?.chat !== undefined && (
        <>
          <PairedRow state={state} paired={`Paired in ${where}`} busy={busy} onAnother={() => { void run(() => api!.newLink()) }} onRemove={() => { void run(() => api!.remove()) }} />
          {state.code !== undefined && (
            <div className="lc-chatapp__steps" data-chat-step="pair-again">
              <p className="lc-settings__note">Send this code to the Locust app from the account that should take {state.chat.name}’s place, within ten minutes.</p>
              <PairingCode code={state.code.code} label="The code to send the Locust app" />
            </div>
          )}
          <p className="lc-settings__note">In Slack, write to the Locust app with a teammate’s name and a colon: “Ada: check the build”. Reply in an answer’s thread to carry on that conversation. Slack’s servers can read what you and your teammates write there. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.</p>
        </>
      )}
    </ChatAppRow>
  )
}

/** The section: what chat apps are for, then Discord, Slack and Telegram, each its own row. */
export function ChatAppsSettings({ heading, more, initial }: {
  readonly heading?: ReactNode
  readonly more?: ReactNode
  /** What each row shows before Locust has answered: a test draws each state from it. */
  readonly initial?: Partial<Record<ChatAppId, ChatAppSettings>>
}): ReactElement {
  const managed = policyFeatureRefusal(useOrganizationPolicy(), 'chatApps') !== undefined
  return (
    <>
      <div className="lc-settingline">
        <div className="lc-settingline__text">
          {heading}
          {managed && <ManagedPolicyNote />}
          {!managed && <p className="lc-settings__lede">Write to your teammates from Discord, Slack or Telegram, get their answers there, and answer their approval cards, while Locust is open on this computer.</p>}
        </div>
      </div>
      {!managed && (
        <ul className="lc-chatapps" aria-label="Chat apps">
          <DiscordSettings {...(initial?.discord === undefined ? {} : { initial: initial.discord })} />
          <SlackSettings {...(initial?.slack === undefined ? {} : { initial: initial.slack })} />
          <TelegramSettings {...(initial?.telegram === undefined ? {} : { initial: initial.telegram })} />
        </ul>
      )}
      {more}
    </>
  )
}
