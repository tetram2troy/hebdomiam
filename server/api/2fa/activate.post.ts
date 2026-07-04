import { totpCodeSchema } from '#shared/schemas/auth'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `2fa:activate:${user.id}`, { max: 5, windowMs: 60_000 })
  const { code } = await readValidatedBody(event, data => totpCodeSchema.parse(data))

  const result = await activateTotp(useDb(), user.id, code)
  if (result === 'invalidCode' || result === 'noSetup') {
    logWarn('auth.2fa_activation_failed', { userId: user.id, reason: result })
    throw createError({ statusCode: 403, message: 'errors.invalidCode' })
  }
  logEvent('auth.2fa_enabled', { userId: user.id, ip: clientIp(event) })
  return { backupCode: result.backupCode }
})
