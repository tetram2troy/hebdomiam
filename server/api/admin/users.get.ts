export default defineEventHandler(async (event) => {
  await requireAdminSession(event)
  return listUsers(useDb())
})
