import { drizzle, type NodePgDatabase } from 'drizzle-orm/node-postgres'
import pg from 'pg'
import * as schema from './schema'

export type Db = NodePgDatabase<typeof schema>

// Factory sans dépendance à Nuxt : utilisée par useDb() en production et par les
// tests d'intégration (base éphémère) sans passer par useRuntimeConfig.
export function createDbClient(connectionString: string): Db {
  const pool = new pg.Pool({ connectionString })
  return drizzle(pool, { schema })
}
