import { groceryItemsMergeSchema } from '#shared/schemas/grocery'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const { itemIds } = await readValidatedBody(event, data => groceryItemsMergeSchema.parse(data))
  const list = await mergeGroceryItems(useDb(), user.id, itemIds)
  if (!list) {
    throw createError({ statusCode: 404, message: 'errors.notFound' })
  }
  logEvent('grocery.items_merged', { userId: user.id, listId: list.id, count: itemIds.length })
  return { list }
})
