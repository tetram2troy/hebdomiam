import { createHash, randomBytes } from 'node:crypto'
import { and, eq, gt, isNull } from 'drizzle-orm'
import type { Db } from '../database/client'
import { emailTokens } from '../database/schema'

// Jetons de courriel : seul le SHA-256 est stocké — un vol de base ne permet
// pas de rejouer les liens. Usage unique, à expiration.

function hashToken(raw: string): string {
  return createHash('sha256').update(raw).digest('hex')
}

export async function createEmailToken(db: Db, userId: string, type: 'verify' | 'reset', ttlMinutes: number): Promise<string> {
  const raw = randomBytes(24).toString('hex') // 48 caractères
  await db.insert(emailTokens).values({
    userId,
    type,
    tokenHash: hashToken(raw),
    expiresAt: new Date(Date.now() + ttlMinutes * 60_000),
  })
  return raw
}

/** Consomme un jeton valide (bon type, non expiré, non utilisé) → userId. */
export async function consumeEmailToken(db: Db, type: 'verify' | 'reset', raw: string, now = new Date()): Promise<string | null> {
  const [token] = await db.update(emailTokens)
    .set({ usedAt: now })
    .where(and(
      eq(emailTokens.tokenHash, hashToken(raw)),
      eq(emailTokens.type, type),
      isNull(emailTokens.usedAt),
      gt(emailTokens.expiresAt, now),
    ))
    .returning({ userId: emailTokens.userId })
  return token?.userId ?? null
}

/** Base des liens de courriel : env explicite en prod, origine de la requête sinon. */
export function appBaseUrl(requestOrigin: string): string {
  return process.env.NUXT_PUBLIC_BASE_URL?.replace(/\/$/, '') || requestOrigin
}
