import { adminUpdateUserSchema, adminUserIdParamsSchema } from '#shared/schemas/admin'

export default defineEventHandler(async (event) => {
  const { user } = await requireAdminSession(event)
  assertRateLimit(event, `admin:update:${user.id}`, { max: 30, windowMs: 60_000 })
  const { id: targetId } = await getValidatedRouterParams(event, data => adminUserIdParamsSchema.parse(data))
  const patch = await readValidatedBody(event, data => adminUpdateUserSchema.parse(data))

  const result = await updateUserAsAdmin(useDb(), user.id, targetId, patch)
  if (typeof result === 'string') {
    logWarn('admin.user_update_refused', { userId: user.id, targetUserId: targetId, reason: result, ip: clientIp(event) })
    const responses = {
      notFound: { statusCode: 404, message: 'errors.notFound' },
      self: { statusCode: 400, message: 'errors.adminSelf' },
      lastAdmin: { statusCode: 409, message: 'errors.lastAdmin' },
    } as const
    throw createError(responses[result])
  }

  logEvent('admin.user_updated', { userId: user.id, targetUserId: targetId, fields: Object.keys(patch), ip: clientIp(event) })
  return result
})
