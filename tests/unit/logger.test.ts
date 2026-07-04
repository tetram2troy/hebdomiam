import { afterEach, describe, expect, it, vi } from 'vitest'
import { logError, logEvent, logWarn } from '../../server/utils/logger'

function lastLine(spy: ReturnType<typeof vi.spyOn>): Record<string, unknown> {
  return JSON.parse(spy.mock.calls.at(-1)![0] as string)
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('logEvent', () => {
  it('émet une ligne JSON structurée avec horodatage ISO et niveau info', () => {
    const info = vi.spyOn(console, 'info').mockImplementation(() => {})
    logEvent('recipe.created', { userId: 'u1', recipeId: 'r1' })

    const line = lastLine(info)
    expect(line).toMatchObject({ level: 'info', event: 'recipe.created', userId: 'u1', recipeId: 'r1' })
    expect(new Date(line.time as string).toISOString()).toBe(line.time)
  })

  it('fonctionne sans champs additionnels', () => {
    const info = vi.spyOn(console, 'info').mockImplementation(() => {})
    logEvent('auth.logout')
    expect(lastLine(info)).toMatchObject({ level: 'info', event: 'auth.logout' })
  })
})

describe('logWarn', () => {
  it('émet sur console.warn au niveau warn', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    logWarn('auth.login_failed', { ip: '203.0.113.9' })
    expect(lastLine(warn)).toMatchObject({ level: 'warn', event: 'auth.login_failed', ip: '203.0.113.9' })
  })
})

describe('logError', () => {
  it('extrait le message d\'une Error sans embarquer l\'objet complet', () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {})
    logError('mailer.verify_send_failed', new Error('ECONNREFUSED'), { userId: 'u1' })
    expect(lastLine(error)).toMatchObject({
      level: 'error',
      event: 'mailer.verify_send_failed',
      userId: 'u1',
      message: 'ECONNREFUSED',
    })
  })

  it('accepte une valeur levée qui n\'est pas une Error', () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {})
    logError('import.crashed', 'boom')
    expect(lastLine(error)).toMatchObject({ message: 'boom' })
  })
})
