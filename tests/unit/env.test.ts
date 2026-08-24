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

  it('exige NUXT_PUBLIC_BASE_URL et SMTP_HOST en production réelle', () => {
    const problems = envProblems({ ...VALID, NODE_ENV: 'production' })
    expect(problems).toHaveLength(2)
    expect(problems[0]).toContain('NUXT_PUBLIC_BASE_URL')
    expect(problems[1]).toContain('SMTP_HOST')
  })

  it('accepte une configuration de production complète', () => {
    expect(envProblems({
      ...VALID,
      NODE_ENV: 'production',
      NUXT_PUBLIC_BASE_URL: 'https://recettes.example.com',
      SMTP_HOST: 'smtp-relay.brevo.com',
    })).toEqual([])
  })

  it('ne réclame pas SMTP hors production (serveur e2e local, courriels en console)', () => {
    expect(envProblems(VALID)).toEqual([])
  })
})
