import { randomBytes } from 'node:crypto'
import { and, asc, eq, isNotNull, isNull } from 'drizzle-orm'
import type { AdminUserDto } from '#shared/types/admin'
import type { Db, DbLike } from '../database/client'
import { users } from '../database/schema'
import { createEmailToken, invalidateEmailTokens } from './email-tokens'
import { hashUserPassword } from './password'

// Gestion des comptes par un administrateur. L'autorisation (rôle admin) est
// vérifiée en amont par requireAdminSession — ici vivent la logique et les
// garde-fous anti-lockout :
//   - 'self' : un admin ne se cible jamais lui-même (son compte se gère dans
//     les réglages) ;
//   - 'lastAdmin' : on ne retire jamais le dernier admin actif de l'instance.

const INVITE_TTL_MINUTES = 7 * 24 * 60 // 7 jours : l'invité n'est pas devant son écran

type UserRow = typeof users.$inferSelect

/** Verrouille (FOR UPDATE) puis compte les admins comptés comme relève :
    rôle admin, non désactivés ET courriel vérifié. Un compte invité jamais
    activé a un mot de passe inutilisable — le compter permettrait un lockout
    (promouvoir l'invitation puis retirer le vrai admin). Biais assumé : un
    admin au courriel non vérifié mais au mot de passe valide (bootstrap) n'est
    pas compté non plus — le garde refuse alors trop, jamais l'inverse.
    Le verrou est ce qui rend le décompte sûr quand deux admins agissent en
    même temps : en READ COMMITTED, un simple COUNT laisserait les deux
    transactions lire « 2 » et retirer chacune un admin. À appeler dans une
    transaction. */
export async function lockActiveAdmins(db: DbLike): Promise<number> {
  const rows = await db.select({ id: users.id }).from(users)
    .where(and(eq(users.role, 'admin'), isNull(users.disabledAt), isNotNull(users.emailVerifiedAt)))
    .for('update')
  return rows.length
}

function toAdminUserDto(row: Pick<UserRow, 'id' | 'email' | 'name' | 'role' | 'emailVerifiedAt' | 'disabledAt' | 'createdAt'>): AdminUserDto {
  return {
    id: row.id,
    email: row.email,
    name: row.name,
    role: row.role,
    emailVerified: row.emailVerifiedAt !== null,
    disabled: row.disabledAt !== null,
    createdAt: row.createdAt.toISOString(),
  }
}

export async function listUsers(db: Db): Promise<AdminUserDto[]> {
  const rows = await db.query.users.findMany({
    columns: { id: true, email: true, name: true, role: true, emailVerifiedAt: true, disabledAt: true, createdAt: true },
    orderBy: asc(users.createdAt),
  })
  return rows.map(toAdminUserDto)
}

/** Crée un compte invité : mot de passe aléatoire inutilisable — le compte ne
    devient connectable qu'à l'acceptation de l'invitation (choix du mot de
    passe, qui vaut vérification du courriel). */
export async function createInvitedUser(db: Db, email: string, name: string): Promise<
  { user: AdminUserDto, token: string } | 'emailTaken'
> {
  const passwordHash = await hashUserPassword(randomBytes(32).toString('hex'))
  // Transaction : pas de compte sans jeton d'invitation (un échec au milieu
  // laisserait un compte orphelin injoignable).
  return db.transaction(async (tx) => {
    const [row] = await tx.insert(users)
      .values({ email, name, passwordHash })
      .onConflictDoNothing({ target: users.email })
      .returning()
    if (!row) return 'emailTaken'

    const token = await createEmailToken(tx, row.id, 'invite', INVITE_TTL_MINUTES)
    return { user: toAdminUserDto(row), token }
  })
}

/** Nouveau jeton d'invitation pour un compte jamais activé (courriel non
    vérifié = l'invité n'a pas encore choisi de mot de passe). Les jetons
    précédents sont invalidés : un seul lien vivant à la fois. */
export async function renewInvite(db: Db, targetId: string): Promise<
  { email: string, locale: 'fr' | 'en', token: string } | 'notFound' | 'alreadyActive' | 'disabled'
> {
  const row = await db.query.users.findFirst({
    where: eq(users.id, targetId),
    columns: { email: true, locale: true, emailVerifiedAt: true, disabledAt: true },
  })
  if (!row) return 'notFound'
  if (row.emailVerifiedAt) return 'alreadyActive'
  // L'UI masque le bouton, mais l'API est la frontière : pas de lien
  // d'activation pour un compte désactivé.
  if (row.disabledAt) return 'disabled'

  return db.transaction(async (tx) => {
    await invalidateEmailTokens(tx, targetId, 'invite')
    const token = await createEmailToken(tx, targetId, 'invite', INVITE_TTL_MINUTES)
    return { email: row.email, locale: row.locale, token }
  })
}

export interface AdminUserPatch {
  role?: 'admin' | 'user'
  disabled?: boolean
}

export async function updateUserAsAdmin(db: Db, actorId: string, targetId: string, patch: AdminUserPatch): Promise<
  AdminUserDto | 'notFound' | 'self' | 'lastAdmin'
> {
  if (actorId === targetId) return 'self'

  return db.transaction(async (tx) => {
    const target = await tx.query.users.findFirst({ where: eq(users.id, targetId) })
    if (!target) return 'notFound'

    // Seul le retrait d'un admin capable d'administrer (vérifié, actif) peut
    // créer un lockout — rétrograder une invitation en attente est sans risque.
    const removesActiveAdmin = target.role === 'admin' && target.disabledAt === null
      && target.emailVerifiedAt !== null
      && (patch.role === 'user' || patch.disabled === true)
    if (removesActiveAdmin && (await lockActiveAdmins(tx)) <= 1) return 'lastAdmin'

    const set: Partial<Pick<UserRow, 'role' | 'disabledAt'>> = {}
    if (patch.role !== undefined) set.role = patch.role
    if (patch.disabled !== undefined) {
      // Re-désactiver un compte déjà désactivé ne change pas la date d'origine.
      set.disabledAt = patch.disabled ? (target.disabledAt ?? new Date()) : null
    }

    const [row] = await tx.update(users).set(set).where(eq(users.id, targetId)).returning()
    return row ? toAdminUserDto(row) : 'notFound'
  })
}

/** Suppression définitive (cascade sur toutes les données métier). */
export async function deleteUserAsAdmin(db: Db, actorId: string, targetId: string): Promise<
  'ok' | 'notFound' | 'self' | 'lastAdmin'
> {
  if (actorId === targetId) return 'self'

  return db.transaction(async (tx) => {
    const target = await tx.query.users.findFirst({
      where: eq(users.id, targetId),
      columns: { role: true, disabledAt: true, emailVerifiedAt: true },
    })
    if (!target) return 'notFound'

    const isActiveAdmin = target.role === 'admin' && target.disabledAt === null
      && target.emailVerifiedAt !== null
    if (isActiveAdmin && (await lockActiveAdmins(tx)) <= 1) return 'lastAdmin'

    await tx.delete(users).where(eq(users.id, targetId))
    return 'ok'
  })
}
