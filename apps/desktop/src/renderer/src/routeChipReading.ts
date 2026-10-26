/**
 * The one control's words: the model, then its effort, as `Haiku 5.5 · Medium`.
 *
 * The agent's mark sits in front of this. A model with nothing to choose has
 * no effort word, and the name stands alone.
 */
export function routeChipReading(model: string, effort: string | undefined): string {
  const name = model.trim()
  if (effort === undefined || effort.trim().length === 0) return name
  return `${name} · ${effort.trim()}`
}
