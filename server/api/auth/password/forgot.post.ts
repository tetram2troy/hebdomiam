import { eq } from 'drizzle-orm'
import { forgotPasswordSchema } from '#shared/schemas/auth'
import { users } from '../../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'password:forgot', { max: 3, windowMs: 10 * 60_000 })
  const { email } = await readValidatedBody(event, data => forgotPasswordSchema.parse(data))

  // Réponse identique que le compte existe ou non (anti-énumération).
  const user = await useDb().query.users.findFirst({ where: eq(users.email, email) })
  if (user) {
    const token = await createEmailToken(useDb(), user.id, 'reset', 60)
    const link = `${appBaseUrl(getRequestURL(event).origin)}/reinitialiser?token=${token}`
    await sendMail(resetPasswordMail(user.email, user.locale, link))
  }
  return { ok: true }
})
