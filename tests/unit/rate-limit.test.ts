import { describe, expect, it } from 'vitest'
import { createRateLimiter } from '../../server/utils/rate-limit'

describe('createRateLimiter', () => {
  it('autorise jusqu’à max requêtes dans la fenêtre', () => {
    let clock = 0
    const limiter = createRateLimiter({ max: 3, windowMs: 60_000 }, () => clock)

    expect(limiter.check('ip-1')).toBe(true)
    expect(limiter.check('ip-1')).toBe(true)
    expect(limiter.check('ip-1')).toBe(true)
    expect(limiter.check('ip-1')).toBe(false)
    clock += 1000
    expect(limiter.check('ip-1')).toBe(false)
  })

  it('réautorise une fois la fenêtre écoulée', () => {
    let clock = 0
    const limiter = createRateLimiter({ max: 2, windowMs: 60_000 }, () => clock)

    limiter.check('ip-1')
    limiter.check('ip-1')
    expect(limiter.check('ip-1')).toBe(false)

    clock = 60_001
    expect(limiter.check('ip-1')).toBe(true)
  })

  it('isole les clés entre elles', () => {
    const limiter = createRateLimiter({ max: 1, windowMs: 60_000 }, () => 0)

    expect(limiter.check('ip-1')).toBe(true)
    expect(limiter.check('ip-1')).toBe(false)
    expect(limiter.check('ip-2')).toBe(true)
  })

  it('un refus ne consomme pas de jeton', () => {
    let clock = 0
    const limiter = createRateLimiter({ max: 2, windowMs: 1000 }, () => clock)

    limiter.check('ip-1') // t=0
    clock = 900
    limiter.check('ip-1') // t=900
    expect(limiter.check('ip-1')).toBe(false)

    // t=0 sort de la fenêtre : il reste un seul appel (t=900) → autorisé
    clock = 1500
    expect(limiter.check('ip-1')).toBe(true)
  })
})
