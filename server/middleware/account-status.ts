import { useDb } from '../utils/db'
import { clientIp, logWarn } from '../utils/logger'
import { getAccountStatus } from '../utils/users-repo'

// Les sessions sont des cookies scellés sans état serveur : rien à révoquer
// côté serveur quand un compte est désactivé ou supprimé. Ce middleware relit
// le statut en base à chaque requête API authentifiée (SELECT par clé primaire,
// coût négligeable) pour que la désactivation soit effective immédiatement,
// pas au terme des 7 jours de vie du cookie.
//
// /api/auth/* est exclu : ces routes doivent rester joignables avec un cookie
// périmé (se reconnecter sous un autre compte, accepter une invitation,
// réinitialiser un mot de passe) et font leurs propres vérifications de statut.
// La session client (/api/_auth/session), elle, reste couverte.
export default defineEventHandler(async (event) => {
  if (!event.path.startsWith('/api/') || event.path.startsWith('/api/auth/')) return

  const session = await getUserSession(event)
  const userId = session.user?.id
  if (!userId) return

  const status = await getAccountStatus(useDb(), userId)

  // Compte supprimé : la session ne référence plus rien — on la purge sans
  // divulguer la raison (le 401 générique suffit).
  if (!status) {
    await clearUserSession(event)
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  if (status.disabledAt) {
    logWarn('security.disabled_session_blocked', { userId, ip: clientIp(event) })
    await clearUserSession(event)
    throw createError({ statusCode: 401, message: 'errors.accountDisabled' })
  }
})
