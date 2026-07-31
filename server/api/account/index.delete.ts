import { accountDeleteSchema } from '#shared/schemas/settings'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `account:delete:${user.id}`, { max: 3, windowMs: 60_000 })
  const { password } = await readValidatedBody(event, data => accountDeleteSchema.parse(data))

  const result = await deleteAccount(useDb(), user.id, password)
  if (result !== 'ok') {
    logWarn('account.delete_failed', { userId: user.id, ip: clientIp(event), reason: result })
    // lastAdmin : le dernier admin actif ne peut pas s'auto-supprimer,
    // l'instance deviendrait ingérable (même garde-fou que les routes admin).
    const responses = {
      invalidPassword: { statusCode: 403, message: 'errors.invalidPassword' },
      notFound: { statusCode: 404, message: 'errors.notFound' },
      lastAdmin: { statusCode: 409, message: 'errors.lastAdmin' },
    } as const
    throw createError(responses[result])
  }
  await clearUserSession(event)
  logEvent('account.deleted', { userId: user.id, ip: clientIp(event) })
  return { ok: true }
})
