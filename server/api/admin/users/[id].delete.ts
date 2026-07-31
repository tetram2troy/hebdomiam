import { adminUserIdParamsSchema } from '#shared/schemas/admin'

export default defineEventHandler(async (event) => {
  const { user } = await requireAdminSession(event)
  assertRateLimit(event, `admin:delete:${user.id}`, { max: 10, windowMs: 60_000 })
  const { id: targetId } = await getValidatedRouterParams(event, data => adminUserIdParamsSchema.parse(data))

  const result = await deleteUserAsAdmin(useDb(), user.id, targetId)
  if (result !== 'ok') {
    logWarn('admin.user_delete_refused', { userId: user.id, targetUserId: targetId, reason: result, ip: clientIp(event) })
    const responses = {
      notFound: { statusCode: 404, message: 'errors.notFound' },
      self: { statusCode: 400, message: 'errors.adminSelf' },
      lastAdmin: { statusCode: 409, message: 'errors.lastAdmin' },
    } as const
    throw createError(responses[result])
  }

  logEvent('admin.user_deleted', { userId: user.id, targetUserId: targetId, ip: clientIp(event) })
  return { ok: true }
})
