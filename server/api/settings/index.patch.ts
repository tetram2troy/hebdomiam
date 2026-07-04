import { profileUpdateSchema } from '#shared/schemas/settings'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const patch = await readValidatedBody(event, data => profileUpdateSchema.parse(data))
  const profile = await updateProfile(useDb(), user.id, patch)
  if (!profile) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  if (patch.name) {
    await setUserSession(event, { user: { ...user, name: patch.name } })
  }
  // Les clés modifiées suffisent — les valeurs n'apportent rien à l'audit.
  logEvent('settings.updated', { userId: user.id, fields: Object.keys(patch) })
  return profile
})
