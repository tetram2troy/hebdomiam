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
  // Requises seulement en production réelle (NODE_ENV, posé par l'image Docker) :
  // le serveur e2e local tourne sans SMTP (courriels journalisés en console).
  if (env.NODE_ENV === 'production') {
    if (!env.NUXT_PUBLIC_BASE_URL) {
      problems.push('NUXT_PUBLIC_BASE_URL manquant (base des liens de courriels, ex. https://recettes.example.com — sans elle les liens seraient dérivés de l\'en-tête Host)')
    }
    if (!env.SMTP_HOST) {
      problems.push('SMTP_HOST manquant (relais SMTP — sans lui vérifications, invitations et réinitialisations partiraient en silence dans les logs)')
    }
  }
  return problems
}
