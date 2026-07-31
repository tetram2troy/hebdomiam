import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { recipeInputSchema } from '../../shared/schemas/recipe'
import { recipes, users } from '../../server/database/schema'
import { hashUserPassword, verifyUserPassword } from '../../server/utils/password'
import { createRecipe } from '../../server/utils/recipe-repo'
import {
  changePassword,
  deleteAccount,
  getProfile,
  updateProfile,
} from '../../server/utils/settings-repo'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()
const PASSWORD = 'mot-de-passe-tres-long-123'

async function createUserWithPassword(role: 'admin' | 'user' = 'user') {
  const [user] = await db.insert(users).values({
    email: `settings-${process.pid}-${Math.random().toString(36).slice(2)}@example.com`,
    passwordHash: await hashUserPassword(PASSWORD),
    name: 'Testeuse Réglages',
    role,
    // Compte établi : le garde lastAdmin ne compte que les admins vérifiés.
    emailVerifiedAt: new Date(),
  }).returning({ id: users.id, email: users.email })
  return user!
}

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('getProfile', () => {
  it('retourne le profil avec les valeurs par défaut du schéma', async () => {
    const user = await createTestUser(db, 'Fabrice')
    const profile = await getProfile(db, user.id)

    expect(profile).toEqual({
      name: 'Fabrice',
      email: user.email,
      emailVerifiedAt: null,
      locale: 'fr',
      unitSystem: 'metric',
      defaultDishCount: 4,
      defaultPeopleCount: 1,
      defaultDaysCount: 7,
      rotationWeeks: 3,
      totpEnabled: false,
    })
  })

  it('retourne null pour un utilisateur inconnu', async () => {
    expect(await getProfile(db, '00000000-0000-0000-0000-000000000000')).toBeNull()
  })
})

describe('updateProfile', () => {
  it('applique un patch partiel sans toucher au reste', async () => {
    const user = await createTestUser(db, 'Avant')
    const profile = await updateProfile(db, user.id, { name: 'Après', rotationWeeks: 6, unitSystem: 'imperial' })

    expect(profile).toMatchObject({ name: 'Après', rotationWeeks: 6, unitSystem: 'imperial' })
    // Champs non patchés inchangés.
    expect(profile).toMatchObject({ locale: 'fr', defaultDishCount: 4 })
  })
})

describe('changePassword', () => {
  it('refuse un mot de passe actuel invalide', async () => {
    const user = await createUserWithPassword()
    expect(await changePassword(db, user.id, 'mauvais-mot-de-passe', 'nouveau-mot-de-passe-solide'))
      .toBe('invalidPassword')

    // L'ancien mot de passe fonctionne toujours.
    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(await verifyUserPassword(row!.passwordHash, PASSWORD)).toBe(true)
  })

  it('remplace le hachage avec le bon mot de passe actuel', async () => {
    const user = await createUserWithPassword()
    expect(await changePassword(db, user.id, PASSWORD, 'nouveau-mot-de-passe-solide')).toBe('ok')

    const row = await db.query.users.findFirst({ where: eq(users.id, user.id) })
    expect(await verifyUserPassword(row!.passwordHash, 'nouveau-mot-de-passe-solide')).toBe(true)
    expect(await verifyUserPassword(row!.passwordHash, PASSWORD)).toBe(false)
  })
})

describe('deleteAccount', () => {
  it('refuse un mot de passe invalide sans rien supprimer', async () => {
    const user = await createUserWithPassword()
    expect(await deleteAccount(db, user.id, 'mauvais-mot-de-passe')).toBe('invalidPassword')
    expect(await getProfile(db, user.id)).not.toBeNull()
  })

  it('supprime le compte et toutes ses données en cascade', async () => {
    const user = await createUserWithPassword()
    await createRecipe(db, user.id, recipeInputSchema.parse({
      title: 'Recette à emporter dans la tombe',
      baseServings: 4,
      ingredients: [{ quantity: 1, unit: 'unit', name: 'ingrédient' }],
      steps: ['Cuisiner.'],
      tags: ['test'],
    }))

    expect(await deleteAccount(db, user.id, PASSWORD)).toBe('ok')
    expect(await getProfile(db, user.id)).toBeNull()
    expect(await db.select().from(recipes)).toHaveLength(0)
  })

  it('refuse l\'auto-suppression du dernier admin actif', async () => {
    const admin = await createUserWithPassword('admin')
    expect(await deleteAccount(db, admin.id, PASSWORD)).toBe('lastAdmin')
    expect(await getProfile(db, admin.id)).not.toBeNull()
  })

  it('supprime un admin quand un autre admin actif existe', async () => {
    const admin = await createUserWithPassword('admin')
    await createTestUser(db, 'Autre admin', { role: 'admin', emailVerifiedAt: new Date() })
    expect(await deleteAccount(db, admin.id, PASSWORD)).toBe('ok')
  })

  it('un admin désactivé ne compte pas comme relève', async () => {
    const admin = await createUserWithPassword('admin')
    await createTestUser(db, 'Admin désactivé', { role: 'admin', emailVerifiedAt: new Date(), disabledAt: new Date() })
    expect(await deleteAccount(db, admin.id, PASSWORD)).toBe('lastAdmin')
  })

  it('un admin jamais activé ne compte pas comme relève', async () => {
    const admin = await createUserWithPassword('admin')
    await createTestUser(db, 'Invitation en attente', { role: 'admin' })
    expect(await deleteAccount(db, admin.id, PASSWORD)).toBe('lastAdmin')
  })
})
