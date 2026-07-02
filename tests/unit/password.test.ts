import { describe, expect, it } from 'vitest'
import { hashUserPassword, verifyUserPassword } from '../../server/utils/password'

describe('hashUserPassword / verifyUserPassword', () => {
  it('vérifie un mot de passe correct', async () => {
    const hashed = await hashUserPassword('mon-mot-de-passe-secret')
    expect(await verifyUserPassword(hashed, 'mon-mot-de-passe-secret')).toBe(true)
  })

  it('rejette un mot de passe incorrect', async () => {
    const hashed = await hashUserPassword('mon-mot-de-passe-secret')
    expect(await verifyUserPassword(hashed, 'mauvais-mot-de-passe')).toBe(false)
  })

  it('produit un hachage Argon2id salé (jamais deux fois le même)', async () => {
    const first = await hashUserPassword('mon-mot-de-passe-secret')
    const second = await hashUserPassword('mon-mot-de-passe-secret')
    expect(first).toMatch(/^\$argon2id\$/)
    expect(first).not.toBe(second)
  })
})
