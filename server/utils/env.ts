// Contrat de configuration 12-factor : l'app refuse de démarrer en production
// si une variable requise manque — jamais de mode dégradé silencieux
// (docs/HOSTING.md §2.9). Fonction pure : testée en unitaire.

export function envProblems(env: Record<string, string | undefined>): string[] {
  const problems: string[] = []
  if (!env.NUXT_DATABASE_URL) {
    problems.push('NUXT_DATABASE_URL manquant (URL PostgreSQL, ex. postgresql://user:mdp@db:5432/hebdomiam)')
  }
  if (!env.NUXT_SESSION_PASSWORD) {
    problems.push('NUXT_SESSION_PASSWORD manquant (scelle les cookies de session — openssl rand -base64 32)')
  }
  else if (env.NUXT_SESSION_PASSWORD.length < 32) {
    problems.push('NUXT_SESSION_PASSWORD trop court (minimum 32 caractères)')
  }
  return problems
}
