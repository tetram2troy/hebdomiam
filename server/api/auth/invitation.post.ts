import { acceptInviteSchema } from '#shared/schemas/auth'

// Acceptation d'une invitation créée par un admin : l'utilisateur choisit son
// mot de passe via le lien reçu — la possession du lien vaut vérification du
// courriel. Pas de session posée : l'invité se connecte ensuite (modèle du
// flux de réinitialisation de mot de passe).
export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:invitation', { max: 5, windowMs: 60_000 })
  const { token, password } = await readValidatedBody(event, data => acceptInviteSchema.parse(data))

  const result = await acceptInvitation(useDb(), token, await hashUserPassword(password))
  if (typeof result === 'string') {
    logWarn('auth.invite_refused', { reason: result, ip: clientIp(event) })
    const responses = {
      invalidToken: { statusCode: 400, message: 'errors.invalidToken' },
      // Compte déjà actif : un jeton remplacé ne doit jamais écraser le mot de
      // passe d'un compte en usage (le jeton vient d'être brûlé, c'est voulu).
      alreadyActive: { statusCode: 409, message: 'errors.inviteAlreadyActive' },
      disabled: { statusCode: 403, message: 'errors.accountDisabled' },
    } as const
    throw createError(responses[result])
  }

  logEvent('auth.invite_accepted', { userId: result.userId, ip: clientIp(event) })
  return { ok: true }
})
