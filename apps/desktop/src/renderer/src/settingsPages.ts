import type { IconName } from './components/Icon.js'

/**
 * Settings as PAGES, not one scroll.
 *
 * Colin asked for this twice. The first time (2026-09-14) the answer was to
 * group thirteen subjects into five named areas, still in one column, and he
 * said again on 2026-09-17: "our settings is a literal disaster". It was: the
 * areas were right and you still had to scroll past four of them to reach the
 * fifth, and nothing told you the fifth existed.
 *
 * So the areas become pages with a list beside them, which is the shape
 * Claude Code uses and the one he pointed at. Nothing about the settings
 * themselves changes -- same sections, same order, same words behind `More`.
 * What changes is that you can see the whole map at once and land on any of
 * it in one press.
 *
 * The headings are listed here so the search can find a setting by its own
 * name rather than only by the page it lives on. `settings-pages-are-real`
 * holds this list to the headings the file actually renders, because an index
 * that drifts is worse than no index: it would quietly stop finding things.
 *
 * `alsoKnownAs` is the OTHER half, and it exists because an index of headings
 * only finds the words WE chose. Grok's pass 9 typed four words a person who
 * had just used the app would type -- memory, worktree, node, ledger -- and
 * every one was answered "Nothing matches. The pages are still here". All
 * four settings exist. Memory is under "What your team remembers"; worktrees
 * are under "Project folder"; the ledger is a definition on "Privacy & local
 * data". Being told a thing does not exist is worse than having to scroll for
 * it, because a person stops looking.
 *
 * Each word is filed UNDER THE HEADING IT LEADS TO, so a search for a word
 * that is nowhere on the screen can still answer with the place it lives:
 * type "memory", get "What your team remembers". A flat list of page-level
 * words would find the page and leave the person to hunt it.
 *
 * These are deliberately not headings. Renaming "What your team remembers" to
 * "Memory" would make the screen read like a search index, and the heading is
 * the better sentence. The two lists are kept apart so the drift guard goes
 * on meaning exactly what it means.
 */
/*
 * ELEVEN PAGES IN FIVE GROUPS, each with an icon (0.393).
 *
 * Colin, 2026-09-27, with a screenshot of Claude's own settings: "we should
 * just make our settings like claude codes, way cleaner and more organized
 * than ours and has icons". Claude files each subject on a page of its own
 * under a small group label, with an icon beside every name. Five pages that
 * each held two to four unrelated subjects became eleven that each hold what
 * their name says; the sections themselves are unchanged.
 */
export type SettingsPageId =
  | 'app'
  | 'chat'
  | 'appearance'
  | 'privacy'
  | 'teammates'
  | 'memory'
  | 'relay'
  | 'runtimes'
  | 'models'
  | 'connectors'
  | 'cloud'
  | 'workspace'
  | 'whatsnew'

/** The label a run of pages sits under in the list. */
export type SettingsGroup = 'Locust' | 'Team' | 'Agents' | 'This folder' | 'About'

export interface SettingsPage {
  readonly id: SettingsPageId
  readonly label: string
  readonly group: SettingsGroup
  readonly icon: IconName
  readonly headings: readonly string[]
  /**
   * Words a person types that are not the name of anything on the page,
   * keyed by the heading each one leads to. Every key must be one of
   * `headings` -- `settings-search-knows-the-words` holds that.
   */
  readonly alsoKnownAs?: Readonly<Record<string, readonly string[]>>
}

export const SETTINGS_PAGES: readonly SettingsPage[] = [
  {
    id: 'app',
    label: 'General',
    group: 'Locust',
    icon: 'settings',
    headings: ['Managed by your organization', 'Let your other AI apps use Locust', 'Your phone', 'Telegram', 'Voice typing', 'Talk from any app', 'Scout keeps watch', 'Startup', 'Updates', 'Report a problem', 'Help'],
    alsoKnownAs: {
      'Managed by your organization': ['organization', 'organisation', 'policy', 'administrator', 'IT', 'managed settings'],
      'Let your other AI apps use Locust': ['MCP server', 'stdio', 'Claude Code', 'Codex', 'Cursor', 'other apps'],
      // A page on the home network, paired by a QR code (0.775).
      'Your phone': ['phone', 'mobile', 'QR code', 'iPhone', 'Android', 'away from your desk'],
      // Your teammates in Telegram (0.831): your own bot, paired to your own chat.
      Telegram: ['Telegram', 'Telegram bot', 'BotFather', 'chat app', 'messaging', 'Slack', 'Discord'],
      'Voice typing': ['dictation', 'microphone', 'speech', 'whisper', 'transcription'],
      // The keys that start Talk from any app (0.755).
      'Talk from any app': ['shortcut', 'hotkey', 'keyboard shortcut', 'voice call', 'Ctrl+Alt+Space'],
      // A teammate that keeps watch (0.756).
      'Scout keeps watch': ['Scout', 'proactive', 'keeps watch', 'noticed', 'failing checks', 'review requests'],
      Startup: ['login item', 'tray', 'keep running', 'sign-in'],
      Updates: ['version', 'upgrade'],
      'Report a problem': ['bug', 'feedback', 'crash'],
      // Where to reach a person (0.593, R23): the address, the public issues page.
      Help: ['email', 'contact', 'support', 'issue']
    }
  },
  {
    id: 'chat',
    label: 'Chat',
    group: 'Locust',
    icon: 'message',
    headings: ['Your commands', 'How to write one'],
    alsoKnownAs: {
      'Your commands': ['custom command', 'command mode', 'markdown command'],
      'How to write one': ['command file', 'arguments placeholder']
    }
  },
  {
    id: 'appearance',
    label: 'Appearance',
    group: 'Locust',
    icon: 'palette',
    headings: ['Reply text size', 'Terminal faces', 'What a face says', 'Plush', 'Sidebar', 'Send button', 'Boot screen'],
    alsoKnownAs: {
      // 'font' moved to Reply text size, which is the only setting in this
      // app that changes one. The sidebar has never had a font control.
      Sidebar: ['theme', 'dark', 'light', 'colour', 'color', 'width'],
      // The words a person reaches for when a reply is too big or too small:
      // Colin's own were 'text size', and nobody searches for 'prose'.
      'Send button': ['metal', 'chromatic', 'silver', 'gold', 'shine', 'shimmer', 'hover', 'send', 'button', 'effect', 'bend', 'gooey', 'animation', 'motion'],
      'Reply text size': ['text', 'text size', 'font', 'font size', 'bigger', 'smaller', 'type', 'reading'],
      'Terminal faces': ['face', 'faces', 'screen', 'eyes', 'bot', 'bots', 'avatar', 'terminal', 'mascot', 'look'],
      // The legend (FaceLegend.tsx): what each face means.
      'What a face says': ['legend', 'expression', 'expressions', 'mood', 'moods', 'meaning', 'status', 'state', 'states'],
      Plush: ['plush', 'fur', 'furry', 'fabric', 'soft', 'fluffy', 'plastic', 'material', 'texture'],
      'Boot screen': ['splash', 'startup', 'launch']
    }
  },
  {
    id: 'privacy',
    label: 'Privacy & data',
    group: 'Locust',
    icon: 'shield',
    headings: ['Privacy & local data', 'Trash', 'Back up and restore'],
    alsoKnownAs: {
      // "ledger" is a word the app itself says to people, in the sentence
      // telling them where the record of a run lives -- and it was the
      // fourth miss.
      'Privacy & local data': ['ledger', 'log', 'logs', 'telemetry', 'privacy', 'where is my data'],
      // "recycle bin" is here because that is what Trash is called on the
      // platform this ships to.
      Trash: ['deleted', 'delete', 'restore', 'undo', 'recycle', 'recycle bin', 'bin'],
      // The words a person moving to a new laptop would type (0.614).
      'Back up and restore': ['backup', 'back up', 'export', 'import', 'new laptop', 'move', 'copy', 'save my data', 'transfer']
    }
  },
  {
    id: 'teammates',
    label: 'Teammates',
    group: 'Team',
    icon: 'users',
    headings: ['Swarm', 'Auto mode', 'Plans', 'Your skills', 'Skills from GitHub', 'Saved approvals'],
    alsoKnownAs: {
      Swarm: ['parallel', 'at once', 'concurrent'],
      'Auto mode': ['permission', 'approve', 'ask first', 'sandbox'],
      Plans: ['plan', 'steps'],
      'Your skills': ['skill', 'skills', 'claude code', 'plugin', 'slash'],
      'Skills from GitHub': ['github', 'repository', 'library', 'download', 'skill folder'],
      'Saved approvals': ['rule', 'rules', 'always', 'allow', 'deny', 'never', 'saved']
    }
  },
  {
    id: 'memory',
    label: 'Memory',
    group: 'Team',
    icon: 'brain',
    headings: ['What your team remembers'],
    alsoKnownAs: {
      // The one Grok said to do first.
      'What your team remembers': ['memory', 'remember', 'remembers', 'notes', 'recall']
    }
  },
  {
    id: 'relay',
    label: 'Between teammates',
    group: 'Team',
    icon: 'message',
    headings: ['Between teammates'],
    alsoKnownAs: {
      'Between teammates': ['relay', 'reply', 'replies', 'answer', 'message', 'messages', 'interrupt', 'role', 'roles', 'name', 'rename']
    }
  },
  {
    id: 'runtimes',
    label: 'AI agents',
    group: 'Agents',
    icon: 'terminal',
    headings: ['AI agents & accounts', 'What Locust can stop', 'When a plan runs out', 'Your own agents'],
    alsoKnownAs: {
      // "node" and "npm" are what a person types after the first screen has
      // just talked to them about Node. This is the page about the CLIs that
      // are installed with them.
      'AI agents & accounts': ['runtime', 'runtimes', 'node', 'node.js', 'npm', 'install', 'cli', 'model', 'models', 'sign in', 'account', 'api key', 'remote control', 'claude.ai'],
      // R9 (0.617): which agents ask first, so which a card can stop.
      // Not 'approve', 'permission', 'ask first' or 'always': Teammates has those.
      'What Locust can stop': ['approval', 'approvals', 'permissions', 'card', 'cards', 'gate', 'stop', 'safety', 'security'],
      // 0.774: the section that carries work on when a plan runs out, by the words a person looking for it types.
      'When a plan runs out': ['route', 'limit', 'quota', 'rate limit', 'usage', 'fallback', 'fallback chain', 'backup agent', 'carry on', 'ran out', 'out of credits'],
      // 0.777: any Agent Client Protocol agent, by its command line.
      'Your own agents': ['ACP', 'agent client protocol', 'add an agent', 'Goose', 'Qwen Code', 'Kimi', 'custom agent']
    }
  },
  {
    id: 'models',
    label: 'Your own models',
    group: 'Agents',
    icon: 'chip',
    headings: ['Your own models'],
    alsoKnownAs: {
      'Your own models': ['own model', 'custom model', 'add model', 'endpoint', 'base url', 'address', 'openai', 'compatible', 'ollama', 'vllm', 'lm studio', 'local model', 'company', 'key']
    }
  },
  {
    id: 'connectors',
    label: 'Connectors',
    group: 'Agents',
    icon: 'plug',
    headings: ['GitHub', 'Connectors', 'Add a connector'],
    alsoKnownAs: {
      // 0.720: the GitHub CLI's own account.
      GitHub: ['gh', 'git hub', 'sign in to github', 'github account', 'pull request', 'pull requests', 'push', 'source control'],
      Connectors: ['mcp', 'tools', 'server'],
      // 0.716: one connector, to every agent at once.
      'Add a connector': ['add mcp', 'mcp server', 'every agent', 'all agents']
    }
  },
  {
    // Your cloud computer (0.761): a GitHub Codespace for each repository a teammate works on in the cloud.
    id: 'cloud',
    label: 'Cloud computers',
    group: 'Agents',
    icon: 'cloud',
    headings: ['Cloud computers'],
    alsoKnownAs: {
      'Cloud computers': ['cloud', 'codespace', 'codespaces', 'vm', 'virtual machine', 'remote', 'cloud teammate', 'github codespaces', 'free hours']
    }
  },
  {
    id: 'workspace',
    label: 'Project folder',
    group: 'This folder',
    icon: 'folder',
    headings: ['Project folder'],
    alsoKnownAs: {
      // Own branches are set here, under the folder they branch from; and the check after edits, with what a failure does.
      'Project folder': ['worktree', 'worktrees', 'branch', 'branches', 'folder', 'path', 'directory', 'project', 'repo', 'workspace', 'check after edits', 'tests', 'npm test', 'lint', 'fix until it passes']
    }
  },
  {
    // The whole changelog, as Claude Code's What's new (Colin, 2026-09-23).
    id: 'whatsnew',
    label: "What's new",
    group: 'About',
    icon: 'spark',
    headings: ['Changelog'],
    alsoKnownAs: {
      Changelog: ['what’s new', "what's new", 'what changed', 'release notes', 'releases', 'history', 'versions', 'new']
    }
  }
]

/** Case- and punctuation-insensitive enough that "auto" finds "Auto mode". */
export function matches(text: string, query: string): boolean {
  return text.toLowerCase().includes(query.toLowerCase())
}

/**
 * The headings on this page that answer what was typed -- whether the person
 * used the heading's own words or one of the words it is also known by.
 */
export function matchedHeadings(page: SettingsPage, query: string): readonly string[] {
  return page.headings.filter(
    (heading) =>
      matches(heading, query) ||
      (page.alsoKnownAs?.[heading] ?? []).some((word) => matches(word, query))
  )
}

/** Whether this page answers what was typed at all. */
export function pageMatches(page: SettingsPage, query: string): boolean {
  return matches(page.label, query) || matchedHeadings(page, query).length > 0
}
