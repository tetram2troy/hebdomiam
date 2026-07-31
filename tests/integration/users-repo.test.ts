import { randomUUID } from 'node:crypto'
import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { users } from '../../server/database/schema'
import { createInvitedUser } from '../../server/utils/admin-repo'
import { createEmailToken } from '../../server/utils/email-tokens'
import { acceptInvitation, countUsers, getAccountStatus, registerUser } from '../../server/utils/users-repo'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()

const input = {
  email: 'premier@example.com',
  name: 'Première',
  passwordHash: '$argon2id$fake-hash-for-tests',
}

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('countUsers', () => {
  it('compte les comptes existants', async () => {
    expect(await countUsers(db)).toBe(0)
    await createTestUser(db)
    expect(await countUsers(db)).toBe(1)
  })
})

describe('registerUser (bootstrap)', () => {
  it('sur une instance vierge, le premier inscrit devient admin', async () => {
    const result = await registerUser(db, input)
    expect(result).toMatchObject({ email: 'premier@example.com', name: 'Première', role: 'admin' })
  })

  it('dès qu\'un compte existe, l\'inscription est fermée', async () => {
    await createTestUser(db)
    expect(await registerUser(db, input)).toBe('closed')
  })

  it('l\'inscription reste fermée même si le seul compte est désactivé', async () => {
    await createTestUser(db, 'Désactivé', { disabledAt: new Date() })
    expect(await registerUser(db, input)).toBe('closed')
  })
})

describe('acceptInvitation', () => {
  const NEW_HASH = '$argon2id$nouveau-hash-de-test'

  async function invited() {
    const result = await createInvitedUser(db, 'invitee@example.com', 'Invitée')
    if (result === 'emailTaken') throw new Error('création inattendue en échec')
    return result
  }

  it('fixe le mot de passe, marque le courriel vérifié et brûle le jeton', async () => {
    const { user, token } = await invited()

    expect(await acceptInvitation(db, token, NEW_HASH)).toEqual({ userId: user.id })
    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(row!.passwordHash).toBe(NEW_HASH)
    expect(row!.emailVerifiedAt).not.toBeNull()

    // Usage unique : rejouer le même jeton échoue.
    expect(await acceptInvitation(db, token, '$argon2id$autre')).toBe('invalidToken')
  })

  it('refuse un jeton inconnu', async () => {
    expect(await acceptInvitation(db, 'z'.repeat(48), NEW_HASH)).toBe('invalidToken')
  })

  it('un jeton survivant ne peut pas écraser le mot de passe d\'un compte actif', async () => {
    const { user, token } = await invited()
    await acceptInvitation(db, token, NEW_HASH)

    // Jeton « fantôme » encore valide (ex. courriel d'invitation résiduel).
    const ghost = await createEmailToken(db, user.id, 'invite', 60)
    expect(await acceptInvitation(db, ghost, '$argon2id$pirate')).toBe('alreadyActive')

    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(row!.passwordHash).toBe(NEW_HASH) // rien n'a changé
  })

  it('refuse un compte désactivé', async () => {
    const { user, token } = await invited()
    await db.update(users).set({ disabledAt: new Date() }).where(eq(users.id, user.id))
    expect(await acceptInvitation(db, token, NEW_HASH)).toBe('disabled')
  })
})

describe('getAccountStatus', () => {
  it('retourne le rôle et le statut de désactivation', async () => {
    const disabledAt = new Date('2026-01-01T00:00:00Z')
    const admin = await createTestUser(db, 'Admin', { role: 'admin' })
    const disabled = await createTestUser(db, 'Désactivée', { disabledAt })

    expect(await getAccountStatus(db, admin.id)).toEqual({ role: 'admin', disabledAt: null })
    expect(await getAccountStatus(db, disabled.id)).toEqual({ role: 'user', disabledAt })
  })

  it('retourne null pour un compte inconnu', async () => {
    expect(await getAccountStatus(db, randomUUID())).toBeNull()
  })
})
