import { eq } from 'drizzle-orm'
import { verifyEmailSchema } from '#shared/schemas/auth'
import { users } from '../../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'verify:confirm', { max: 10, windowMs: 60_000 })
  const { token } = await readValidatedBody(event, data => verifyEmailSchema.parse(data))

  const userId = await consumeEmailToken(useDb(), 'verify', token)
  if (!userId) {
    logWarn('auth.verify_invalid_token', { ip: clientIp(event) })
    throw createError({ statusCode: 400, message: 'errors.invalidToken' })
  }
  await useDb().update(users).set({ emailVerifiedAt: new Date() }).where(eq(users.id, userId))
  logEvent('auth.email_verified', { userId })
  return { ok: true }
})
