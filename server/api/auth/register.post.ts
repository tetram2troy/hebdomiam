import { registerSchema } from '#shared/schemas/auth'
import { users } from '../../database/schema'

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:register', { max: 5, windowMs: 60_000 })

  const body = await readValidatedBody(event, data => registerSchema.parse(data))
  const passwordHash = await hashUserPassword(body.password)

  const [user] = await useDb()
    .insert(users)
    .values({ email: body.email, passwordHash, name: body.name })
    .onConflictDoNothing({ target: users.email })
    .returning({ id: users.id, email: users.email, name: users.name })

  if (!user) {
    throw createError({ statusCode: 409, message: 'errors.emailTaken' })
  }

  await setUserSession(event, { user })
  setResponseStatus(event, 201)
  return { user }
})
