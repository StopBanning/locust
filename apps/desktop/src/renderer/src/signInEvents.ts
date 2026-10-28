/**
 * Said on the window by a Sign in button when it has opened a sign-in window,
 * so the window asks about runtimes again the moment the person comes back
 * -- that is when the answer changes. See App.tsx's discovery effect.
 */
export const SIGN_IN_OPENED_EVENT = 'locust:sign-in-opened'

/**
 * Said by a "Get it" link when it opened an agent's own download page (0.838.x):
 * the next return to the window asks about every agent again, the ones not
 * installed too, since the person may have just installed one. Colin,
 * 2026-10-28: Kilo installed from a terminal, and Locust still said
 * "not installed" -- the re-checks only ever asked about installed agents.
 */
export const INSTALL_PAGE_OPENED_EVENT = 'locust:install-page-opened'
