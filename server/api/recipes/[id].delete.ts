import { recipeIdParamsSchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => recipeIdParamsSchema.parse(data))
  const deleted = await softDeleteRecipe(useDb(), user.id, id)
  if (!deleted) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return { ok: true }
})
