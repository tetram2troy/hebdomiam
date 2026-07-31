import type { ProfileUpdateInput } from '#shared/schemas/settings'
import type { ProfileDto } from '#shared/types/settings'
import { eq } from 'drizzle-orm'
import type { Db } from '../database/client'
import { users } from '../database/schema'
import { lockActiveAdmins } from './admin-repo'
import { hashUserPassword, verifyUserPassword } from './password'

// Réglages du compte — chaque fonction est scopée sur l'utilisateur de la session.

export async function getProfile(db: Db, userId: string): Promise<ProfileDto | null> {
  const user = await db.query.users.findFirst({ where: eq(users.id, userId) })
  if (!user) return null
  return {
    name: user.name,
    email: user.email,
    emailVerifiedAt: user.emailVerifiedAt?.toISOString() ?? null,
    locale: user.locale,
    unitSystem: user.unitSystem,
    defaultDishCount: user.defaultDishCount,
    defaultPeopleCount: user.defaultPeopleCount,
    defaultDaysCount: user.defaultDaysCount,
    rotationWeeks: user.rotationWeeks,
    totpEnabled: user.totpEnabledAt !== null,
  }
}

export async function updateProfile(db: Db, userId: string, patch: ProfileUpdateInput): Promise<ProfileDto | null> {
  await db.update(users).set(patch).where(eq(users.id, userId))
  return getProfile(db, userId)
}

export async function changePassword(db: Db, userId: string, currentPassword: string, newPassword: string): Promise<'ok' | 'invalidPassword' | 'notFound'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { passwordHash: true },
  })
  if (!user) return 'notFound'
  if (!(await verifyUserPassword(user.passwordHash, currentPassword))) return 'invalidPassword'
  await db.update(users)
    .set({ passwordHash: await hashUserPassword(newPassword) })
    .where(eq(users.id, userId))
  return 'ok'
}

/** Suppression immédiate et définitive (cascade sur toutes les données métier).
    Refusée au dernier admin actif : l'instance deviendrait ingérable — c'est le
    même garde-fou que dans admin-repo, appliqué ici au seul contournement
    possible (l'auto-suppression via les réglages). */
export async function deleteAccount(db: Db, userId: string, password: string): Promise<'ok' | 'invalidPassword' | 'notFound' | 'lastAdmin'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { passwordHash: true, role: true, disabledAt: true },
  })
  if (!user) return 'notFound'
  if (!(await verifyUserPassword(user.passwordHash, password))) return 'invalidPassword'

  if (user.role === 'admin' && user.disabledAt === null) {
    return db.transaction(async (tx) => {
      // Décompte verrouillé des admins vérifiés et actifs : un admin jamais
      // activé (invitation en attente) n'est pas une relève.
      if ((await lockActiveAdmins(tx)) <= 1) return 'lastAdmin'
      await tx.delete(users).where(eq(users.id, userId))
      return 'ok'
    })
  }

  await db.delete(users).where(eq(users.id, userId))
  return 'ok'
}
