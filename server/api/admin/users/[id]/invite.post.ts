import { adminUserIdParamsSchema } from '#shared/schemas/admin'

// Renvoie une invitation (nouveau jeton) pour un compte jamais activé —
// utile quand le lien a expiré ou que le courriel s'est perdu.
export default defineEventHandler(async (event) => {
  const { user } = await requireAdminSession(event)
  assertRateLimit(event, `admin:invite:${user.id}`, { max: 30, windowMs: 60_000 })
  const { id: targetId } = await getValidatedRouterParams(event, data => adminUserIdParamsSchema.parse(data))

  const result = await renewInvite(useDb(), targetId)
  if (typeof result === 'string') {
    logWarn('admin.invite_resend_refused', { userId: user.id, targetUserId: targetId, reason: result, ip: clientIp(event) })
    const responses = {
      notFound: { statusCode: 404, message: 'errors.notFound' },
      alreadyActive: { statusCode: 409, message: 'errors.inviteAlreadyActive' },
      disabled: { statusCode: 409, message: 'errors.accountDisabled' },
    } as const
    throw createError(responses[result])
  }

  const inviteUrl = `${appBaseUrl(getRequestURL(event).origin)}/invitation?token=${result.token}`
  try {
    await sendMail(inviteMail(result.email, result.locale, inviteUrl))
  }
  catch (error) {
    logError('mailer.invite_send_failed', error, { userId: user.id, targetUserId: targetId })
  }

  logEvent('admin.invite_resent', { userId: user.id, targetUserId: targetId, ip: clientIp(event) })
  return { inviteUrl }
})
