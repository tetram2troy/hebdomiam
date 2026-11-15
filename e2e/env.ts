// Base de données de la suite e2e (celle du serveur testé). DATABASE_URL, avec
// repli sur l'ancien nom NUXT_DATABASE_URL pour un .env pas encore migré.
export const DB_URL = process.env.DATABASE_URL ?? process.env.NUXT_DATABASE_URL ?? ''

export const E2E_PORT = 3000
export const BASE_URL = `http://127.0.0.1:${E2E_PORT}`
