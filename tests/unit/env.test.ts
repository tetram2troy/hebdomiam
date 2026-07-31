import { describe, expect, it } from 'vitest'
import { envProblems } from '../../server/utils/env'

const VALID = {
  NUXT_DATABASE_URL: 'postgresql://hebdomiam:x@db:5432/hebdomiam',
  NUXT_SESSION_PASSWORD: 'a'.repeat(32),
}

describe('envProblems', () => {
  it('accepte une configuration complète', () => {
    expect(envProblems(VALID)).toEqual([])
  })

  it('signale chaque variable manquante avec un message actionnable', () => {
    const problems = envProblems({})
    expect(problems).toHaveLength(2)
    expect(problems[0]).toContain('NUXT_DATABASE_URL')
    expect(problems[1]).toContain('NUXT_SESSION_PASSWORD')
  })

  it('refuse un mot de passe de session trop court', () => {
    const problems = envProblems({ ...VALID, NUXT_SESSION_PASSWORD: 'court' })
    expect(problems).toHaveLength(1)
    expect(problems[0]).toContain('32 caractères')
  })
})
