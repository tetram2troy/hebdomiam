import { planIdParamsSchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => planIdParamsSchema.parse(data))
  const result = await validatePlan(useDb(), user.id, id)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  logEvent('plan.validated', { userId: user.id, planId: id })
  return result.plan
})
