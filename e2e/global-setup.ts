import { createDbClient } from '../server/database/client'
import { users } from '../server/database/schema'
import { hashUserPassword } from '../server/utils/password'
import { ADMIN_EMAIL, ADMIN_PASSWORD } from './helpers'

// Dérogation assumée au principe « e2e via l'UI » : l'inscription publique est
// fermée (seul le bootstrap d'une instance vierge passe), le compte admin de la
// suite est donc provisionné directement en base — l'équivalent du provisioning
// d'une instance réelle. Upsert idempotent : même chemin sur la DB de dev
// peuplée et sur une DB vierge en CI.
export default async function globalSetup() {
  const url = process.env.NUXT_DATABASE_URL
  if (!url) {
    throw new Error('NUXT_DATABASE_URL manquant : charge le .env avant npm run test:e2e')
  }

  // Garde-fou : ce seed écrit un admin dont le mot de passe est public (dans le
  // dépôt). Refuser toute base non locale — un .env pointant sur le home lab ne
  // doit jamais y créer une porte dérobée.
  const host = new URL(url).hostname
  if (!['localhost', '127.0.0.1', '::1'].includes(host) && process.env.HEBDOMIAM_E2E_ALLOW_REMOTE_DB !== '1') {
    throw new Error(`global-setup e2e refuse la base non locale « ${host} » : il y sèmerait un admin à mot de passe connu`)
  }

  const db = createDbClient(url)
  const passwordHash = await hashUserPassword(ADMIN_PASSWORD)
  await db.insert(users)
    .values({
      email: ADMIN_EMAIL,
      passwordHash,
      name: 'Admin E2E',
      role: 'admin',
      emailVerifiedAt: new Date(),
    })
    .onConflictDoUpdate({
      target: users.email,
      set: { passwordHash, role: 'admin', disabledAt: null, emailVerifiedAt: new Date() },
    })
  await db.$client.end()
}
