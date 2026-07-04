import { createHash, randomBytes } from 'node:crypto'
import { generateSecret, generateURI, verifySync } from 'otplib'
import { eq } from 'drizzle-orm'
import type { Db } from '../database/client'
import { users } from '../database/schema'
import { verifyUserPassword } from './password'

// 2FA TOTP optionnelle (PRD §8, M5). Le secret est posé au setup et n'active
// la 2FA qu'après vérification d'un premier code ; un code de secours unique
// (haché) permet de rentrer si l'authenticateur est perdu.

/** Tolère ±30 s de dérive d'horloge entre le téléphone et le serveur. */
const EPOCH_TOLERANCE = 30

function verifyCode(secret: string, token: string): boolean {
  try {
    return verifySync({ secret, token, epochTolerance: EPOCH_TOLERANCE }).valid
  }
  catch {
    return false
  }
}

function hashBackupCode(code: string): string {
  return createHash('sha256').update(code).digest('hex')
}

export async function startTotpSetup(db: Db, userId: string, email: string) {
  const secret = generateSecret()
  await db.update(users).set({ totpSecret: secret, totpEnabledAt: null }).where(eq(users.id, userId))
  return {
    secret,
    otpauthUrl: generateURI({ issuer: 'Hebdomiam', label: email, secret }),
  }
}

export async function activateTotp(db: Db, userId: string, code: string): Promise<{ backupCode: string } | 'invalidCode' | 'noSetup'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { totpSecret: true, totpEnabledAt: true },
  })
  if (!user?.totpSecret || user.totpEnabledAt) return 'noSetup'
  if (!verifyCode(user.totpSecret, code)) return 'invalidCode'

  const backupCode = randomBytes(6).toString('hex') // 12 caractères
  await db.update(users)
    .set({ totpEnabledAt: new Date(), totpBackupHash: hashBackupCode(backupCode) })
    .where(eq(users.id, userId))
  return { backupCode }
}

export async function disableTotp(db: Db, userId: string, password: string): Promise<'ok' | 'invalidPassword'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { passwordHash: true },
  })
  if (!user || !(await verifyUserPassword(user.passwordHash, password))) return 'invalidPassword'
  await db.update(users)
    .set({ totpSecret: null, totpEnabledAt: null, totpBackupHash: null })
    .where(eq(users.id, userId))
  return 'ok'
}

/** Vérifie un code TOTP (ou le code de secours, consommé à l'usage) au login.
    Retourne la méthode acceptée — l'usage du code de secours mérite d'être
    journalisé (authenticateur perdu… ou compromis). */
export async function verifyTotpLogin(db: Db, userId: string, code: string): Promise<'totp' | 'backup' | null> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { totpSecret: true, totpEnabledAt: true, totpBackupHash: true },
  })
  if (!user?.totpSecret || !user.totpEnabledAt) return null

  if (verifyCode(user.totpSecret, code)) return 'totp'

  if (user.totpBackupHash && user.totpBackupHash === hashBackupCode(code.trim())) {
    await db.update(users).set({ totpBackupHash: null }).where(eq(users.id, userId))
    return 'backup'
  }
  return null
}

export async function isTotpEnabled(db: Db, userId: string): Promise<boolean> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { totpEnabledAt: true },
  })
  return Boolean(user?.totpEnabledAt)
}
