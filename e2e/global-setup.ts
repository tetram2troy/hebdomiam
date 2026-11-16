import { hash } from '@node-rs/argon2'
import pg from 'pg'
import { cleanupTestAccounts } from './cleanup'
import { DB_URL } from './env'
import { ADMIN_EMAIL, ADMIN_PASSWORD } from './helpers'
import { startServer } from './server'

// 1. Démarre le serveur testé (binaire Rust ou conteneur — voir e2e/server.ts),
//    qui applique aussi les migrations.
// 2. Sème le compte admin de la suite. Dérogation assumée au principe « e2e via
//    l'UI » : l'inscription publique est fermée, l'admin est donc provisionné
//    directement en base — l'équivalent du provisioning d'une instance réelle.
//    Upsert idempotent : même chemin sur la DB de dev peuplée et en CI.
// 3. Retourne l'arrêt de fin de suite : suppression des comptes de test
//    (e2e/cleanup.ts), puis arrêt du serveur.
export default async function globalSetup() {
  if (!DB_URL) {
    throw new Error('DATABASE_URL manquant : charge le .env avant npm run test:e2e')
  }

  // Garde-fou : ce seed écrit un admin dont le mot de passe est public (dans le
  // dépôt). Refuser toute base non locale — un .env pointant sur le home lab ne
  // doit jamais y créer une porte dérobée.
  const host = new URL(DB_URL).hostname
  if (!['localhost', '127.0.0.1', '::1'].includes(host) && process.env.HEBDOMIAM_E2E_ALLOW_REMOTE_DB !== '1') {
    throw new Error(`global-setup e2e refuse la base non locale « ${host} » : il y sèmerait un admin à mot de passe connu`)
  }

  const stopServer = await startServer()

  // Mêmes paramètres Argon2id que l'API (api/src/crypto.rs).
  const passwordHash = await hash(ADMIN_PASSWORD, { memoryCost: 19456, timeCost: 2, parallelism: 1 })
  const client = new pg.Client({ connectionString: DB_URL })
  await client.connect()
  try {
    await client.query(
      `INSERT INTO users (email, password_hash, name, role, email_verified_at)
       VALUES ($1, $2, 'Admin E2E', 'admin', now())
       ON CONFLICT (email) DO UPDATE
       SET password_hash = EXCLUDED.password_hash, role = 'admin', disabled_at = NULL, email_verified_at = now()`,
      [ADMIN_EMAIL, passwordHash],
    )
  }
  finally {
    await client.end()
  }

  // Fin de suite (réussie ou non) : comptes de test supprimés, puis serveur
  // arrêté — même si le nettoyage échoue.
  return async () => {
    try {
      const deleted = await cleanupTestAccounts()
      console.log(`[e2e] ${deleted} compte(s) de test supprimé(s)`)
    }
    finally {
      await stopServer()
    }
  }
}
