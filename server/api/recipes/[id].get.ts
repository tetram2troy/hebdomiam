import { recipeIdParamsSchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => recipeIdParamsSchema.parse(data))
  const recipe = await getRecipe(useDb(), user.id, id)
  if (!recipe) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return recipe
})
