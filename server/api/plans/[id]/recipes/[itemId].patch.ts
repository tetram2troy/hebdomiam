import { planRecipeActionSchema, planRecipeParamsSchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id, itemId } = await getValidatedRouterParams(event, data => planRecipeParamsSchema.parse(data))
  const { action } = await readValidatedBody(event, data => planRecipeActionSchema.parse(data))
  const result = await updatePlanRecipeStatus(useDb(), user.id, id, itemId, action)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  return result.plan
})
