import { createDbClient, type Db } from '../database/client'

let db: Db | null = null

export function useDb() {
  if (!db) {
    const { databaseUrl } = useRuntimeConfig()
    if (!databaseUrl) {
      throw new Error('NUXT_DATABASE_URL manquant')
    }
    db = createDbClient(databaseUrl)
  }
  return db
}

/** Ferme le pool pg à l'arrêt du serveur (hook Nitro `close`, docs/HOSTING.md §2.7). */
export async function closeDb() {
  if (db) {
    await db.$client.end()
    db = null
  }
}
