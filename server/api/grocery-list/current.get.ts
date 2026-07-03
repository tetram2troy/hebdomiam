export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  const list = await getGroceryListForWeek(useDb(), user.id)
  return { list }
})
