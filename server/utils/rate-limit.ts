import type { H3Event } from 'h3'

export interface RateLimitOptions {
  max: number
  windowMs: number
}

// Fenêtre glissante en mémoire — suffisant pour une instance unique (M0).
// Complété au périmètre par le middleware Traefik rate-limit-auth@file.
export function createRateLimiter(options: RateLimitOptions, now: () => number = Date.now) {
  const hits = new Map<string, number[]>()

  return {
    check(key: string): boolean {
      const timestamp = now()
      const windowStart = timestamp - options.windowMs
      const recent = (hits.get(key) ?? []).filter(t => t > windowStart)
      if (recent.length >= options.max) {
        hits.set(key, recent)
        return false
      }
      recent.push(timestamp)
      hits.set(key, recent)
      return true
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
  const ip = getRequestIP(event, { xForwardedFor: true }) ?? 'unknown'
  if (!limiter.check(ip)) {
    throw createError({ statusCode: 429, message: 'errors.tooManyRequests' })
  }
}
