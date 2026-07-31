import { count, eq } from 'drizzle-orm'
import type { Db } from '../database/client'
import { users } from '../database/schema'
import { consumeEmailToken } from './email-tokens'

// Cycle de vie des comptes côté inscription publique et invitation. La
// gestion par les admins vit dans admin-repo.ts.

export async function countUsers(db: Db): Promise<number> {
  const [row] = await db.select({ value: count() }).from(users)
  return row?.value ?? 0
}

export interface RegisterUserInput {
  email: string
  name: string
  passwordHash: string
}

/** Inscription publique : fermée dès qu'un compte existe — seule la première
    inscription d'une instance vierge passe, et ce compte est promu admin
    (« premier inscrit = admin »). Les comptes suivants sont créés par un
    admin via une invitation (admin-repo). */
export async function registerUser(db: Db, input: RegisterUserInput): Promise<
  { id: string, email: string, name: string, role: 'admin' | 'user' } | 'closed' | 'emailTaken'
> {
  return db.transaction(async (tx) => {
    const [existing] = await tx.select({ value: count() }).from(users)
    if ((existing?.value ?? 0) > 0) return 'closed'

    const [user] = await tx.insert(users)
      .values({ email: input.email, name: input.name, passwordHash: input.passwordHash, role: 'admin' })
      .onConflictDoNothing({ target: users.email })
      .returning({ id: users.id, email: users.email, name: users.name, role: users.role })

    // Course improbable (deux inscriptions simultanées sur base vide) : la
    // contrainte unique tranche, le perdant reçoit la même erreur que si le
    // courriel était pris.
    if (!user) return 'emailTaken'
    return user
  })
}

/** Acceptation d'une invitation : fixe le mot de passe et marque le courriel
    vérifié (la possession du lien en est la preuve). Refusée si le compte est
    déjà actif — un jeton remplacé mais pas encore expiré ne doit jamais servir
    à écraser le mot de passe d'un compte en usage. Le jeton est consommé même
    en cas de refus : c'est voulu, un lien caduc doit mourir au premier clic. */
export async function acceptInvitation(db: Db, rawToken: string, passwordHash: string): Promise<
  { userId: string } | 'invalidToken' | 'alreadyActive' | 'disabled'
> {
  const userId = await consumeEmailToken(db, 'invite', rawToken)
  if (!userId) return 'invalidToken'

  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { emailVerifiedAt: true, disabledAt: true },
  })
  if (!user) return 'invalidToken'
  if (user.emailVerifiedAt) return 'alreadyActive'
  if (user.disabledAt) return 'disabled'

  await db.update(users)
    .set({ passwordHash, emailVerifiedAt: new Date() })
    .where(eq(users.id, userId))
  return { userId }
}

/** Statut minimal d'un compte pour les décisions d'autorisation (gardes de
    session) — null si le compte n'existe plus. */
export async function getAccountStatus(db: Db, userId: string): Promise<
  { role: 'admin' | 'user', disabledAt: Date | null } | null
> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { role: true, disabledAt: true },
  })
  return user ?? null
}
