import { sql } from 'drizzle-orm'
import { useDb } from '../utils/db'

// Sonde de santé (HEALTHCHECK Docker, Uptime Kuma, webServer Playwright) :
// vérifie ce qui compte — la base répond — sans authentification et sans
// bruit de journalisation (une ligne toutes les 30 s ne dit rien d'utile).
export default defineEventHandler(async (event) => {
  try {
    await useDb().execute(sql`SELECT 1`)
    return { status: 'ok' }
  }
  catch {
    setResponseStatus(event, 503)
    return { status: 'db-unreachable' }
  }
})
