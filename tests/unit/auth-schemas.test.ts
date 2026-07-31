import { describe, expect, it } from 'vitest'
import {
  acceptInviteSchema,
  forgotPasswordSchema,
  loginSchema,
  registerSchema,
  resetPasswordSchema,
  totpCodeSchema,
  verifyEmailSchema,
} from '../../shared/schemas/auth'

describe('registerSchema', () => {
  const valid = {
    email: 'fab@example.com',
    password: 'un-mot-de-passe-solide',
    name: 'Fabrice',
  }

  it('accepte une entrée valide', () => {
    expect(registerSchema.parse(valid)).toEqual(valid)
  })

  it('normalise le courriel (casse et espaces)', () => {
    const result = registerSchema.parse({ ...valid, email: '  Fab@Example.COM ' })
    expect(result.email).toBe('fab@example.com')
  })

  it('rejette un courriel invalide', () => {
    expect(() => registerSchema.parse({ ...valid, email: 'pas-un-courriel' })).toThrow()
  })

  it('rejette un mot de passe de moins de 12 caractères', () => {
    expect(() => registerSchema.parse({ ...valid, password: 'court' })).toThrow()
  })

  it('rejette un nom vide', () => {
    expect(() => registerSchema.parse({ ...valid, name: '   ' })).toThrow()
  })

  it('ignore les champs inconnus', () => {
    const result = registerSchema.parse({ ...valid, isAdmin: true })
    expect(result).not.toHaveProperty('isAdmin')
  })

  it('ignore un rôle injecté — le rôle ne vient jamais du corps de requête', () => {
    const result = registerSchema.parse({ ...valid, role: 'admin' })
    expect(result).not.toHaveProperty('role')
  })
})

describe('loginSchema', () => {
  it('accepte une entrée valide', () => {
    expect(() => loginSchema.parse({ email: 'fab@example.com', password: 'x' })).not.toThrow()
  })

  it('rejette un mot de passe vide', () => {
    expect(() => loginSchema.parse({ email: 'fab@example.com', password: '' })).toThrow()
  })
})

describe('totpCodeSchema', () => {
  it('accepte un code à 6 chiffres et retire les espaces', () => {
    expect(totpCodeSchema.parse({ code: ' 123456 ' })).toEqual({ code: '123456' })
  })

  it('accepte un code de secours de 12 caractères', () => {
    expect(() => totpCodeSchema.parse({ code: 'a1b2c3d4e5f6' })).not.toThrow()
  })

  it('rejette un code trop court ou trop long', () => {
    expect(() => totpCodeSchema.parse({ code: '12345' })).toThrow()
    expect(() => totpCodeSchema.parse({ code: 'x'.repeat(17) })).toThrow()
  })
})

describe('forgotPasswordSchema', () => {
  it('normalise le courriel', () => {
    expect(forgotPasswordSchema.parse({ email: ' Fab@Example.COM ' })).toEqual({ email: 'fab@example.com' })
  })

  it('rejette un courriel invalide', () => {
    expect(() => forgotPasswordSchema.parse({ email: 'pas-un-courriel' })).toThrow()
  })
})

describe('resetPasswordSchema', () => {
  const token = 'a'.repeat(48)

  it('accepte un jeton et un mot de passe valides', () => {
    expect(() => resetPasswordSchema.parse({ token, password: 'mot-de-passe-solide' })).not.toThrow()
  })

  it('rejette un jeton trop court (anti force brute)', () => {
    expect(() => resetPasswordSchema.parse({ token: 'court', password: 'mot-de-passe-solide' })).toThrow()
  })

  it('rejette un nouveau mot de passe de moins de 12 caractères', () => {
    expect(() => resetPasswordSchema.parse({ token, password: 'court' })).toThrow()
  })
})

describe('acceptInviteSchema', () => {
  const token = 'a'.repeat(48)

  it('accepte un jeton et un mot de passe valides', () => {
    expect(() => acceptInviteSchema.parse({ token, password: 'mot-de-passe-solide' })).not.toThrow()
  })

  it('rejette un jeton hors bornes (anti force brute)', () => {
    expect(() => acceptInviteSchema.parse({ token: 'court', password: 'mot-de-passe-solide' })).toThrow()
  })

  it('rejette un mot de passe de moins de 12 caractères', () => {
    expect(() => acceptInviteSchema.parse({ token, password: 'court' })).toThrow()
  })
})

describe('verifyEmailSchema', () => {
  it('accepte un jeton de 48 caractères', () => {
    expect(() => verifyEmailSchema.parse({ token: 'b'.repeat(48) })).not.toThrow()
  })

  it('rejette un jeton hors bornes', () => {
    expect(() => verifyEmailSchema.parse({ token: 'court' })).toThrow()
    expect(() => verifyEmailSchema.parse({ token: 'c'.repeat(129) })).toThrow()
  })
})
