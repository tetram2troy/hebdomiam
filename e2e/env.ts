// Base de données de la suite e2e (celle du serveur testé). DATABASE_URL, avec
// repli sur l'ancien nom NUXT_DATABASE_URL pour un .env pas encore migré.
export const DB_URL = process.env.DATABASE_URL ?? process.env.NUXT_DATABASE_URL ?? ''

export const E2E_PORT = 3000
export const BASE_URL = `http://127.0.0.1:${E2E_PORT}`

/** Garde-fou commun au seed et au nettoyage : la suite écrit un admin au mot
    de passe public et SUPPRIME des comptes — jamais sur une base non locale.
    Seule la CI (base éphémère du runner) peut lever la garde. */
export function assertLocalDatabase(action: string): void {
  if (!DB_URL) {
    throw new Error('DATABASE_URL manquant : charge le .env avant npm run test:e2e')
  }
  const host = new URL(DB_URL).hostname
  const local = ['localhost', '127.0.0.1', '::1'].includes(host)
  const ciOverride = Boolean(process.env.CI) && process.env.HEBDOMIAM_E2E_ALLOW_REMOTE_DB === '1'
  if (!local && !ciOverride) {
    throw new Error(`e2e : ${action} refusé sur la base non locale « ${host} »`)
  }
}
