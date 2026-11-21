import pg from 'pg'
import { DB_URL, assertLocalDatabase } from './env'
import { ADMIN_EMAIL } from './helpers'

// Comptes créés par la suite e2e — et SEULEMENT eux : les motifs reprennent
// exactement les adresses générées par les specs (horodatage + aléa), jamais
// un domaine entier. Les données métier (recettes, plans, jetons…) suivent
// par ON DELETE CASCADE.
//   - e2e-<ms>-<aléa>@example.com           : registerFreshUser (helpers.ts)
//   - admin-invitee-<ms>-<aléa>@example.com : invitation testée (admin.spec.ts)
//   - admin-e2e@example.com                 : admin semé par global-setup.ts
// Sans borne de date : les restes d'une exécution interrompue partent aussi.
export const GENERATED_EMAIL_PATTERNS = [
  String.raw`^e2e-[0-9]+-[0-9]+@example\.com$`,
  String.raw`^admin-invitee-[0-9]+-[0-9]+@example\.com$`,
]

/** Supprime les comptes de test ; retourne le nombre de comptes supprimés. */
export async function cleanupTestAccounts(): Promise<number> {
  assertLocalDatabase('la suppression des comptes de test')
  const client = new pg.Client({ connectionString: DB_URL })
  await client.connect()
  try {
    const result = await client.query(
      'DELETE FROM users WHERE email = $1 OR email ~ ANY($2::text[])',
      [ADMIN_EMAIL, GENERATED_EMAIL_PATTERNS],
    )
    return result.rowCount ?? 0
  }
  finally {
    await client.end()
  }
}
