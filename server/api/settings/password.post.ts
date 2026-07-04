import { passwordChangeSchema } from '#shared/schemas/settings'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `settings:password:${user.id}`, { max: 5, windowMs: 60_000 })
  const { currentPassword, newPassword } = await readValidatedBody(event, data => passwordChangeSchema.parse(data))

  const result = await changePassword(useDb(), user.id, currentPassword, newPassword)
  if (result !== 'ok') {
    throw createError({
      statusCode: result === 'invalidPassword' ? 403 : 404,
      message: result === 'invalidPassword' ? 'errors.invalidPassword' : 'errors.notFound',
    })
  }
  return { ok: true }
})
