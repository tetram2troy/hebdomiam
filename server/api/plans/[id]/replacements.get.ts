import { planIdParamsSchema, replacementQuerySchema } from '#shared/schemas/plan'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => planIdParamsSchema.parse(data))
  const query = await getValidatedQuery(event, data => replacementQuerySchema.parse(data))
  const result = await listReplacementOptions(useDb(), user.id, id, query)
  if ('error' in result) {
    throw createError({ statusCode: PLAN_ERROR_STATUS[result.error], message: `errors.${result.error}` })
  }
  return result
})
