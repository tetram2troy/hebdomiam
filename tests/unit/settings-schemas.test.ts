import { describe, expect, it } from 'vitest'
import {
  accountDeleteSchema,
  passwordChangeSchema,
  profileUpdateSchema,
} from '../../shared/schemas/settings'

describe('profileUpdateSchema', () => {
  it('accepte un patch partiel', () => {
    expect(profileUpdateSchema.parse({ name: ' Fabrice ' })).toEqual({ name: 'Fabrice' })
    expect(profileUpdateSchema.parse({ rotationWeeks: 5 })).toEqual({ rotationWeeks: 5 })
  })

  it('rejette un patch vide', () => {
    expect(() => profileUpdateSchema.parse({})).toThrow()
  })

  it('rejette une locale hors fr/en', () => {
    expect(() => profileUpdateSchema.parse({ locale: 'de' })).toThrow()
  })

  it('rejette un système d\'unités inconnu', () => {
    expect(() => profileUpdateSchema.parse({ unitSystem: 'us' })).toThrow()
  })

  it('borne les valeurs de planification', () => {
    expect(() => profileUpdateSchema.parse({ defaultDishCount: 0 })).toThrow()
    expect(() => profileUpdateSchema.parse({ defaultDishCount: 11 })).toThrow()
    expect(() => profileUpdateSchema.parse({ defaultPeopleCount: 13 })).toThrow()
    expect(() => profileUpdateSchema.parse({ defaultDaysCount: 8 })).toThrow()
    expect(() => profileUpdateSchema.parse({ rotationWeeks: 0 })).toThrow()
    expect(() => profileUpdateSchema.parse({ rotationWeeks: 13 })).toThrow()
  })

  it('rejette les valeurs non entières', () => {
    expect(() => profileUpdateSchema.parse({ rotationWeeks: 2.5 })).toThrow()
  })

  it('rejette un nom vide', () => {
    expect(() => profileUpdateSchema.parse({ name: '   ' })).toThrow()
  })
})

describe('passwordChangeSchema', () => {
  it('accepte un changement valide', () => {
    expect(() => passwordChangeSchema.parse({
      currentPassword: 'x',
      newPassword: 'mot-de-passe-solide',
    })).not.toThrow()
  })

  it('rejette un nouveau mot de passe de moins de 12 caractères', () => {
    expect(() => passwordChangeSchema.parse({ currentPassword: 'x', newPassword: 'court' })).toThrow()
  })

  it('rejette un mot de passe actuel vide', () => {
    expect(() => passwordChangeSchema.parse({ currentPassword: '', newPassword: 'mot-de-passe-solide' })).toThrow()
  })
})

describe('accountDeleteSchema', () => {
  it('exige le mot de passe', () => {
    expect(() => accountDeleteSchema.parse({ password: '' })).toThrow()
    expect(() => accountDeleteSchema.parse({ password: 'mon-mot-de-passe' })).not.toThrow()
  })
})
