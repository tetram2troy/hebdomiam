import { eq } from 'drizzle-orm'
import { users } from '../../../database/schema'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `verify:request:${user.id}`, { max: 3, windowMs: 10 * 60_000 })

  const row = await useDb().query.users.findFirst({ where: eq(users.id, user.id) })
  if (!row || row.emailVerifiedAt) return { ok: true }

  const token = await createEmailToken(useDb(), user.id, 'verify', 24 * 60)
  const link = `${appBaseUrl(getRequestURL(event).origin)}/verifier-courriel?token=${token}`
  await sendMail(verifyEmailMail(row.email, row.locale, link))
  return { ok: true }
})
