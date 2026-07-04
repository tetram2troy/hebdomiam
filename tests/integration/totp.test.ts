import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { generateSync } from 'otplib'
import { users } from '../../server/database/schema'
import { hashUserPassword } from '../../server/utils/password'
import {
  activateTotp,
  disableTotp,
  isTotpEnabled,
  startTotpSetup,
  verifyTotpLogin,
} from '../../server/utils/totp'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()
const PASSWORD = 'mot-de-passe-tres-long-123'

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

/** Active complètement la 2FA pour un utilisateur de test. */
async function setupAndActivate(userId: string, email: string) {
  const { secret } = await startTotpSetup(db, userId, email)
  const result = await activateTotp(db, userId, generateSync({ secret }))
  if (typeof result === 'string') throw new Error(`activation inattendue : ${result}`)
  return { secret, backupCode: result.backupCode }
}

describe('startTotpSetup', () => {
  it('pose le secret sans activer la 2FA', async () => {
    const user = await createTestUser(db)
    const { secret, otpauthUrl } = await startTotpSetup(db, user.id, user.email)

    expect(secret.length).toBeGreaterThanOrEqual(16)
    expect(otpauthUrl).toContain('otpauth://totp/')
    expect(otpauthUrl).toContain('Hebdomiam')
    expect(await isTotpEnabled(db, user.id)).toBe(false)
  })

  it('remplace un setup précédent non activé', async () => {
    const user = await createTestUser(db)
    const first = await startTotpSetup(db, user.id, user.email)
    const second = await startTotpSetup(db, user.id, user.email)

    expect(second.secret).not.toBe(first.secret)
    // Seul le dernier secret est activable.
    expect(await activateTotp(db, user.id, generateSync({ secret: first.secret }))).toBe('invalidCode')
    expect(await activateTotp(db, user.id, generateSync({ secret: second.secret }))).not.toBeTypeOf('string')
  })
})

describe('activateTotp', () => {
  it('refuse sans setup préalable', async () => {
    const user = await createTestUser(db)
    expect(await activateTotp(db, user.id, '123456')).toBe('noSetup')
  })

  it('refuse un mauvais code sans activer', async () => {
    const user = await createTestUser(db)
    await startTotpSetup(db, user.id, user.email)

    expect(await activateTotp(db, user.id, '000000')).toBe('invalidCode')
    expect(await isTotpEnabled(db, user.id)).toBe(false)
  })

  it('active avec un bon code et retourne un code de secours', async () => {
    const user = await createTestUser(db)
    const { backupCode } = await setupAndActivate(user.id, user.email)

    expect(backupCode).toMatch(/^[0-9a-f]{12}$/)
    expect(await isTotpEnabled(db, user.id)).toBe(true)

    // Le code de secours n'est stocké que haché.
    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(row!.totpBackupHash).not.toBe(backupCode)
  })

  it('refuse une seconde activation quand la 2FA est déjà active', async () => {
    const user = await createTestUser(db)
    const { secret } = await setupAndActivate(user.id, user.email)
    expect(await activateTotp(db, user.id, generateSync({ secret }))).toBe('noSetup')
  })
})

describe('verifyTotpLogin', () => {
  it('accepte le code courant et refuse un mauvais code', async () => {
    const user = await createTestUser(db)
    const { secret } = await setupAndActivate(user.id, user.email)

    expect(await verifyTotpLogin(db, user.id, generateSync({ secret }))).toBe(true)
    expect(await verifyTotpLogin(db, user.id, '000000')).toBe(false)
  })

  it('refuse tout code tant que la 2FA n\'est pas activée', async () => {
    const user = await createTestUser(db)
    const { secret } = await startTotpSetup(db, user.id, user.email)

    expect(await verifyTotpLogin(db, user.id, generateSync({ secret }))).toBe(false)
  })

  it('accepte le code de secours une seule fois, le TOTP restant valide', async () => {
    const user = await createTestUser(db)
    const { secret, backupCode } = await setupAndActivate(user.id, user.email)

    expect(await verifyTotpLogin(db, user.id, backupCode)).toBe(true)
    // Consommé : un second essai échoue.
    expect(await verifyTotpLogin(db, user.id, backupCode)).toBe(false)
    // Mais l'authenticateur fonctionne toujours.
    expect(await verifyTotpLogin(db, user.id, generateSync({ secret }))).toBe(true)
  })
})

describe('disableTotp', () => {
  async function userWithRealPassword() {
    const [user] = await db.insert(users).values({
      email: `totp-${process.pid}-${Math.random().toString(36).slice(2)}@example.com`,
      passwordHash: await hashUserPassword(PASSWORD),
      name: 'Testeuse 2FA',
    }).returning({ id: users.id, email: users.email })
    return user!
  }

  it('refuse un mauvais mot de passe', async () => {
    const user = await userWithRealPassword()
    await setupAndActivate(user.id, user.email)

    expect(await disableTotp(db, user.id, 'mauvais-mot-de-passe')).toBe('invalidPassword')
    expect(await isTotpEnabled(db, user.id)).toBe(true)
  })

  it('désactive et purge secret et code de secours avec le bon mot de passe', async () => {
    const user = await userWithRealPassword()
    const { backupCode } = await setupAndActivate(user.id, user.email)

    expect(await disableTotp(db, user.id, PASSWORD)).toBe('ok')
    expect(await isTotpEnabled(db, user.id)).toBe(false)
    expect(await verifyTotpLogin(db, user.id, backupCode)).toBe(false)

    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(row!.totpSecret).toBeNull()
    expect(row!.totpBackupHash).toBeNull()
  })
})
