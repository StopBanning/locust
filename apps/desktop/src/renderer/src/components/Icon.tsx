import type { ReactNode } from 'react'

/**
 * The shell's icon set. Extracted verbatim from the pre-redesign App so the
 * glyphs stay identical while everything around them is replaced.
 */
export type IconName =
  | 'mic'
  | 'activity'
  | 'thought'
  | 'arrow-up'
  | 'attachment'
  | 'check'
  | 'chevron-down'
  | 'chevron-right'
  | 'clock'
  | 'close'
  | 'code'
  | 'command'
  | 'copy'
  | 'diff'
  | 'download'
  | 'dots'
  | 'file'
  | 'folder'
  | 'grid'
  | 'inbox'
  | 'maximize'
  | 'message'
  | 'minimize'
  | 'pause'
  | 'pencil'
  | 'play'
  | 'refresh'
  | 'plus'
  | 'route'
  | 'search'
  | 'settings'
  | 'shield'
  | 'spark'
  | 'terminal'
  | 'users'
  // Settings' page list (0.393), from Lucide (ISC) in this set's stroke.
  | 'palette'
  | 'brain'
  | 'chip'
  | 'plug'
  // A comparison column given the width, and given it back (0.444), from Lucide's maximize-2 / minimize-2.
  | 'expand'
  | 'collapse'
  // Compare and Blind in the chat mode chip (0.451), from Lucide's columns-2 / eye-off.
  | 'columns'
  | 'eye-off'
  /** Scout keeps watch (0.756): the eye, open. */
  | 'eye'
  | 'target'
  | 'wallet'
  | 'cloud'
  // Your cloud computer for a Claude Code or Codex teammate (0.768), from Lucide's server.
  | 'server'
  // Commit (0.680), from Lucide's git-commit-horizontal.
  | 'commit'
  // A voice call (0.737), from Lucide's audio-lines and mic-off.
  | 'waveform'
  | 'mic-off'
  // A link (0.770), from Lucide's link.
  | 'link'
  | 'globe'
  | 'arrow-left'
  | 'arrow-right'

export function Icon({ name, size = 16 }: { name: IconName; size?: number }): ReactNode {
  const paths: Record<IconName, ReactNode> = {
    mic: <><rect x="9" y="2" width="6" height="12" rx="3" /><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8" /></>,
    // Lucide's activity, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    activity: <path d="M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2" />,
    // A thought bubble: the cloud, then the two trailing dots that make it a
    // thought and not speech. Colin, 2026-09-17: "use a little thought bubble".
    thought: (
      <>
        <path d="M9 3.5h6a5.5 5.5 0 0 1 0 11h-1.2l-2.3 2.3v-2.3H9a5.5 5.5 0 0 1 0-11Z" />
        <circle cx="6.5" cy="18.5" r="1.1" />
        <circle cx="3.6" cy="21.2" r=".7" />
      </>
    ),
    'arrow-up': <><path d="m6 10 6-6 6 6" /><path d="M12 4v16" /></>,
    attachment: <path d="m20.5 11.5-8.9 8.9a6 6 0 0 1-8.5-8.5l9.6-9.6a4 4 0 0 1 5.7 5.7l-9.6 9.6A2 2 0 1 1 6 14.8l8.9-8.9" />,
    check: <path d="m5 12 4 4L19 6" />,
    'chevron-down': <path d="m7 10 5 5 5-5" />,
    'chevron-right': <path d="m9 18 6-6-6-6" />,
    clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
    close: <><path d="m7 7 10 10" /><path d="M17 7 7 17" /></>,
    code: <><path d="m8 9-4 3 4 3" /><path d="m16 9 4 3-4 3" /><path d="m14 5-4 14" /></>,
    command: <path d="M9 6V5a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v14a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6Z" />,
    // Two sheets, the front one offset. Same 24-box, same 2px stroke as the
    // rest of the set; used by the command output foot to take all of it.
    copy: <><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1" /></>,
    dots: <><circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" /></>,
    // Two columns and a divider: a change with a before and an after.
    commit: <><circle cx="12" cy="12" r="3" /><path d="M3 12h6" /><path d="M15 12h6" /></>,
    waveform: <path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3" />,
    'mic-off': <><path d="M2 2l20 20" /><path d="M18.89 13.23A7.12 7.12 0 0 0 19 12v-2" /><path d="M5 10v2a7 7 0 0 0 12 5" /><path d="M15 9.34V5a3 3 0 0 0-5.68-1.33" /><path d="M9 9v3a3 3 0 0 0 5.12 2.12" /><path d="M12 19v3" /></>,
    diff: <><rect x="3" y="5" width="6" height="14" /><rect x="15" y="5" width="6" height="14" /><path d="M12 2v20" /></>,
    // Lucide's file, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    file: <><path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z" /><path d="M14 2v5a1 1 0 0 0 1 1h5" /></>,
    // The arrow into a tray: what every other client draws for "save this
    // somewhere". A desktop app copies rather than downloads, and the glyph
    // is still the one people know (Colin, 2026-09-20).
    download: <><path d="M12 4v10" /><path d="M8 11l4 4 4-4" /><path d="M5 19h14" /></>,
    // Lucide's folder, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    folder: <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />,
    grid: <><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></>,
    // Lucide's inbox, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    inbox: <><polyline points="22 12 16 12 14 15 10 15 8 12 2 12" /><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" /></>,
    maximize: <rect x="5" y="5" width="14" height="14" rx="1" />,
    // Lucide's message-square, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    message: <path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z" />,
    minimize: <path d="M5 12h14" />,
    // Lucide's pause, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    pause: <><rect x="14" y="3" width="5" height="18" rx="1" /><rect x="5" y="3" width="5" height="18" rx="1" /></>,
    // Lucide's play, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    play: <path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" />,
    // Lucide's rotate-cw, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    refresh: <><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" /><path d="M21 3v5h-5" /></>,
    plus: <><path d="M12 5v14" /><path d="M5 12h14" /></>,
    // Lucide's route, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    route: <><path d="M9 5h8.5a1 1 0 010 7h-11a1 1 0 000 7H15" /><circle cx="18" cy="19" r="3" /><circle cx="6" cy="5" r="3" /></>,
    search: <><circle cx="11" cy="11" r="7" /><path d="m16 16 4 4" /></>,
    // Lucide's settings, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    settings: <><path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" /><circle cx="12" cy="12" r="3" /></>,
    shield: <><path d="M12 3 5 6v5c0 4.7 2.8 8.2 7 10 4.2-1.8 7-5.3 7-10V6z" /><path d="m9 12 2 2 4-5" /></>,
    // Lucide's sparkles, in this set's stroke (0.879, 18 Nov: the first quick drawing replaced).
    spark: <><path d="M11.017 2.814a1 1 0 0 1 1.966 0l1.051 5.558a2 2 0 0 0 1.594 1.594l5.558 1.051a1 1 0 0 1 0 1.966l-5.558 1.051a2 2 0 0 0-1.594 1.594l-1.051 5.558a1 1 0 0 1-1.966 0l-1.051-5.558a2 2 0 0 0-1.594-1.594l-5.558-1.051a1 1 0 0 1 0-1.966l5.558-1.051a2 2 0 0 0 1.594-1.594z" /><path d="M20 2v4" /><path d="M22 4h-4" /><circle cx="4" cy="20" r="2" /></>,
    /*
     * THE ONE ADDITION, and it was asked for rather than assumed.
     *
     * The 2026-09-21 design brief drew Edit as a pencil and said so plainly:
     * `IconName` has 32 names -- `settings`, `dots`, `code` among them -- and
     * not one means edit. So a pencil is a new glyph, not a lookup, and the
     * brief offered a fallback (Edit keeps its word) in case it was not
     * wanted. Colin sent a second design showing the same pencil, so it is
     * wanted.
     *
     * Drawn in this set's idiom, not imported: the 24 box, 1.7 stroke and
     * round caps the wrapper already applies. The nib and the body are one
     * line; the short stroke is the ferrule, which is what stops it reading
     * as an arrow at 13px.
     */
    // Lucide's wallet, in this set's stroke (0.501).
    // Cloud tasks (0.503): Lucide's cloud, in this set's stroke.
    cloud: <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" />,
    wallet: <><path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1" /><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4" /></>,
    pencil: <><path d="M4 20h4L19.5 8.5a2.1 2.1 0 0 0-3-3L5 17z" /><path d="m14.5 6.5 3 3" /></>,
    terminal: <><path d="m5 7 4 4-4 4" /><path d="M11 16h8" /></>,
    /*
     * SETTINGS' PAGE LIST (0.393). Colin, 2026-09-27, holding Claude's own
     * settings up: "way cleaner and more organized than ours and has icons".
     * Four subjects had no glyph in this set -- how it looks, what the team
     * remembers, a model of your own, a connector -- so these four are
     * Lucide's (ISC licence), whose grid and round joins this set already
     * shares; the wrapper gives them its stroke.
     */
    columns: <><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M12 3v18" /></>,
    target: <><circle cx="12" cy="12" r="7" /><circle cx="12" cy="12" r="2" /><path d="M12 2v3" /><path d="M12 19v3" /><path d="M2 12h3" /><path d="M19 12h3" /></>,
    eye: <><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" /><circle cx="12" cy="12" r="3" /></>,
    'eye-off': <><path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" /><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" /><path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" /><path d="m2 2 20 20" /></>,
    expand: <><path d="M15 3h6v6" /><path d="M9 21H3v-6" /><path d="m21 3-7 7" /><path d="m3 21 7-7" /></>,
    collapse: <><path d="M4 14h6v6" /><path d="M20 10h-6V4" /><path d="m14 10 7-7" /><path d="m3 21 7-7" /></>,
    palette: <><circle cx="13.5" cy="6.5" r="1" fill="currentColor" stroke="none" /><circle cx="17.5" cy="10.5" r="1" fill="currentColor" stroke="none" /><circle cx="8.5" cy="7.5" r="1" fill="currentColor" stroke="none" /><circle cx="6.5" cy="12.5" r="1" fill="currentColor" stroke="none" /><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.65-.75 1.65-1.69 0-.44-.18-.84-.44-1.13-.29-.29-.44-.65-.44-1.13a1.64 1.64 0 0 1 1.67-1.67h2c3.05 0 5.55-2.5 5.55-5.55C21.97 6.01 17.46 2 12 2z" /></>,
    brain: <><path d="M12 5a3 3 0 1 0-5.997.125 4 4 0 0 0-2.526 5.77 4 4 0 0 0 .556 6.588A4 4 0 1 0 12 18Z" /><path d="M12 5a3 3 0 1 1 5.997.125 4 4 0 0 1 2.526 5.77 4 4 0 0 1-.556 6.588A4 4 0 1 1 12 18Z" /><path d="M15 13a4.5 4.5 0 0 1-3-4 4.5 4.5 0 0 1-3 4" /><path d="M12 5v13" /></>,
    chip: <><rect x="5" y="5" width="14" height="14" rx="2" /><rect x="9" y="9" width="6" height="6" rx="1" /><path d="M9 2v3" /><path d="M15 2v3" /><path d="M9 19v3" /><path d="M15 19v3" /><path d="M2 9h3" /><path d="M2 15h3" /><path d="M19 9h3" /><path d="M19 15h3" /></>,
    plug: <><path d="M12 22v-5" /><path d="M9 8V2" /><path d="M15 8V2" /><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z" /></>,
    server: <><rect x="2" y="2" width="20" height="8" rx="2" /><rect x="2" y="14" width="20" height="8" rx="2" /><path d="M6 6h.01" /><path d="M6 18h.01" /></>,
    link: <><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" /></>,
    globe: <><circle cx="12" cy="12" r="9" /><path d="M3 12h18" /><path d="M12 3a14.5 14.5 0 0 0 0 18 14.5 14.5 0 0 0 0-18" /></>,
    'arrow-left': <><path d="m12 19-7-7 7-7" /><path d="M19 12H5" /></>,
    'arrow-right': <><path d="M5 12h14" /><path d="m12 5 7 7-7 7" /></>,
    users: <><circle cx="9" cy="8" r="3" /><path d="M3.5 19a5.5 5.5 0 0 1 11 0" /><path d="M15 6.5a3 3 0 0 1 0 5.8" /><path d="M17 14a5 5 0 0 1 3.5 5" /></>
  }

  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      {paths[name]}
    </svg>
  )
}
