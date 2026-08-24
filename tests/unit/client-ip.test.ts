import type { H3Event } from 'h3'
import { describe, expect, it } from 'vitest'
import { resolveClientIp } from '../../server/utils/client-ip'

// Événement h3 minimal : getRequestHeader lit node.req.headers,
// getRequestIP lit context.clientAddress puis node.req.socket.remoteAddress.
function fakeEvent(headers: Record<string, string>, remoteAddress = '172.18.0.2'): H3Event {
  return {
    context: {},
    node: { req: { headers, socket: { remoteAddress } } },
  } as unknown as H3Event
}

describe('resolveClientIp', () => {
  it('préfère cf-connecting-ip (posé par Cloudflare) à tout le reste', () => {
    const event = fakeEvent({
      'cf-connecting-ip': '198.51.100.7',
      'x-forwarded-for': '1.2.3.4, 198.51.100.7',
    })
    expect(resolveClientIp(event, {})).toBe('198.51.100.7')
  })

  it('ignore X-Forwarded-For par défaut : son entrée gauche est fournie par le client', () => {
    const event = fakeEvent({ 'x-forwarded-for': '1.2.3.4' })
    expect(resolveClientIp(event, {})).toBe('172.18.0.2')
  })

  it('fait confiance à X-Forwarded-For seulement sous NUXT_TRUST_XFF (e2e)', () => {
    const event = fakeEvent({ 'x-forwarded-for': '203.0.113.31' })
    expect(resolveClientIp(event, { NUXT_TRUST_XFF: '1' })).toBe('203.0.113.31')
  })

  it('retombe sur l\'IP socket sans aucun en-tête', () => {
    expect(resolveClientIp(fakeEvent({}), {})).toBe('172.18.0.2')
  })

  it('rend "unknown" quand rien n\'est déterminable', () => {
    const event = fakeEvent({})
    ;(event as unknown as { node: { req: { socket: { remoteAddress?: string } } } })
      .node.req.socket.remoteAddress = undefined
    expect(resolveClientIp(event, {})).toBe('unknown')
  })
})
