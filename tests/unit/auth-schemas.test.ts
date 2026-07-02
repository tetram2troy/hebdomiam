import { describe, expect, it } from 'vitest'
import { loginSchema, registerSchema } from '../../shared/schemas/auth'

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
})

describe('loginSchema', () => {
  it('accepte une entrée valide', () => {
    expect(() => loginSchema.parse({ email: 'fab@example.com', password: 'x' })).not.toThrow()
  })

  it('rejette un mot de passe vide', () => {
    expect(() => loginSchema.parse({ email: 'fab@example.com', password: '' })).toThrow()
  })
})
