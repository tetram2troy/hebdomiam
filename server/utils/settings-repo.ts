import type { ProfileUpdateInput } from '#shared/schemas/settings'
import type { ProfileDto } from '#shared/types/settings'
import { eq } from 'drizzle-orm'
import type { Db } from '../database/client'
import { users } from '../database/schema'
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

/** Suppression immédiate et définitive (cascade sur toutes les données métier). */
export async function deleteAccount(db: Db, userId: string, password: string): Promise<'ok' | 'invalidPassword' | 'notFound'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { passwordHash: true },
  })
  if (!user) return 'notFound'
  if (!(await verifyUserPassword(user.passwordHash, password))) return 'invalidPassword'
  await db.delete(users).where(eq(users.id, userId))
  return 'ok'
}
