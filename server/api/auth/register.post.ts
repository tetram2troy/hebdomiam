import { registerSchema } from '#shared/schemas/auth'
import { users } from '../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:register', { max: 5, windowMs: 60_000 })

  const body = await readValidatedBody(event, data => registerSchema.parse(data))
  const passwordHash = await hashUserPassword(body.password)

  const [user] = await useDb()
    .insert(users)
    .values({ email: body.email, passwordHash, name: body.name })
    .onConflictDoNothing({ target: users.email })
    .returning({ id: users.id, email: users.email, name: users.name })

  if (!user) {
    logWarn('auth.register_email_taken', { ip: clientIp(event) })
    throw createError({ statusCode: 409, message: 'errors.emailTaken' })
  }
  logEvent('auth.register', { userId: user.id, ip: clientIp(event) })

  // Courriel de vérification — l'inscription n'échoue jamais sur un souci SMTP.
  try {
    const token = await createEmailToken(useDb(), user.id, 'verify', 24 * 60)
    const link = `${appBaseUrl(getRequestURL(event).origin)}/verifier-courriel?token=${token}`
    // locale du compte à la création = fr (défaut du schéma)
    await sendMail(verifyEmailMail(user.email, 'fr', link))
  }
  catch (error) {
    logError('mailer.verify_send_failed', error, { userId: user.id })
  }

  await setUserSession(event, { user })
  setResponseStatus(event, 201)
  return { user }
})
