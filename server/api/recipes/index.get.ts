import { recipeListQuerySchema } from '#shared/schemas/recipe'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const query = await getValidatedQuery(event, data => recipeListQuerySchema.parse(data))
  return listRecipes(useDb(), user.id, query)
})
