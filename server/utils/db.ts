import { drizzle, type NodePgDatabase } from 'drizzle-orm/node-postgres'
import pg from 'pg'
import * as schema from '../database/schema'

let db: NodePgDatabase<typeof schema> | null = null

export function useDb() {
  if (!db) {
    const { databaseUrl } = useRuntimeConfig()
    if (!databaseUrl) {
      throw new Error('NUXT_DATABASE_URL manquant')
    }
    const pool = new pg.Pool({ connectionString: databaseUrl })
    db = drizzle(pool, { schema })
  }
  return db
}
