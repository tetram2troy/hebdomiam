import { eq } from 'drizzle-orm'
import { resetPasswordSchema } from '#shared/schemas/auth'
import { users } from '../../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'password:reset', { max: 5, windowMs: 60_000 })
  const { token, password } = await readValidatedBody(event, data => resetPasswordSchema.parse(data))

  const userId = await consumeEmailToken(useDb(), 'reset', token)
  if (!userId) {
    throw createError({ statusCode: 400, message: 'errors.invalidToken' })
  }
  await useDb().update(users)
    .set({ passwordHash: await hashUserPassword(password) })
    .where(eq(users.id, userId))
  return { ok: true }
})
