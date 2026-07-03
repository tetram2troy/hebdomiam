import { groceryItemIdParamsSchema } from '#shared/schemas/grocery'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => groceryItemIdParamsSchema.parse(data))
  const list = await deleteGroceryItem(useDb(), user.id, id)
  if (!list) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return { list }
})
