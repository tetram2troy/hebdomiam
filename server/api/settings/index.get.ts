export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const profile = await getProfile(useDb(), user.id)
  if (!profile) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return profile
})
