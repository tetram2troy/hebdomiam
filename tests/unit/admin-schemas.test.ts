import { describe, expect, it } from 'vitest'
import {
  adminCreateUserSchema,
  adminUpdateUserSchema,
  adminUserIdParamsSchema,
} from '../../shared/schemas/admin'

describe('adminCreateUserSchema', () => {
  const valid = { email: 'invite@example.com', name: 'Invitée' }

  it('accepte une entrée valide', () => {
    expect(adminCreateUserSchema.parse(valid)).toEqual(valid)
  })

  it('normalise le courriel (casse et espaces)', () => {
    const result = adminCreateUserSchema.parse({ ...valid, email: '  Invite@Example.COM ' })
    expect(result.email).toBe('invite@example.com')
  })

  it('retire les espaces du nom et rejette un nom vide', () => {
    expect(adminCreateUserSchema.parse({ ...valid, name: ' Invitée ' }).name).toBe('Invitée')
    expect(() => adminCreateUserSchema.parse({ ...valid, name: '   ' })).toThrow()
  })

  it('rejette un courriel invalide', () => {
    expect(() => adminCreateUserSchema.parse({ ...valid, email: 'pas-un-courriel' })).toThrow()
  })

  it('ignore les champs inconnus — pas de rôle ni de mot de passe à la création', () => {
    const result = adminCreateUserSchema.parse({ ...valid, role: 'admin', passwordHash: 'x' })
    expect(result).not.toHaveProperty('role')
    expect(result).not.toHaveProperty('passwordHash')
  })
})

describe('adminUpdateUserSchema', () => {
  it('accepte un patch de rôle seul', () => {
    expect(adminUpdateUserSchema.parse({ role: 'admin' })).toEqual({ role: 'admin' })
  })

  it('accepte un patch de désactivation seul', () => {
    expect(adminUpdateUserSchema.parse({ disabled: true })).toEqual({ disabled: true })
  })

  it('rejette un patch vide', () => {
    expect(() => adminUpdateUserSchema.parse({})).toThrow()
  })

  it('rejette un rôle hors domaine', () => {
    expect(() => adminUpdateUserSchema.parse({ role: 'superadmin' })).toThrow()
  })

  it('rejette un booléen non strict', () => {
    expect(() => adminUpdateUserSchema.parse({ disabled: 'oui' })).toThrow()
  })
})

describe('adminUserIdParamsSchema', () => {
  it('accepte un UUID', () => {
    expect(() => adminUserIdParamsSchema.parse({ id: 'c1b0e6cc-6f4e-4b8f-9d7e-2f4a5b6c7d8e' })).not.toThrow()
  })

  it('rejette un identifiant non UUID', () => {
    expect(() => adminUserIdParamsSchema.parse({ id: '1; DROP TABLE users' })).toThrow()
  })
})
