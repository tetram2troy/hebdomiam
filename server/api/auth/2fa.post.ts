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
  const method = await verifyTotpLogin(useDb(), pendingUserId, code)
  if (!method) {
    logWarn('auth.2fa_failed', { userId: pendingUserId, ip: clientIp(event) })
    throw createError({ statusCode: 401, message: 'errors.invalidCode' })
  }

  const user = await useDb().query.users.findFirst({ where: eq(users.id, pendingUserId) })
  if (!user) {
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  // Fenêtre login → 2FA : le compte a pu être désactivé entre les deux étapes.
  if (user.disabledAt) {
    logWarn('auth.login_disabled', { userId: user.id, ip: clientIp(event) })
    throw createError({ statusCode: 403, message: 'errors.accountDisabled' })
  }

  const publicUser = { id: user.id, email: user.email, name: user.name, role: user.role }
  await replaceUserSession(event, { user: publicUser })
  // method=backup : authenticateur perdu — ou compromis. À surveiller.
  logEvent('auth.login', { userId: user.id, ip: clientIp(event), twoFactor: method })
  return { user: publicUser }
})
