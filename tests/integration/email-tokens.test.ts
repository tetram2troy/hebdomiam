import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { emailTokens, users } from '../../server/database/schema'
import { consumeEmailToken, createEmailToken } from '../../server/utils/email-tokens'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('createEmailToken', () => {
  it('retourne un jeton brut et ne stocke que son empreinte', async () => {
    const user = await createTestUser(db)
    const raw = await createEmailToken(db, user.id, 'verify', 60)

    expect(raw).toMatch(/^[0-9a-f]{48}$/)

    const [row] = await db.select().from(emailTokens).where(eq(emailTokens.userId, user.id))
    expect(row!.tokenHash).not.toBe(raw)
    expect(row!.tokenHash).toMatch(/^[0-9a-f]{64}$/) // sha-256 hex
    expect(row!.usedAt).toBeNull()
  })
})

describe('consumeEmailToken', () => {
  it('consomme un jeton valide une seule fois (usage unique)', async () => {
    const user = await createTestUser(db)
    const raw = await createEmailToken(db, user.id, 'reset', 60)

    expect(await consumeEmailToken(db, 'reset', raw)).toBe(user.id)
    expect(await consumeEmailToken(db, 'reset', raw)).toBeNull()
  })

  it('refuse un jeton du mauvais type sans le consommer', async () => {
    const user = await createTestUser(db)
    const raw = await createEmailToken(db, user.id, 'verify', 60)

    expect(await consumeEmailToken(db, 'reset', raw)).toBeNull()
    // Toujours consommable avec le bon type : le refus n'a rien brûlé.
    expect(await consumeEmailToken(db, 'verify', raw)).toBe(user.id)
  })

  it('refuse un jeton expiré', async () => {
    const user = await createTestUser(db)
    const raw = await createEmailToken(db, user.id, 'reset', 60)

    const inTwoHours = new Date(Date.now() + 2 * 60 * 60_000)
    expect(await consumeEmailToken(db, 'reset', raw, inTwoHours)).toBeNull()
  })

  it('refuse un jeton inconnu', async () => {
    expect(await consumeEmailToken(db, 'reset', 'f'.repeat(48))).toBeNull()
  })
})

describe('cascade utilisateur', () => {
  it('supprime les jetons quand le compte disparaît', async () => {
    const user = await createTestUser(db)
    await createEmailToken(db, user.id, 'verify', 60)
    await createEmailToken(db, user.id, 'reset', 60)

    await db.delete(users).where(eq(users.id, user.id))

    const rows = await db.select().from(emailTokens)
    expect(rows).toHaveLength(0)
  })
})
