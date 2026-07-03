import { drizzle, type NodePgDatabase } from 'drizzle-orm/node-postgres'
import pg from 'pg'
import * as schema from './schema'

export type Db = NodePgDatabase<typeof schema>

// Factory sans dépendance à Nuxt : utilisée par useDb() en production et par les
// tests d'intégration (base éphémère) sans passer par useRuntimeConfig.
export function createDbClient(connectionString: string): Db {
  const pool = new pg.Pool({
    connectionString,
    // SSR + API en parallèle : le défaut (10) peut s'épuiser en pointe, et une
    // attente sans limite se manifeste en requêtes suspendues — on préfère
    // échouer bruyamment après 10 s.
    max: 20,
    connectionTimeoutMillis: 10000,
  })
  return drizzle(pool, { schema })
}
