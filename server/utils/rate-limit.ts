import type { H3Event } from 'h3'
import { resolveClientIp } from './client-ip'
import { logWarn } from './logger'

export interface RateLimitOptions {
  max: number
  windowMs: number
}

// Fenêtre glissante en mémoire — suffisant pour une instance unique (M0).
// Complété au périmètre par le middleware Traefik rate-limit-auth@file.
export function createRateLimiter(options: RateLimitOptions, now: () => number = Date.now) {
  const hits = new Map<string, number[]>()
  // Sans purge, chaque clé vue laisse une entrée pour toujours — mémoire non
  // bornée sur un conteneur limité. Balayage amorti : au plus un par fenêtre.
  let nextSweepAt = 0

  return {
    check(key: string): boolean {
      const timestamp = now()
      const windowStart = timestamp - options.windowMs
      if (timestamp >= nextSweepAt) {
        nextSweepAt = timestamp + options.windowMs
        for (const [k, times] of hits) {
          if (!times.some(t => t > windowStart)) hits.delete(k)
        }
      }
      const recent = (hits.get(key) ?? []).filter(t => t > windowStart)
      if (recent.length >= options.max) {
        hits.set(key, recent)
        return false
      }
      recent.push(timestamp)
      hits.set(key, recent)
      return true
    },
    /** Nombre de clés retenues — observable de la purge pour les tests. */
    size(): number {
      return hits.size
    },
  }
}

const limiters = new Map<string, ReturnType<typeof createRateLimiter>>()

export function assertRateLimit(event: H3Event, bucket: string, options: RateLimitOptions) {
  let limiter = limiters.get(bucket)
  if (!limiter) {
    limiter = createRateLimiter(options)
    limiters.set(bucket, limiter)
  }
  const ip = resolveClientIp(event)
  if (!limiter.check(ip)) {
    logWarn('security.rate_limited', { bucket, ip })
    throw createError({ statusCode: 429, message: 'errors.tooManyRequests' })
  }
}
