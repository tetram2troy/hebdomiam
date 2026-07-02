import { recipeIdParamsSchema, recipeInputSchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => recipeIdParamsSchema.parse(data))
  const input = await readValidatedBody(event, data => recipeInputSchema.parse(data))
  const recipe = await updateRecipe(useDb(), user.id, id, input)
  if (!recipe) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return recipe
})
