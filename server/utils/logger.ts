import type { H3Event } from 'h3'
import { getRequestIP } from 'h3'

// Journalisation applicative structurée : une ligne JSON par événement sur la
// sortie standard — captée par Docker en prod (PRD §8), exploitable avec jq/grep.
//
// Règles :
// - nommage `domaine.action` (auth.login, recipe.deleted…) ;
// - niveaux : info = opération normale digne d'audit, warn = échec de sécurité
//   ou comportement suspect, error = défaillance technique ;
// - jamais de secret (mot de passe, jeton, code TOTP) ni de contenu utilisateur
//   (titres, libellés) ; PII minimale : userId de préférence — le courriel
//   uniquement pour les événements d'auth sans compte résolu (détection d'abus).

type Level = 'info' | 'warn' | 'error'

function write(level: Level, event: string, fields: Record<string, unknown>) {
  const line = JSON.stringify({ time: new Date().toISOString(), level, event, ...fields })
  if (level === 'error') console.error(line)
  else if (level === 'warn') console.warn(line)
  else console.info(line)
}

/** Opération normale digne d'audit (connexion, création, suppression…). */
export function logEvent(event: string, fields: Record<string, unknown> = {}) {
  write('info', event, fields)
}

/** Échec de sécurité ou comportement suspect (mot de passe refusé, jeton invalide…). */
export function logWarn(event: string, fields: Record<string, unknown> = {}) {
  write('warn', event, fields)
}

/** Défaillance technique inattendue — consigne le message, jamais la charge utile. */
export function logError(event: string, error: unknown, fields: Record<string, unknown> = {}) {
  write('error', event, { ...fields, message: error instanceof Error ? error.message : String(error) })
}

/** IP du client (X-Forwarded-For derrière Traefik) pour les événements de sécurité. */
export function clientIp(event: H3Event): string {
  return getRequestIP(event, { xForwardedFor: true }) ?? 'unknown'
}
