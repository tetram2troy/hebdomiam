import { envProblems } from '../utils/env'

// Validation du contrat de configuration au démarrage (production seulement :
// en dev, nuxt-auth-utils génère lui-même un mot de passe de session).
// process.exit plutôt qu'un throw : un plugin Nitro qui échoue ne doit pas
// laisser le serveur tourner à moitié configuré.
export default defineNitroPlugin(() => {
  if (import.meta.dev) return

  const problems = envProblems(process.env)
  if (problems.length > 0) {
    for (const problem of problems) {
      console.error(`[config] ${problem}`)
    }
    console.error('[config] démarrage refusé : configuration incomplète (voir .env.example)')
    process.exit(1)
  }
})
