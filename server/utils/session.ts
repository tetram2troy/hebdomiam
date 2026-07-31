import type { H3Event } from 'h3'
import { useDb } from './db'
import { clientIp, logWarn } from './logger'
import { getAccountStatus } from './users-repo'

// Gardes de session au-delà de requireUserSession (nuxt-auth-utils).

/** Session admin exigée pour les routes /api/admin/*.
    Le rôle et le statut sont relus EN BASE à chaque appel : le cookie scellé
    peut être périmé (démotion, désactivation) — on ne lui fait jamais confiance
    pour une décision d'autorisation. */
export async function requireAdminSession(event: H3Event) {
  const { user } = await requireUserSession(event)
  const status = await getAccountStatus(useDb(), user.id)
  if (!status || status.role !== 'admin' || status.disabledAt) {
    logWarn('security.admin_denied', { userId: user.id, ip: clientIp(event) })
    throw createError({ statusCode: 403, message: 'errors.forbidden' })
  }
  return { user }
}
