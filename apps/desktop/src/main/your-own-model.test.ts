import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { createOwnModelStore, isOwnRouteModel, ownRouteModel, testOwnEndpoint } from './own-models.js'
import type { SecretBox } from './own-models.js'

/**
 * YOUR OWN MODEL (0.357): kept, keyed, listed and tested.
 *
 * The key is the part that must be right. It is encrypted by the operating
 * system before it touches the disk, it is never handed to the window, and a
 * machine that cannot encrypt keeps no key rather than writing it in the
 * clear.
 */
const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

/** A stand-in for safeStorage that visibly changes the text, and can be switched off. */
const box = (available = true): SecretBox => ({
  available: () => available,
  encrypt: (text) => Buffer.from(`sealed:${[...text].reverse().join('')}`, 'utf8'),
  decrypt: (cipher) => [...cipher.toString('utf8').replace(/^sealed:/, '')].reverse().join('')
})

async function store(available = true) {
  const rootDirectory = await mkdtemp(join(tmpdir(), 'locust-own-models-'))
  roots.push(rootDirectory)
  let id = 0
  return {
    rootDirectory,
    models: createOwnModelStore({ rootDirectory, secrets: box(available), createId: () => `a1b2c3d${String(id++)}`, now: () => new Date('2026-09-26T09:00:00.000Z') })
  }
}

const ACME = { name: 'Acme Chat', baseUrl: 'https://llm.acme.example/v1/', model: 'acme-70b', key: 'sk-acme-secret' }

describe('a model of your own', () => {
  it('is kept with its key encrypted, and the window is told only that a key is kept', async () => {
    const { rootDirectory, models } = await store()
    const added = await models.add(ACME)
    expect(added).toEqual({ ownId: 'a1b2c3d0', name: 'Acme Chat', baseUrl: 'https://llm.acme.example/v1', model: 'acme-70b', hasKey: true, chatOnly: false, createdAt: '2026-09-26T09:00:00.000Z' })
    expect(JSON.stringify(await models.list())).not.toContain('sk-acme-secret')
    const onDisk = await readFile(join(rootDirectory, 'own-models.json'), 'utf8')
    expect(onDisk).not.toContain('sk-acme-secret')
    expect(onDisk).toContain(Buffer.from('sealed:terces-emca-ks', 'utf8').toString('base64'))
  })

  it('gives a run the provider it needs, key decrypted, and nothing for one no longer kept', async () => {
    const { models } = await store()
    const added = await models.add(ACME)
    const route = ownRouteModel(added)
    expect(route).toBe('own-a1b2c3d0/acme-70b')
    expect(isOwnRouteModel(route)).toBe(true)
    expect(await models.providerFor(route)).toEqual({
      id: 'own-a1b2c3d0',
      provider: { name: 'Acme Chat', baseUrl: 'https://llm.acme.example/v1', models: ['acme-70b'], apiKey: 'sk-acme-secret' }
    })
    await models.remove(added.ownId)
    expect(await models.providerFor(route)).toBeUndefined()
  })

  it('is in the model list under OpenCode, by its own name, marked as yours', async () => {
    const { models } = await store()
    await models.add({ ...ACME, key: undefined })
    expect(await models.catalog()).toEqual([
      { id: 'own-a1b2c3d0/acme-70b', runtime: 'opencode', displayName: 'Acme Chat', description: 'Your model · llm.acme.example', supportedEfforts: [], own: true }
    ])
  })

  it('that only chats gives its runs no tools, and says so in the list (0.358)', async () => {
    const { models } = await store()
    const plain = await models.add({ ...ACME, chatOnly: true })
    expect(plain.chatOnly).toBe(true)
    expect((await models.providerFor(ownRouteModel(plain)))?.provider.toolCalls).toBe(false)
    expect((await models.catalog())[0]?.description).toBe('Your model · llm.acme.example · chat only')
    // Marked as such too, so a judge is never one that cannot read a file (0.810).
    expect((await models.catalog())[0]?.chatOnly).toBe(true)
    // And back: a model found to take tools after all.
    expect((await models.setChatOnly(plain.ownId, false)).chatOnly).toBe(false)
    expect((await models.providerFor(ownRouteModel(plain)))?.provider.toolCalls).toBeUndefined()
    await expect(models.setChatOnly('nobody00', true)).rejects.toThrow(/no longer kept/)
  })

  it('keeps no key on a machine that cannot protect one -- and says so', async () => {
    const { models } = await store(false)
    await expect(models.add(ACME)).rejects.toThrow(/cannot protect a key/)
    // Without a key it is kept: a model on this machine usually needs none.
    expect((await models.add({ ...ACME, key: '' })).hasKey).toBe(false)
  })

  it('refuses what is not a name, an http(s) address or a model id, in words', async () => {
    const { models } = await store()
    await expect(models.add({ ...ACME, name: '' })).rejects.toThrow(/name/)
    await expect(models.add({ ...ACME, baseUrl: 'ftp://llm.example' })).rejects.toThrow(/http:\/\/ or https:\/\//)
    await expect(models.add({ ...ACME, model: 'two words' })).rejects.toThrow(/model is the id/)
    await models.add(ACME)
    await expect(models.add(ACME)).rejects.toThrow(/already have a model called Acme Chat/)
  })
})

describe('testing an endpoint', () => {
  /** An endpoint: its model list, and how it answers a request that carries a tool. */
  const endpoint = (list: { status: number; body: unknown }, chat: { status: number; body: unknown } = { status: 200, body: {} }): typeof fetch =>
    (async (url: string) => {
      const answer = url.endsWith('/models') ? list : chat
      return new Response(JSON.stringify(answer.body), { status: answer.status })
    }) as unknown as typeof fetch
  const answering = (status: number, body: unknown): typeof fetch => endpoint({ status, body }, { status: 500, body: {} })

  it('says it answered and serves the model -- and, when the one-word request fails, that it could not tell about tools', async () => {
    expect(await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'acme-70b' }, answering(200, { data: [{ id: 'acme-70b' }] }))).toEqual({
      ok: false,
      said: 'It answered, and serves acme-70b. A one-word request got 500 back, so Test could not tell whether it can use tools.'
    })
  })

  /*
   * A REAL SERVER ON THIS MACHINE (0.640). llama.cpp's server, a 3B model on
   * a CPU: the list came back at once, the one-word request with a tool in 8
   * s, past the 8 s the two shared -- and Test said only "It answered, and
   * serves qwen2.5-3b-instruct.", nothing about tools. The model gets its own
   * minute, and a Test that cannot tell says so.
   */
  describe('a model slow to answer', () => {
    const listed = { data: [{ id: 'm' }] }
    /** Lists at once; answers the one-word request after `delayMs`, or never, until it is given up on. */
    const slow = (delayMs: number | undefined): typeof fetch =>
      (async (url: string, init?: RequestInit) => {
        if (url.endsWith('/models')) return new Response(JSON.stringify(listed), { status: 200 })
        return new Promise<Response>((resolve, reject) => {
          const timer = delayMs === undefined ? undefined : setTimeout(() => resolve(new Response('{}', { status: 200 })), delayMs)
          init?.signal?.addEventListener('abort', () => {
            clearTimeout(timer)
            reject(new DOMException('This operation was aborted', 'AbortError'))
          })
        })
      }) as unknown as typeof fetch

    it('is waited for past the time the list is given', async () => {
      expect(await testOwnEndpoint({ baseUrl: 'http://127.0.0.1:8089/v1', model: 'm' }, slow(150), 50)).toEqual({ ok: true, said: 'It answered, and serves m. It can use tools.', tools: true })
    })

    it('gets a minute by default', async () => {
      const bounds: number[] = []
      const timeout = vi.spyOn(globalThis, 'setTimeout')
      await testOwnEndpoint({ baseUrl: 'http://127.0.0.1:8089/v1', model: 'm' }, slow(10))
      for (const call of timeout.mock.calls) if (typeof call[1] === 'number') bounds.push(call[1])
      timeout.mockRestore()
      expect(bounds).toContain(8000)
      expect(bounds).toContain(60_000)
    })

    it('that never answers is said, not left out', async () => {
      expect(await testOwnEndpoint({ baseUrl: 'http://127.0.0.1:8089/v1', model: 'm' }, slow(undefined), 50, 1000)).toEqual({
        ok: false,
        said: 'It answered, and serves m. It did not answer a one-word request in 1 second, so Test could not tell whether it can use tools.'
      })
    })

    it('that cannot be reached for the request is said too', async () => {
      const gone = (async (url: string) => {
        if (url.endsWith('/models')) return new Response(JSON.stringify(listed), { status: 200 })
        throw new TypeError('fetch failed')
      }) as unknown as typeof fetch
      expect((await testOwnEndpoint({ baseUrl: 'http://127.0.0.1:8089/v1', model: 'm' }, gone)).said).toBe(
        'It answered, and serves m. A one-word request to it failed (fetch failed), so Test could not tell whether it can use tools.'
      )
    })
  })

  it('says whether it takes tools, and finds a model that only chats (0.358)', async () => {
    const listed = { status: 200, body: { data: [{ id: 'acme-70b' }] } }
    expect(await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'acme-70b' }, endpoint(listed, { status: 200, body: { choices: [] } }))).toEqual({
      ok: true,
      said: 'It answered, and serves acme-70b. It can use tools.',
      tools: true
    })
    // Ollama's own words for a model without tool support.
    const refused = { status: 400, body: { error: { message: 'registry.ollama.ai/library/plain does not support tools' } } }
    expect(await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'acme-70b' }, endpoint(listed, refused))).toEqual({
      ok: true,
      said: 'It answered, and serves acme-70b. It cannot use tools, so it is set to chat only.',
      tools: false
    })
  })

  it('asks about tools with one request, capped at a single token', async () => {
    const bodies: string[] = []
    const recording = (async (url: string, init?: RequestInit) => {
      if (url.endsWith('/chat/completions')) bodies.push(String(init?.body))
      return new Response(JSON.stringify(url.endsWith('/models') ? { data: [{ id: 'm' }] } : {}), { status: 200 })
    }) as unknown as typeof fetch
    await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'm' }, recording)
    expect(bodies).toHaveLength(1)
    expect(JSON.parse(bodies[0]!)).toMatchObject({ model: 'm', max_tokens: 1, tools: [{ type: 'function' }] })
  })

  it('catches a model name the endpoint does not know, naming what it does', async () => {
    const said = await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'acme-7b' }, answering(200, { data: [{ id: 'acme-70b' }, { id: 'acme-8b' }] }))
    expect(said).toEqual({ ok: false, said: 'It answered, but does not list acme-7b. It lists acme-70b, acme-8b.' })
  })

  it('says a refused key is a refused key', async () => {
    expect((await testOwnEndpoint({ baseUrl: 'https://x/v1', model: 'm', key: 'bad' }, answering(401, {}))).said).toBe('It answered, and refused the key.')
  })

  it('sends the key as a bearer token, to that address only', async () => {
    const asked: { url: string; auth: string | undefined }[] = []
    const recording = (async (url: string, init?: RequestInit) => {
      asked.push({ url, auth: (init?.headers as Record<string, string> | undefined)?.authorization })
      return new Response(JSON.stringify({ data: [] }), { status: 200 })
    }) as unknown as typeof fetch
    await testOwnEndpoint({ baseUrl: 'https://llm.acme.example/v1', model: 'm', key: 'sk-1' }, recording)
    expect(asked).toEqual([
      { url: 'https://llm.acme.example/v1/models', auth: 'Bearer sk-1' },
      { url: 'https://llm.acme.example/v1/chat/completions', auth: 'Bearer sk-1' }
    ])
  })

  it('lists the models of Anthropic’s OpenAI-compatible endpoint, which wants a version header there', async () => {
    const anthropic = (async (url: string, init?: RequestInit) => {
      const headers = (init?.headers ?? {}) as Record<string, string>
      if (url.endsWith('/models') && headers['anthropic-version'] === undefined) {
        return new Response(JSON.stringify({ type: 'error', error: { type: 'invalid_request_error', message: 'anthropic-version: header is required' } }), { status: 400, statusText: 'Bad Request' })
      }
      return new Response(JSON.stringify(url.endsWith('/models') ? { data: [{ id: 'claude-haiku-5-5' }] } : { choices: [] }), { status: 200 })
    }) as unknown as typeof fetch
    expect(await testOwnEndpoint({ baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-5-5', key: 'sk-ant-x' }, anthropic)).toEqual({
      ok: true,
      said: 'It answered, and serves claude-haiku-5-5. It can use tools.',
      tools: true
    })
  })

  it('says it could not be reached, rather than throwing', async () => {
    const down = (async () => {
      throw new Error('connect ECONNREFUSED 127.0.0.1:11434')
    }) as unknown as typeof fetch
    expect((await testOwnEndpoint({ baseUrl: 'http://127.0.0.1:11434/v1', model: 'm' }, down)).said).toBe('It could not be reached: connect ECONNREFUSED 127.0.0.1:11434.')
  })
})
