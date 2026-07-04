import { groceryItemCreateSchema } from '#shared/schemas/grocery'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const input = await readValidatedBody(event, data => groceryItemCreateSchema.parse(data))
  const list = await addManualGroceryItem(useDb(), user.id, input)
  if (!list) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  logEvent('grocery.item_added', { userId: user.id, listId: list.id })
  setResponseStatus(event, 201)
  return { list }
})
