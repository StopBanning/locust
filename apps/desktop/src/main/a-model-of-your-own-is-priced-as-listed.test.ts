import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'
import { withOpenCodeProviders } from '@teammate/runtime-adapters'

import { createPriceList, listedPrice, openCodePriceListPath } from './listed-prices.js'
import { createOwnModelStore, ownRouteModel } from './own-models.js'
import type { SecretBox } from './own-models.js'

/**
 * A MODEL OF YOUR OWN, PRICED AS ITS PROVIDER LISTS IT (0.810).
 *
 * Colin's benchmark of 10/21 ran Claude Haiku, Sonnet and Opus through his
 * own API key, and "Cost a task" read tokens for all three. OpenCode keeps
 * models.dev's list on this computer; a model at its provider's own address,
 * under the id the provider lists, is told its listed price, and OpenCode
 * reports its runs in dollars. Anything less than that exact match is not
 * priced, and reports tokens as before.
 */
const HAIKU = { input: 0.1, output: 0.5, cache_read: 0.01, cache_write: 0.125, tiers: [{ input: 0.5, output: 2.5, cache_read: 0.05, cache_write: 0.625, tier: { type: 'context', size: 100000 } }] }
const LIST = {
  anthropic: {
    id: 'anthropic',
    name: 'Anthropic',
    npm: '@ai-sdk/anthropic',
    models: {
      'claude-haiku-5-5': { id: 'claude-haiku-5-5', cost: HAIKU },
      'claude-sonnet-5-5': { id: 'claude-sonnet-5-5', cost: { input: 2, output: 10, cache_read: 0.1, cache_write: 2.5 } },
      'claude-unpriced': { id: 'claude-unpriced' }
    }
  },
  deepseek: { api: 'https://api.deepseek.com', models: { 'deepseek-chat': { cost: { input: 0.28, output: 0.42 } } } },
  // A pay-as-you-go account and a plan at one host, listing one model at two prices.
  zai: { api: 'https://api.z.ai/api/paas/v4', models: { 'glm-5': { cost: { input: 1, output: 3.2 } } } },
  'zai-coding-plan': { api: 'https://api.z.ai/api/coding/paas/v4', models: { 'glm-5': { cost: { input: 0, output: 0 } } } }
}

describe('the price a model of your own is told', () => {
  it('is its provider\'s listed one, at the provider\'s own address, tiers and all', () => {
    expect(listedPrice(LIST, 'https://api.anthropic.com/v1', 'claude-haiku-5-5')).toEqual(HAIKU)
    expect(listedPrice(LIST, 'https://API.anthropic.com/v1/', 'claude-sonnet-5-5')).toEqual({ input: 2, output: 10, cache_read: 0.1, cache_write: 2.5 })
    // A provider the list gives an address for is matched by it.
    expect(listedPrice(LIST, 'https://api.deepseek.com/v1', 'deepseek-chat')).toEqual({ input: 0.28, output: 0.42 })
  })

  it('is nothing anywhere less than an exact match', () => {
    // The same id at a gateway, a reseller, or a server of the person's own.
    expect(listedPrice(LIST, 'https://llm.acme.example/v1', 'claude-sonnet-5-5')).toBeUndefined()
    expect(listedPrice(LIST, 'http://127.0.0.1:11434/v1', 'claude-sonnet-5-5')).toBeUndefined()
    // An id the provider does not list, or lists with no price.
    expect(listedPrice(LIST, 'https://api.anthropic.com/v1', 'claude-sonnet-5-5-custom')).toBeUndefined()
    expect(listedPrice(LIST, 'https://api.anthropic.com/v1', 'claude-unpriced')).toBeUndefined()
    // No list, or a list that is not one.
    expect(listedPrice(undefined, 'https://api.anthropic.com/v1', 'claude-sonnet-5-5')).toBeUndefined()
    expect(listedPrice([], 'https://api.anthropic.com/v1', 'claude-sonnet-5-5')).toBeUndefined()
  })

  it('tells a plan and an account at one host apart by their paths, and prices neither when it cannot', () => {
    expect(listedPrice(LIST, 'https://api.z.ai/api/paas/v4', 'glm-5')).toEqual({ input: 1, output: 3.2 })
    expect(listedPrice(LIST, 'https://api.z.ai/api/coding/paas/v4', 'glm-5')).toEqual({ input: 0, output: 0 })
    expect(listedPrice(LIST, 'https://api.z.ai/other/v1', 'glm-5')).toBeUndefined()
  })

  it('drops what is not a price rather than passing it to OpenCode', () => {
    const odd = { anthropic: { models: { m: { cost: { input: 1, output: 'two' } }, n: { cost: { input: 1, output: 2, cache_read: -1, tiers: [{ input: 3 }, { input: 4, output: 5, tier: { type: 'context', size: 1000 } }] } } } } }
    expect(listedPrice(odd, 'https://api.anthropic.com/v1', 'm')).toBeUndefined()
    expect(listedPrice(odd, 'https://api.anthropic.com/v1', 'n')).toEqual({ input: 1, output: 2, tiers: [{ input: 4, output: 5, tier: { type: 'context', size: 1000 } }] })
  })

  it('is read from OpenCode\'s cache folder, once while the file is unchanged', async () => {
    expect(openCodePriceListPath({}, 'C:/Users/someone')).toBe(join('C:/Users/someone', '.cache', 'opencode', 'models.json'))
    expect(openCodePriceListPath({ XDG_CACHE_HOME: '/var/cache/me' }, '/home/me')).toBe(join('/var/cache/me', 'opencode', 'models.json'))
    let reads = 0
    let at = 1
    const prices = createPriceList({
      path: 'models.json',
      modifiedAt: async () => at,
      readText: async () => {
        reads += 1
        return JSON.stringify(LIST)
      }
    })
    expect(await prices.priceAt('https://api.anthropic.com/v1', 'claude-sonnet-5-5')).toMatchObject({ input: 2 })
    expect(await prices.priceAt('https://api.anthropic.com/v1', 'claude-haiku-5-5')).toMatchObject({ input: 0.1 })
    expect(reads).toBe(1)
    at = 2
    await prices.priceAt('https://api.anthropic.com/v1', 'claude-haiku-5-5')
    expect(reads).toBe(2)
    // No list -- OpenCode never run here -- is no price, not an error.
    const none = createPriceList({ path: 'missing.json', modifiedAt: async () => { throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' }) } })
    expect(await none.priceAt('https://api.anthropic.com/v1', 'claude-sonnet-5-5')).toBeUndefined()
  })
})

const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})
const box: SecretBox = { available: () => true, encrypt: (text) => Buffer.from(text, 'utf8'), decrypt: (cipher) => cipher.toString('utf8') }

describe('a run of a model of your own', () => {
  it('carries the listed price into OpenCode\'s config, where OpenCode prices each step with it', async () => {
    const rootDirectory = await mkdtemp(join(tmpdir(), 'locust-own-priced-'))
    roots.push(rootDirectory)
    const models = createOwnModelStore({
      rootDirectory,
      secrets: box,
      createId: () => '0a1b2c3d',
      prices: createPriceList({ path: 'models.json', modifiedAt: async () => 1, readText: async () => JSON.stringify(LIST) })
    })
    const haiku = await models.add({ name: 'claude-haiku-5-5', baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-5-5', key: 'sk-test' })
    const found = await models.providerFor(ownRouteModel(haiku))
    expect(found?.provider.prices).toEqual({ 'claude-haiku-5-5': HAIKU })
    const config = JSON.parse(withOpenCodeProviders('{}', { [found!.id]: found!.provider }) ?? '{}')
    expect(config.provider['own-0a1b2c3d'].models).toEqual({ 'claude-haiku-5-5': { name: 'claude-haiku-5-5', cost: HAIKU } })
  })

  it('carries none for a model its provider does not list, and its config is as it was', async () => {
    const rootDirectory = await mkdtemp(join(tmpdir(), 'locust-own-unpriced-'))
    roots.push(rootDirectory)
    const models = createOwnModelStore({
      rootDirectory,
      secrets: box,
      createId: () => '0a1b2c3e',
      prices: createPriceList({ path: 'models.json', modifiedAt: async () => 1, readText: async () => JSON.stringify(LIST) })
    })
    const acme = await models.add({ name: 'Acme Chat', baseUrl: 'https://llm.acme.example/v1', model: 'acme-70b' })
    const found = await models.providerFor(ownRouteModel(acme))
    expect(found?.provider.prices).toBeUndefined()
    const config = JSON.parse(withOpenCodeProviders('{}', { [found!.id]: found!.provider }) ?? '{}')
    expect(config.provider['own-0a1b2c3e'].models).toEqual({ 'acme-70b': { name: 'acme-70b' } })
  })
})
