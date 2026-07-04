import { accountDeleteSchema } from '#shared/schemas/settings'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `account:delete:${user.id}`, { max: 3, windowMs: 60_000 })
  const { password } = await readValidatedBody(event, data => accountDeleteSchema.parse(data))

  const result = await deleteAccount(useDb(), user.id, password)
  if (result !== 'ok') {
    logWarn('account.delete_failed', { userId: user.id, ip: clientIp(event) })
    throw createError({
      statusCode: result === 'invalidPassword' ? 403 : 404,
      message: result === 'invalidPassword' ? 'errors.invalidPassword' : 'errors.notFound',
    })
  }
  await clearUserSession(event)
  logEvent('account.deleted', { userId: user.id, ip: clientIp(event) })
  return { ok: true }
})
