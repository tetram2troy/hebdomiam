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
    // Le courriel visé (pas le mot de passe !) : détecte le brute-force ciblé.
    logWarn('auth.login_failed', { email: body.email, ip: clientIp(event) })
    throw createError({ statusCode: 401, message: 'errors.invalidCredentials' })
  }

  // Compte désactivé : refusé seulement après validation du mot de passe —
  // on ne révèle rien à qui ne connaît pas les identifiants.
  if (user.disabledAt) {
    logWarn('auth.login_disabled', { userId: user.id, ip: clientIp(event) })
    throw createError({ statusCode: 403, message: 'errors.accountDisabled' })
  }

  // 2FA active : mot de passe validé mais session incomplète tant que le code
  // TOTP n'est pas fourni (POST /api/auth/2fa).
  if (user.totpEnabledAt) {
    await replaceUserSession(event, { pendingTotpUserId: user.id })
    logEvent('auth.login_2fa_pending', { userId: user.id, ip: clientIp(event) })
    return { requires2fa: true as const }
  }

  const publicUser = { id: user.id, email: user.email, name: user.name, role: user.role }
  await setUserSession(event, { user: publicUser })
  logEvent('auth.login', { userId: user.id, ip: clientIp(event) })
  return { user: publicUser }
})
