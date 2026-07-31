// Exécuteur de migrations pour la production — copié par le Dockerfile dans
// .output/server/ afin de résoudre pg et drizzle-orm depuis
// .output/server/node_modules (dépendances externalisées par Nitro).
// Usage : node .output/server/migrate.mjs — en job dédié au déploiement,
// jamais au boot de l'app (PRD §7.3).
import { fileURLToPath } from 'node:url'
import { drizzle } from 'drizzle-orm/node-postgres'
import { migrate } from 'drizzle-orm/node-postgres/migrator'
import pg from 'pg'

const url = process.env.NUXT_DATABASE_URL
if (!url) {
  console.error('[migrate] NUXT_DATABASE_URL manquant — rien à migrer sans base.')
  process.exit(1)
}

const pool = new pg.Pool({ connectionString: url, max: 1 })
const db = drizzle(pool)

try {
  await migrate(db, { migrationsFolder: fileURLToPath(new URL('./migrations', import.meta.url)) })
  console.log(JSON.stringify({ time: new Date().toISOString(), level: 'info', event: 'db.migrated' }))
}
catch (error) {
  console.error('[migrate] échec :', error instanceof Error ? error.message : error)
  process.exitCode = 1
}
finally {
  await pool.end()
}
