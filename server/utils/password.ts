import { hash, verify } from '@node-rs/argon2'

// Argon2id, paramètres recommandés OWASP : 19 Mio de mémoire, 2 itérations.
// Nommés *UserPassword pour ne pas entrer en collision avec les hashPassword /
// verifyPassword (scrypt) auto-importés par nuxt-auth-utils.
const ARGON2_OPTIONS = {
  memoryCost: 19456,
  timeCost: 2,
  parallelism: 1,
}

export function hashUserPassword(password: string): Promise<string> {
  return hash(password, ARGON2_OPTIONS)
}

export function verifyUserPassword(hashed: string, password: string): Promise<boolean> {
  return verify(hashed, password)
}
