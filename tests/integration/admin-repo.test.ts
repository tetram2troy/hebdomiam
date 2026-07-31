import { randomUUID } from 'node:crypto'
import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { recipes, users } from '../../server/database/schema'
import {
  createInvitedUser,
  deleteUserAsAdmin,
  listUsers,
  renewInvite,
  updateUserAsAdmin,
} from '../../server/utils/admin-repo'
import { consumeEmailToken } from '../../server/utils/email-tokens'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('listUsers', () => {
  it('retourne les comptes en ordre de création, champs de la liste blanche seulement', async () => {
    const alice = await createTestUser(db, 'Alice', { role: 'admin' })
    const bob = await createTestUser(db, 'Bob')

    const list = await listUsers(db)
    expect(list.map(u => u.id)).toEqual([alice.id, bob.id])
    expect(list[0]).toMatchObject({ name: 'Alice', role: 'admin', disabled: false, emailVerified: false })
    // Liste blanche stricte : jamais de hachage ni de données métier (D2).
    expect(Object.keys(list[0]!).sort()).toEqual(
      ['createdAt', 'disabled', 'email', 'emailVerified', 'id', 'name', 'role'],
    )
  })
})

describe('createInvitedUser', () => {
  it('crée un compte user non vérifié avec un jeton d\'invitation consommable', async () => {
    const result = await createInvitedUser(db, 'invite@example.com', 'Invitée')
    expect(result).not.toBe('emailTaken')
    if (result === 'emailTaken') return

    expect(result.user).toMatchObject({ email: 'invite@example.com', role: 'user', emailVerified: false, disabled: false })
    expect(await consumeEmailToken(db, 'invite', result.token)).toBe(result.user.id)
  })

  it('refuse un courriel déjà pris', async () => {
    const existing = await createTestUser(db, 'Déjà là')
    expect(await createInvitedUser(db, existing.email, 'Doublon')).toBe('emailTaken')
  })

  it('le mot de passe aléatoire n\'est pas déterministe entre deux comptes', async () => {
    const a = await createInvitedUser(db, 'a@example.com', 'A')
    const b = await createInvitedUser(db, 'b@example.com', 'B')
    if (a === 'emailTaken' || b === 'emailTaken') throw new Error('création inattendue en échec')
    const rows = await db.query.users.findMany({ columns: { passwordHash: true } })
    expect(rows[0]!.passwordHash).not.toBe(rows[1]!.passwordHash)
  })
})

describe('renewInvite', () => {
  it('génère un nouveau jeton pour un compte jamais activé', async () => {
    const created = await createInvitedUser(db, 'invite@example.com', 'Invitée')
    if (created === 'emailTaken') throw new Error('création inattendue en échec')

    const result = await renewInvite(db, created.user.id)
    expect(result).toMatchObject({ email: 'invite@example.com', locale: 'fr' })
    if (typeof result === 'string') return
    expect(await consumeEmailToken(db, 'invite', result.token)).toBe(created.user.id)
  })

  it('refuse pour un compte déjà activé', async () => {
    const active = await createTestUser(db, 'Active', { emailVerifiedAt: new Date() })
    expect(await renewInvite(db, active.id)).toBe('alreadyActive')
  })

  it('refuse pour un compte désactivé — l\'API est la frontière, pas l\'UI', async () => {
    const disabled = await createTestUser(db, 'Désactivée', { disabledAt: new Date() })
    expect(await renewInvite(db, disabled.id)).toBe('disabled')
  })

  it('invalide les jetons précédents : un seul lien d\'invitation vivant à la fois', async () => {
    const created = await createInvitedUser(db, 'invite@example.com', 'Invitée')
    if (created === 'emailTaken') throw new Error('création inattendue en échec')

    const renewed = await renewInvite(db, created.user.id)
    if (typeof renewed === 'string') throw new Error('renouvellement inattendu en échec')

    // L'ancien jeton est mort, le nouveau fonctionne.
    expect(await consumeEmailToken(db, 'invite', created.token)).toBeNull()
    expect(await consumeEmailToken(db, 'invite', renewed.token)).toBe(created.user.id)
  })

  it('refuse pour un compte inconnu', async () => {
    expect(await renewInvite(db, randomUUID())).toBe('notFound')
  })
})

describe('updateUserAsAdmin', () => {
  it('promeut, rétrograde, désactive et réactive un compte', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    const bob = await createTestUser(db, 'Bob')

    expect(await updateUserAsAdmin(db, actor.id, bob.id, { role: 'admin' })).toMatchObject({ role: 'admin' })
    expect(await updateUserAsAdmin(db, actor.id, bob.id, { role: 'user' })).toMatchObject({ role: 'user' })
    expect(await updateUserAsAdmin(db, actor.id, bob.id, { disabled: true })).toMatchObject({ disabled: true })
    expect(await updateUserAsAdmin(db, actor.id, bob.id, { disabled: false })).toMatchObject({ disabled: false })
  })

  it('re-désactiver un compte déjà désactivé conserve la date d\'origine', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    const origin = new Date('2026-01-01T00:00:00Z')
    const bob = await createTestUser(db, 'Bob', { disabledAt: origin })

    const result = await updateUserAsAdmin(db, actor.id, bob.id, { disabled: true })
    expect(result).toMatchObject({ disabled: true })
    const row = await db.query.users.findFirst({ where: eq(users.id, bob.id) })
    expect(row!.disabledAt).toEqual(origin)
  })

  it('refuse de se cibler soi-même (anti-lockout)', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    expect(await updateUserAsAdmin(db, actor.id, actor.id, { role: 'user' })).toBe('self')
  })

  it('refuse de rétrograder ou désactiver le dernier admin actif', async () => {
    const bob = await createTestUser(db, 'Bob')
    const lastAdmin = await createTestUser(db, 'Dernière admin', { role: 'admin', emailVerifiedAt: new Date() })

    expect(await updateUserAsAdmin(db, bob.id, lastAdmin.id, { role: 'user' })).toBe('lastAdmin')
    expect(await updateUserAsAdmin(db, bob.id, lastAdmin.id, { disabled: true })).toBe('lastAdmin')
  })

  it('un admin désactivé ne compte pas comme relève pour le garde lastAdmin', async () => {
    const bob = await createTestUser(db, 'Bob')
    const lastActive = await createTestUser(db, 'Dernière active', { role: 'admin', emailVerifiedAt: new Date() })
    await createTestUser(db, 'Admin désactivé', { role: 'admin', emailVerifiedAt: new Date(), disabledAt: new Date() })

    expect(await updateUserAsAdmin(db, bob.id, lastActive.id, { role: 'user' })).toBe('lastAdmin')
  })

  it('un admin jamais activé ne compte pas comme relève — promotion permise, lockout impossible', async () => {
    const bob = await createTestUser(db, 'Bob')
    const lastActive = await createTestUser(db, 'Dernière active', { role: 'admin', emailVerifiedAt: new Date() })
    const invited = await createInvitedUser(db, 'future-admin@example.com', 'Future admin')
    if (invited === 'emailTaken') throw new Error('création inattendue en échec')

    // Promouvoir un compte en attente d'activation est permis…
    expect(await updateUserAsAdmin(db, lastActive.id, invited.user.id, { role: 'admin' })).toMatchObject({ role: 'admin' })
    // …mais il ne peut pas se connecter : il n'autorise pas le retrait du
    // dernier admin réellement capable d'administrer.
    expect(await updateUserAsAdmin(db, bob.id, lastActive.id, { role: 'user' })).toBe('lastAdmin')
  })

  it('rétrograde un admin quand un autre admin actif existe', async () => {
    const actor = await createTestUser(db, 'Admin 1', { role: 'admin', emailVerifiedAt: new Date() })
    const other = await createTestUser(db, 'Admin 2', { role: 'admin', emailVerifiedAt: new Date() })

    expect(await updateUserAsAdmin(db, actor.id, other.id, { role: 'user' })).toMatchObject({ role: 'user' })
  })

  it('refuse un compte inconnu', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    expect(await updateUserAsAdmin(db, actor.id, randomUUID(), { disabled: true })).toBe('notFound')
  })
})

describe('deleteUserAsAdmin', () => {
  it('supprime le compte et ses données en cascade', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    const bob = await createTestUser(db, 'Bob')
    await db.insert(recipes).values({ userId: bob.id, title: 'Recette de Bob', baseServings: 4 })

    expect(await deleteUserAsAdmin(db, actor.id, bob.id)).toBe('ok')
    expect(await db.query.users.findFirst({ where: eq(users.id, bob.id) })).toBeUndefined()
    expect(await db.select().from(recipes)).toHaveLength(0)
  })

  it('refuse de se supprimer soi-même', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    expect(await deleteUserAsAdmin(db, actor.id, actor.id)).toBe('self')
  })

  it('refuse de supprimer le dernier admin actif', async () => {
    const bob = await createTestUser(db, 'Bob')
    const lastAdmin = await createTestUser(db, 'Dernière admin', { role: 'admin', emailVerifiedAt: new Date() })
    expect(await deleteUserAsAdmin(db, bob.id, lastAdmin.id)).toBe('lastAdmin')
  })

  it('refuse un compte inconnu', async () => {
    const actor = await createTestUser(db, 'Admin', { role: 'admin' })
    expect(await deleteUserAsAdmin(db, actor.id, randomUUID())).toBe('notFound')
  })
})
