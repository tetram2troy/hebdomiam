import { planIdParamsSchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => planIdParamsSchema.parse(data))
  const plan = await getPlan(useDb(), user.id, id)
  if (!plan) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return plan
})
