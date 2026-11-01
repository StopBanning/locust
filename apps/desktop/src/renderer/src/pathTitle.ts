/**
 * A FIRST MESSAGE THAT IS ONLY A PATH (0.849) is titled by the file it names. Colin sends a handoff as its file's path
 * and nothing else, so seven sidebar rows read "/Users/locust/work/locust-move/..." and the column cut off the one part
 * that told them apart. Any other first line is the title as typed.
 */
export function pathOnlyTitle(line: string): string | undefined {
  const trimmed = line.trim()
  if (!/^(?:~|[A-Za-z]:)?[\\/]\S+$/.test(trimmed)) return undefined
  const parts = trimmed.split(/[\\/]+/).filter((part) => part.length > 0)
  return parts.length >= 2 ? parts[parts.length - 1] : undefined
}
