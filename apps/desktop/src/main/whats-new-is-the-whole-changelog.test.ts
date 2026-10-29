import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { entries, splashEntries } from './changelog.js'

/**
 * WHAT'S NEW IS THE WHOLE CHANGELOG, AND A SPLASH IS FOR A BIG BUILD ONLY.
 *
 * Colin, 2026-09-23, with a frame of Claude Code's What's new: "we can really
 * get this and just introduce a proper changelog the way claude code does, if
 * we have really good big updates where the user has to know things, we can
 * have a splash page on update" -- and, of the home banner, "it adds a
 * needless scrollbar on that title menu".
 */

const FILE = [
  '# Changelog',
  '',
  '## 0.281.0 - 2026-09-23',
  '',
  '### New',
  '',
  "- **What's new, in Settings.** Every version.",
  '',
  '### Improved',
  '',
  '- **No banner.** Gone.',
  '',
  '## 0.280.0 - 2026-09-23',
  '',
  '- **Antigravity asks here.** Written before the groups.',
  '',
  '## 0.277.0 - 2026-09-22',
  '<!-- big -->',
  '',
  '- **Every teammate is a bot.**',
  '',
  '## 0.276.0 - 2026-09-22',
  '',
  '- **Dialogs fit.**'
].join('\n')

describe('an entry', () => {
  it("keeps its groups -- New, Improved, Fixed -- as Claude Code's What's new does", () => {
    const [newest] = entries(FILE)
    expect(newest?.groups).toEqual([
      { label: 'New', text: "- **What's new, in Settings.** Every version." },
      { label: 'Improved', text: '- **No banner.** Gone.' }
    ])
  })

  it('written before the groups, stays as it was written: one group, no label guessed for it', () => {
    expect(entries(FILE)[1]?.groups).toEqual([{ text: '- **Antigravity asks here.** Written before the groups.' }])
  })

  it('marked big says so, and the mark itself is nowhere in what is shown', () => {
    const bot = entries(FILE).find((entry) => entry.version === '0.277.0')
    expect(bot?.big).toBe(true)
    expect(bot?.body).not.toContain('<!--')
    expect(JSON.stringify(bot?.groups)).not.toContain('<!--')
    expect(entries(FILE).filter((entry) => entry.big).map((entry) => entry.version)).toEqual(['0.277.0'])
  })
})

describe('the splash', () => {
  const all = entries(FILE)

  it('shows the big builds between the one last seen and the one running', () => {
    expect(splashEntries(all, '0.276.0', '0.281.0').map((entry) => entry.version)).toEqual(['0.277.0'])
  })

  it('shows nothing when nothing big came since, on the same version, or on a first install', () => {
    expect(splashEntries(all, '0.277.0', '0.281.0')).toEqual([])
    expect(splashEntries(all, '0.281.0', '0.281.0')).toEqual([])
    expect(splashEntries(all, undefined, '0.281.0')).toEqual([])
  })

  it('shows nothing when the version last seen is not in the file -- a splash about the wrong builds is worse', () => {
    expect(splashEntries(all, '9.9.9', '0.281.0')).toEqual([])
    expect(splashEntries(all, '0.280.0', '0.276.0')).toEqual([])
  })
})

describe('the changelog that ships', () => {
  const shipped = readFileSync(fileURLToPath(new URL('../../../../CHANGELOG.md', import.meta.url)), 'utf8')
  const all = entries(shipped)

  it('is read whole: hundreds of builds, each with something to say', () => {
    expect(all.length).toBeGreaterThan(300)
    expect(all.every((entry) => entry.groups.length > 0)).toBe(true)
  })

  it("gives someone arriving from 0.276 the bots, the machine, the design pass, their own model, a room that remembers, pages that run each runtime's own commands a monochrome Locust, hand-off chains, landing a teammate's branch, comparing models, comparing their work and building to compare, pets, talking with ChatGPT's voice, the voice using their connectors, talking to a teammate by voice, starting Talk from any app, Scout keeping watch, their own cloud computer, their own benchmark, Claude Code and Codex on their cloud computer, the work carried on when a plan runs out, Locust on their phone, and agents of their own; from 0.777 nothing", () => {
    // 0.295.0 is big too: the title screen became a machine (A2). 0.350.0:
    // the first-impressions design pass -- Home leads with the team, a
    // conversation reads like Claude's (Colin, 2026-09-26). 0.357.0: Add your
    // own model, for a company that has one (Colin, the same day). 0.370.0:
    // a room remembers what was said -- the first thing the rooms research of
    // the same day said to build. 0.425.0: a web page a teammate made runs
    // inside Locust (Colin, 2026-09-28: "full functionality, sacrifice nothing").
    // 0.426.0: Claude Code's own slash commands in the / menu (Colin, the
    // same day: "the nerdier coders live by their commands"). 0.434.0: the
    // lime accent goes monochrome (Colin, the same day).
    // 0.440.0: a teammate's own branch lands on yours as one commit -- idea #2
    // of the product suggestions, which Colin picked the same day.
    // 0.441.0: compare models side by side (Colin, the same day: "works for me!").
    // 0.445.0: a comparison can compare the work -- each model edits its own copy,
    // Keep brings one in (Colin, the same day: "a room where you can compare work").
    // 0.448.0: build and compare from Home, in any folder (Colin, the same day:
    // "Keep working big dog", to a starter on Home).
    // 0.458.0: folders like Claude Code -- switch without a restart, every folder's
    // conversations by project (Colin, 2026-09-29: "Just make it work exactly like Claude code").
    // 0.462.0: ask on the side -- a question on a copy of a conversation (Devin's side chats).
    // 0.474.0: projects like Claude's, and math drawn (Colin, 2026-09-29: "claude has projects/and workspace folders").
    // 0.506.0: the Finances dashboard, from dropped statements.
    // 0.503.0: cloud tasks from the chat-type menu (Codex Cloud).
    // 0.501.0: the Finances place (a tester's ask: Codex's Finances, inside Locust).
    // 0.498.0: edit an earlier message and start again from there (Claude Code's rewind).
    // 0.563.0: a teammate can be a pet (Colin, 2026-10-03: "theyre just going to be added to the list of potential choices for teammates").
    // 0.737.0: Talk -- a voice call with ChatGPT's voice, placed by Codex (Colin, 2026-10-10: "can we make it accessible through locust?").
    // 0.746.0: Talk to a teammate -- the call is with them, in their voice, into their conversation (Colin, 2026-10-11: "just talk to the teammate directly with the voice").
    // 0.742.0: Talk uses your connectors, asking before one changes anything (Colin, 2026-10-11: "want to be able to do simple things like check gmail by asking").
    // 0.756.0: Scout, a teammate that keeps watch (Colin, 2026-10-12: "let's just use the best free model").
    // 0.755.0: Talk from any app, and a summary after each call (Colin, 2026-10-12: "I'll run all your suggestions").
    // 0.761.0: your cloud computer -- an OpenCode teammate's turn on a GitHub Codespace of your own (Colin, 2026-10-12: "yes you can also try out cloud teammates").
    // 0.840.0: a routine step that waits for your OK, and approvals that say in plain words what a command does (Colin, 2026-10-29: "You can ship and update if neeeded").
    // 0.839.0: a teammate's own browser sign-ins, carrying on past a provider error, and a calm shimmer for a working chat (Colin, 2026-10-28: "Alright keep working big dog", the OpenBot scrub's takes 2 and 3).
    // 0.838.0: Locust in any browser by signing in with GitHub, Open in your browser, and /locust within a minute (Colin, 2026-10-28: "i say why not all 3").
    // 0.837.0: write from your phone while the computer is off, and what needs you reaches your chat app (Colin, 2026-10-26: "you can just take over").
    // 0.834.0: Locust in a browser opens at once on what it last showed, and routines as Markdown files (Colin, 2026-10-24: "Right keep working", the Rakazo scrub's takes 4 and 5).
    // 0.833.0: keys a teammate needs, and Antigravity's wait on a stuck helper said (Colin, 2026-10-24: "Alright have fun big dog", to the docket).
    // 0.832.0: your teammates in Discord and Slack, in one Chat apps section (Colin, 2026-10-23: "id be way more intrigued for discord and id download slack").
    // 0.831.0: your teammates in Telegram, and a new teammate interviews you (Colin, 2026-10-23: "Alright sounds good", to the Rakazo scrub's first takes).
    // 0.830.0: Locust in a browser, the whole window driven by your computer (Colin, 2026-10-23: "Sounds good", to step 3 of "Away from your desk").
    // 0.829.0: Scout looks while your computer is off, and its cards are on your phone (Colin, 2026-10-23: "ill just trust your judgement for implemetation", step 2 of the design pass "Away from your desk").
    // 0.828.0: Scout prepares the fix before you look, and says what it saw (Colin, 2026-10-23: "alright sounds good", to the spec "Scout comes to you first", Releases 3 and 4).
    // 0.826.0: your phone reaches Locust from anywhere, through Locust's relay (Colin, 2026-10-23: "ill just trust your judgement for implemetation, sounds safe and good for me", to the design pass for the phone, Scout away and the web client).
    // 0.806.0: a rail of your own messages, and what fills a Claude conversation (Colin, 2026-10-19: "any work you want to pass off to sol or grok?").
    // 0.804.0: references and pull request states (Colin, 2026-10-19: "we can pass off some work to sol as well", the rival scan's last gaps).
    // 0.799.0: start from a GitHub issue, OpenCode and Cursor imports, and Home on one screen (Colin, 2026-10-19: "is there a way to fit all this in one screen with no scrolling").
    // 0.798.0: point at a part of a page, and connectors off until ticked (Colin, 2026-10-19: "sounds like the logical decision for best user experience right?").
    // 0.797.0: more than one account per agent, ! and your own slash commands (Colin, 2026-10-19: "lets run with your reccomendations", the OpenCode 2 gaps).
    // 0.796.0: a browser teammates can use, beside the conversation (Colin, 2026-10-18: "alright ill run with all your suggestions", roadmap item 5).
    // 0.794.0: a terminal beside the conversation (Colin, 2026-10-18: "alright ill run with all your suggestions", roadmap item 4).
    // 0.789.0: Locust runs on Linux, and has a light mode (Colin, 2026-10-18: "alright ill run with all your suggestions", to the gaps OpenCode showed).
    // 0.777.0: any Agent Client Protocol agent, added by its command line (Colin, 2026-10-15: "alright sounds good i trust you brother", to any ACP agent as a teammate).
    // 0.775.0: Locust on your phone, paired by a code (Colin, 2026-10-15: "alright sounds good i trust you brother", to the phone view).
    // 0.774.0: when a plan runs out, the work carries on with another agent (Colin, 2026-10-15: "alright sounds good i trust you brother", to the fallback chain).
    // 0.768.0: Claude Code and Codex on your cloud computer, signed in with one click (Colin, 2026-10-13: "We 100% want the other agents on the cloud computer").
    // 0.763.0: your own benchmark -- a comparison its judge scored, run again and read as one table (the canvas 56riCzZ3ZzR5AQ2sg6ssJm; Colin, of its calls: "trust your judgement").
    // 0.491.0: a turn reads like Claude Code's, its steps as lines between what was said (Colin, 2026-09-30: "ALL of our commands ... get rolled into the bar").
    const newest = all[0]!.version
    expect(splashEntries(all, '0.276.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0', '0.370.0', '0.357.0', '0.350.0', '0.295.0', '0.277.0'])
    expect(splashEntries(all, '0.280.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0', '0.370.0', '0.357.0', '0.350.0', '0.295.0'])
    expect(splashEntries(all, '0.295.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0', '0.370.0', '0.357.0', '0.350.0'])
    expect(splashEntries(all, '0.350.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0', '0.370.0', '0.357.0'])
    expect(splashEntries(all, '0.357.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0', '0.370.0'])
    expect(splashEntries(all, '0.370.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0', '0.425.0'])
    expect(splashEntries(all, '0.425.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0', '0.426.0'])
    expect(splashEntries(all, '0.426.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0', '0.434.0'])
    expect(splashEntries(all, '0.434.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0', '0.435.0'])
    expect(splashEntries(all, '0.435.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0', '0.440.0'])
    expect(splashEntries(all, '0.440.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0', '0.441.0'])
    expect(splashEntries(all, '0.441.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0', '0.445.0'])
    expect(splashEntries(all, '0.445.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0', '0.448.0'])
    expect(splashEntries(all, '0.448.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0', '0.458.0'])
    expect(splashEntries(all, '0.458.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0', '0.462.0'])
    expect(splashEntries(all, '0.462.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0', '0.474.0'])
    expect(splashEntries(all, '0.474.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0', '0.491.0'])
    expect(splashEntries(all, '0.491.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0', '0.498.0'])
    expect(splashEntries(all, '0.498.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0', '0.501.0'])
    expect(splashEntries(all, '0.501.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0', '0.503.0'])
    expect(splashEntries(all, '0.503.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0', '0.506.0'])
    expect(splashEntries(all, '0.506.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0', '0.563.0'])
    expect(splashEntries(all, '0.563.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0', '0.737.0'])
    expect(splashEntries(all, '0.737.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0', '0.742.0'])
    expect(splashEntries(all, '0.742.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0', '0.746.0'])
    expect(splashEntries(all, '0.746.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0', '0.755.0'])
    expect(splashEntries(all, '0.755.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0', '0.756.0'])
    expect(splashEntries(all, '0.756.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0', '0.761.0'])
    expect(splashEntries(all, '0.761.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0', '0.763.0'])
    expect(splashEntries(all, '0.763.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0', '0.768.0'])
    expect(splashEntries(all, '0.768.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0', '0.774.0'])
    expect(splashEntries(all, '0.774.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0', '0.775.0'])
    expect(splashEntries(all, '0.775.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0', '0.777.0'])
    expect(splashEntries(all, '0.777.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0', '0.789.0'])
    expect(splashEntries(all, '0.789.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0', '0.794.0'])
    expect(splashEntries(all, '0.794.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0', '0.796.0'])
    expect(splashEntries(all, '0.796.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0', '0.797.0'])
    expect(splashEntries(all, '0.797.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0', '0.798.0'])
    expect(splashEntries(all, '0.798.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0', '0.799.0'])
    expect(splashEntries(all, '0.799.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0', '0.804.0'])
    expect(splashEntries(all, '0.804.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0', '0.806.0'])
    expect(splashEntries(all, '0.806.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0', '0.826.0'])
    expect(splashEntries(all, '0.826.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0', '0.828.0'])
    expect(splashEntries(all, '0.828.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0', '0.829.0'])
    expect(splashEntries(all, '0.829.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0', '0.830.0'])
    expect(splashEntries(all, '0.830.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0', '0.831.0'])
    expect(splashEntries(all, '0.831.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0', '0.832.0'])
    expect(splashEntries(all, '0.832.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0', '0.833.0'])
    expect(splashEntries(all, '0.833.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0', '0.834.0'])
    expect(splashEntries(all, '0.834.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0', '0.835.0'])
    expect(splashEntries(all, '0.835.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0', '0.836.0'])
    expect(splashEntries(all, '0.836.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0', '0.837.0'])
    expect(splashEntries(all, '0.837.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0', '0.838.0'])
    expect(splashEntries(all, '0.838.0', newest).map((entry) => entry.version)).toEqual(['0.840.0', '0.839.0'])
    expect(splashEntries(all, '0.839.0', newest).map((entry) => entry.version)).toEqual(['0.840.0'])
    expect(splashEntries(all, '0.840.0', newest)).toEqual([])
  })
})

describe('the home banner', () => {
  it('is gone: What changed lives in Settings, and a big build gets the splash', () => {
    const app = readFileSync(fileURLToPath(new URL('../renderer/src/App.tsx', import.meta.url)), 'utf8')
    const screens = readFileSync(fileURLToPath(new URL('../renderer/src/components/Screens.tsx', import.meta.url)), 'utf8')
    expect(app).not.toContain('WhatChangedBanner')
    expect(screens).not.toContain('Here is what changed')
    expect(app).toContain('<WhatsNewSplash')
    expect(screens).toContain("shownPage === 'whatsnew'")
  })
})
