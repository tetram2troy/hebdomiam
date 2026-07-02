import { eq } from 'drizzle-orm'
import { loginSchema } from '#shared/schemas/auth'
import { users } from '../../database/schema'

// Hachage factice vérifié quand le courriel est inconnu : temps de réponse
// comparable dans les deux cas, pas d'énumération de comptes par timing.
let dummyHash: string | null = null

export default defineEventHandler(async (event) => {
  assertRateLimit(event, 'auth:login', { max: 5, windowMs: 60_000 })

  const body = await readValidatedBody(event, data => loginSchema.parse(data))

  const user = await useDb().query.users.findFirst({
    where: eq(users.email, body.email),
  })

  dummyHash ??= await hashUserPassword('hebdomiam-dummy-timing-password')
  const valid = await verifyUserPassword(user?.passwordHash ?? dummyHash, body.password)

  if (!user || !valid) {
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  const publicUser = { id: user.id, email: user.email, name: user.name }
  await setUserSession(event, { user: publicUser })
  return { user: publicUser }
})
