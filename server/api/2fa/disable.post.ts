import { accountDeleteSchema } from '#shared/schemas/settings'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `2fa:disable:${user.id}`, { max: 5, windowMs: 60_000 })
  const { password } = await readValidatedBody(event, data => accountDeleteSchema.parse(data))

  const result = await disableTotp(useDb(), user.id, password)
  if (result !== 'ok') {
    logWarn('auth.2fa_disable_failed', { userId: user.id, ip: clientIp(event) })
    throw createError({ statusCode: 403, message: 'errors.invalidPassword' })
  }
  logEvent('auth.2fa_disabled', { userId: user.id, ip: clientIp(event) })
  return { ok: true }
})
