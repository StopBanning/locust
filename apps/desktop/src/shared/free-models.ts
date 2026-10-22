/**
 * OpenCode's free models that answered, steadiest first. Read fresh from
 * `https://opencode.ai/zen/v1/models` on 2026-10-18 (no sign-in), then one
 * short question ("What is 2+2? Answer with just the number.") asked of each
 * free model through OpenCode in a throwaway home, three tries each. Every
 * model below answered "4" on all three tries; ordered by average answer
 * time, fastest first: nemotron-3.5-lightning-free ~6.7s, space-bunny-free
 * ~7.2s, nemotron-3.5-super-vl-free ~7.2s, deepseek-v4.1-flash-free ~7.7s,
 * glm-5.3-flash-free ~8.2s, mimo-v2.6-flash-free ~8.4s, beam-free ~8.9s,
 * longcat-2.5-preview-free ~13.5s. Kept with _tools/sweep-drives.mjs
 * (FREE_MODELS) at each release's sweep.
 *
 * The order is not that one at the top. A new person's first model is the steadiest and the likeliest to
 * stay, not the quickest at one sum: deepseek-v4.1-flash-free first (Colin, 2026-10-19: "i heard space bunny
 * might be removed soon so maybe use deepseek"), Locust's own drives' model since 2026-10-18 and OpenCode's
 * most used free model that week (opencode.ai/data: 74T tokens), a named provider's model. space-bunny-free
 * second, the default from 0.756 to 0.794: OpenCode calls it a stealth model, free for a limited time, and
 * stealth models are the ones it retires (Fledge Alpha). The rest follow by answer time. A model OpenCode
 * stops listing is never offered, whatever its place here.
 *
 * Gone from OpenCode's free list since the 2026-10-09 sweep, so dropped:
 * nemotron-3-ultra-free, ling-3.1-flash-free, muse-spark-1.3-contributor-free.
 * Gone by the 2026-10-22 sweep (the picker listed nine free models, neither of
 * these), so dropped: space-bunny-free, longcat-2.5-preview-free. Listed then
 * and not yet asked, so not named: flint-40b-free, minimax-m3.1-flash-free,
 * nemotron-3-ultra-free (back). Not named either: jev-1.13-free, listed on
 * 2026-10-18 but answering nothing -- "Unexpected server error" on every one
 * of six tries.
 *
 * Shared since 0.756: the window picks a new person's free model from it, and
 * Scout falls back along it when its free model's provider will not answer.
 */
export const FREE_MODELS_BEST_FIRST: readonly string[] = [
  'opencode/deepseek-v4.1-flash-free',
  'opencode/nemotron-3.5-lightning-free',
  'opencode/nemotron-3.5-super-vl-free',
  'opencode/glm-5.3-flash-free',
  'opencode/mimo-v2.6-flash-free',
  'opencode/beam-free'
]
