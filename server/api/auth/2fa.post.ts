import { eq } from 'drizzle-orm'
import { totpCodeSchema } from '#shared/schemas/auth'
import { users } from '../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:2fa', { max: 5, windowMs: 60_000 })

  const session = await getUserSession(event)
  const pendingUserId = session.pendingTotpUserId
  if (!pendingUserId) {
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  const { code } = await readValidatedBody(event, data => totpCodeSchema.parse(data))
  if (!(await verifyTotpLogin(useDb(), pendingUserId, code))) {
    throw createError({ statusCode: 401, message: 'errors.invalidCode' })
  }

  const user = await useDb().query.users.findFirst({ where: eq(users.id, pendingUserId) })
  if (!user) {
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  const publicUser = { id: user.id, email: user.email, name: user.name }
  await replaceUserSession(event, { user: publicUser })
  return { user: publicUser }
})
