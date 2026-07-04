import { toString as qrToString } from 'qrcode'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `2fa:setup:${user.id}`, { max: 5, windowMs: 60_000 })

  const { secret, otpauthUrl } = await startTotpSetup(useDb(), user.id, user.email)
  const qrSvg = await qrToString(otpauthUrl, { type: 'svg', margin: 1, width: 220 })
  return { secret, otpauthUrl, qrSvg }
})
