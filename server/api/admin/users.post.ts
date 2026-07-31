import { adminCreateUserSchema } from '#shared/schemas/admin'

// Création d'un compte par un admin : le compte naît 'user', non connectable
// tant que l'invitation n'est pas acceptée. L'URL d'invitation est retournée à
// l'admin (copiable dans l'UI) — le courriel n'est qu'un canal de confort.
export default defineEventHandler(async (event) => {
  const { user } = await requireAdminSession(event)
  assertRateLimit(event, `admin:create:${user.id}`, { max: 30, windowMs: 60_000 })
  const body = await readValidatedBody(event, data => adminCreateUserSchema.parse(data))

  const result = await createInvitedUser(useDb(), body.email, body.name)
  if (result === 'emailTaken') {
    logWarn('admin.user_create_conflict', { userId: user.id, ip: clientIp(event) })
    throw createError({ statusCode: 409, message: 'errors.emailTaken' })
  }

  const inviteUrl = `${appBaseUrl(getRequestURL(event).origin)}/invitation?token=${result.token}`
  try {
    // locale du compte à la création = fr (défaut du schéma)
    await sendMail(inviteMail(result.user.email, 'fr', inviteUrl))
  }
  catch (error) {
    logError('mailer.invite_send_failed', error, { userId: user.id, targetUserId: result.user.id })
  }

  logEvent('admin.user_created', { userId: user.id, targetUserId: result.user.id, ip: clientIp(event) })
  setResponseStatus(event, 201)
  return { user: result.user, inviteUrl }
})
