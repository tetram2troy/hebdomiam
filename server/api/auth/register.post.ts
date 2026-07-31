import { registerSchema } from '#shared/schemas/auth'

// Inscription publique fermée : seule la première inscription d'une instance
// vierge passe (bootstrap, compte promu admin). Les autres comptes sont créés
// par un admin via une invitation (/api/admin/users).
export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:register', { max: 5, windowMs: 60_000 })

  const body = await readValidatedBody(event, data => registerSchema.parse(data))
  const passwordHash = await hashUserPassword(body.password)

  const result = await registerUser(useDb(), { email: body.email, name: body.name, passwordHash })

  if (result === 'closed') {
    logWarn('auth.register_closed', { ip: clientIp(event) })
    throw createError({ statusCode: 403, message: 'errors.registrationClosed' })
  }
  if (result === 'emailTaken') {
    logWarn('auth.register_email_taken', { ip: clientIp(event) })
    throw createError({ statusCode: 409, message: 'errors.emailTaken' })
  }
  logEvent('auth.register', { userId: result.id, ip: clientIp(event), bootstrap: true })

  // Courriel de vérification — l'inscription n'échoue jamais sur un souci SMTP.
  try {
    const token = await createEmailToken(useDb(), result.id, 'verify', 24 * 60)
    const link = `${appBaseUrl(getRequestURL(event).origin)}/verifier-courriel?token=${token}`
    // locale du compte à la création = fr (défaut du schéma)
    await sendMail(verifyEmailMail(result.email, 'fr', link))
  }
  catch (error) {
    logError('mailer.verify_send_failed', error, { userId: result.id })
  }

  await setUserSession(event, { user: result })
  setResponseStatus(event, 201)
  return { user: result }
})
