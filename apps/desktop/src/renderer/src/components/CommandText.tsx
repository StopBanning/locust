import type { ReactElement, ReactNode } from 'react'

/**
 * A COMMAND THAT WRAPS AT ITS SPACES (0.837). A browser may break a line after any hyphen, so a long command broke
 * as `powershell.exe" -` / `Command 'python -m pytest -q'`, on the approval card and on the Board: a flag cut from its
 * dash, read as two words (site frames, 2026-10-27). Each flag's dashes are held to its first letter; every other
 * place a line could break stays, and the text, and what a copy of it takes, is the command exactly as sent.
 */
export function CommandText({ text }: { readonly text: string }): ReactElement {
  const parts: ReactNode[] = []
  let last = 0
  for (const flag of text.matchAll(/(?<=^|\s)-+[^\s-]/g)) {
    parts.push(text.slice(last, flag.index), <span key={flag.index} className="lc-nobreak">{flag[0]}</span>)
    last = flag.index + flag[0].length
  }
  parts.push(text.slice(last))
  return <>{parts}</>
}
