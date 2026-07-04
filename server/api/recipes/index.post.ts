import { recipeInputSchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const input = await readValidatedBody(event, data => recipeInputSchema.parse(data))
  const recipe = await createRecipe(useDb(), user.id, input)
  logEvent('recipe.created', { userId: user.id, recipeId: recipe.id })
  setResponseStatus(event, 201)
  return recipe
})
