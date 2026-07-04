import { planCreateSchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const input = await readValidatedBody(event, data => planCreateSchema.parse(data))
  const result = await createPlan(useDb(), user.id, input)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  logEvent('plan.created', { userId: user.id, planId: result.plan.id, weekStartDate: result.plan.weekStartDate })
  setResponseStatus(event, 201)
  return result.plan
})
