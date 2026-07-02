import { recipeInputSchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const input = await readValidatedBody(event, data => recipeInputSchema.parse(data))
  const recipe = await createRecipe(useDb(), user.id, input)
  setResponseStatus(event, 201)
  return recipe
})
