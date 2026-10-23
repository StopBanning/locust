import { describe, expect, it } from 'vitest'

import { encryptPush, senderKeysForTest } from './web-push.js'

/**
 * AN ALERT IS LOCKED FOR ITS PHONE ALONE (0.827). A phone's alert is encrypted on the PC to the browser's own push
 * key, exactly as RFC 8291 sets out, so the relay and the push service pass on what they cannot read. Held to the
 * RFC's own worked example, byte for byte.
 */
const RFC = {
  auth: 'BTBZMqHH6r4Tts7J_aSIgg',
  receiverPublic: 'BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4',
  senderPrivate: 'yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw',
  senderPublic: 'BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8',
  body: 'DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN',
  text: 'When I grow up, I want to be a watermelon'
}

describe('an alert is locked for its phone alone', () => {
  it('makes the RFC\'s own example body from its keys and salt', async () => {
    const salt = new Uint8Array(Buffer.from(RFC.body, 'base64url').subarray(0, 16))
    const keys = await senderKeysForTest(RFC.senderPrivate, RFC.senderPublic)
    const body = await encryptPush({ p256dh: RFC.receiverPublic, auth: RFC.auth }, RFC.text, { salt, keys })
    expect(Buffer.from(body).toString('base64url')).toBe(RFC.body)
  })

  it('uses a fresh salt and key for every alert', async () => {
    const one = await encryptPush({ p256dh: RFC.receiverPublic, auth: RFC.auth }, 'Mira needs your approval')
    const two = await encryptPush({ p256dh: RFC.receiverPublic, auth: RFC.auth }, 'Mira needs your approval')
    expect(Buffer.from(one.subarray(0, 16)).equals(Buffer.from(two.subarray(0, 16)))).toBe(false)
    expect(Buffer.from(one.subarray(21, 86)).equals(Buffer.from(two.subarray(21, 86)))).toBe(false)
  })

  it('refuses keys that are not a browser\'s push keys', async () => {
    await expect(encryptPush({ p256dh: 'AAAA', auth: RFC.auth }, 'x')).rejects.toThrow('not a P-256 public key')
    await expect(encryptPush({ p256dh: RFC.receiverPublic, auth: 'AAAA' }, 'x')).rejects.toThrow('not 16 bytes')
  })
})
