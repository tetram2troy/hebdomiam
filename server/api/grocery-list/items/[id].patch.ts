import { groceryItemIdParamsSchema, groceryItemUpdateSchema } from '#shared/schemas/grocery'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { id } = await getValidatedRouterParams(event, data => groceryItemIdParamsSchema.parse(data))
  const patch = await readValidatedBody(event, data => groceryItemUpdateSchema.parse(data))
  const list = await updateGroceryItem(useDb(), user.id, id, patch)
  if (!list) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  return { list }
})
