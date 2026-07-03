import { planRecipeParamsSchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id, itemId } = await getValidatedRouterParams(event, data => planRecipeParamsSchema.parse(data))
  const result = await swapPlanRecipe(useDb(), user.id, id, itemId)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  return result.plan
})
