import { migrate } from 'drizzle-orm/node-postgres/migrator'
import pg from 'pg'
import { createDbClient } from '../../server/database/client'
import { TEST_DATABASE_URL } from './test-db'

// Crée la base de test si absente, puis applique les migrations Drizzle.
export default async function setup() {
  const testUrl = new URL(TEST_DATABASE_URL)
  const dbName = testUrl.pathname.slice(1)

  const adminUrl = new URL(TEST_DATABASE_URL)
  adminUrl.pathname = `/${process.env.TEST_ADMIN_DB ?? 'hebdomiam'}`

  const admin = new pg.Client({ connectionString: adminUrl.toString() })
  await admin.connect()
  const exists = await admin.query('SELECT 1 FROM pg_database WHERE datname = $1', [dbName])
  if (exists.rowCount === 0) {
    await admin.query(`CREATE DATABASE "${dbName}"`)
  }
  await admin.end()

  const db = createDbClient(TEST_DATABASE_URL)
  await migrate(db, { migrationsFolder: 'server/database/migrations' })
  await db.$client.end()
}
