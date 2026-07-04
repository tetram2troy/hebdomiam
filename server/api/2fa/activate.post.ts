import { totpCodeSchema } from '#shared/schemas/auth'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `2fa:activate:${user.id}`, { max: 5, windowMs: 60_000 })
  const { code } = await readValidatedBody(event, data => totpCodeSchema.parse(data))

  const result = await activateTotp(useDb(), user.id, code)
  if (result === 'invalidCode' || result === 'noSetup') {
    throw createError({ statusCode: 403, message: 'errors.invalidCode' })
  }
  return { backupCode: result.backupCode }
})
