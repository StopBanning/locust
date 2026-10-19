export function isTerminalToggle(event: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'metaKey' | 'key' | 'code'>): boolean {
  return event.ctrlKey && !event.altKey && !event.metaKey && (event.code === 'Backquote' || event.key === '`')
}
