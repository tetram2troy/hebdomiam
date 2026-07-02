import { createDbClient, type Db } from '../../server/database/client'
import { users } from '../../server/database/schema'

export const TEST_DATABASE_URL = process.env.TEST_DATABASE_URL
  ?? 'postgresql://hebdomiam:hebdomiam@localhost:5432/hebdomiam_test'

export function connectTestDb(): Db {
  return createDbClient(TEST_DATABASE_URL)
}

/** Vide toutes les tables (users CASCADE emporte recettes, tags, etc.). */
export async function truncateAll(db: Db) {
  await db.execute('TRUNCATE TABLE users CASCADE')
}

let userCounter = 0

/** Insère un utilisateur de test (hash factice : on ne teste pas l'auth ici). */
export async function createTestUser(db: Db, name = 'Testeur') {
  userCounter += 1
  const [user] = await db.insert(users).values({
    email: `test-${userCounter}-${process.pid}@example.com`,
    passwordHash: '$argon2id$fake-hash-for-tests',
    name,
  }).returning({ id: users.id, email: users.email })
  return user!
}
