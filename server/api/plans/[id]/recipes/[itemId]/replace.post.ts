import { planRecipeParamsSchema, planReplaceBodySchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id, itemId } = await getValidatedRouterParams(event, data => planRecipeParamsSchema.parse(data))
  const { recipeId } = await readValidatedBody(event, data => planReplaceBodySchema.parse(data))
  const result = await replacePlanRecipe(useDb(), user.id, id, itemId, recipeId)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  return result.plan
})
