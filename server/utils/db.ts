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
