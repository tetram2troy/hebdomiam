import { describe, expect, it } from 'vitest'
import { KNOWN_KEYS, errorMessage } from '../../app/utils/error-message'
import en from '../../i18n/locales/en.json'
import fr from '../../i18n/locales/fr.json'

// Une clé de KNOWN_KEYS sans traduction passe l'allowlist puis s'affiche
// brute à l'écran (« errors.planExists ») — vécu : les quatre erreurs de
// planification sont restées deux jalons sans traduction.

function hasKey(locale: Record<string, unknown>, dotted: string): boolean {
  const [ns, key] = dotted.split('.')
  const section = locale[ns!]
  return typeof section === 'object' && section !== null && key! in section
}

describe('KNOWN_KEYS', () => {
  it.each([...KNOWN_KEYS])('%s est traduite en FR et en EN', (key) => {
    expect(hasKey(fr, key), `clé manquante dans fr.json : ${key}`).toBe(true)
    expect(hasKey(en, key), `clé manquante dans en.json : ${key}`).toBe(true)
  })

  it('les clés de repli existent aussi dans les deux locales', () => {
    for (const key of ['errors.invalidInput', 'errors.unknown']) {
      expect(hasKey(fr, key)).toBe(true)
      expect(hasKey(en, key)).toBe(true)
    }
  })
})

describe('errorMessage', () => {
  const t = (key: string) => `t:${key}`

  it('traduit une clé connue renvoyée par le serveur', () => {
    expect(errorMessage({ statusCode: 409, data: { message: 'errors.planExists' } }, t))
      .toBe('t:errors.planExists')
  })

  it('replie une clé inconnue sur invalidInput (400) ou unknown', () => {
    expect(errorMessage({ statusCode: 400, data: { message: 'errors.inventee' } }, t))
      .toBe('t:errors.invalidInput')
    expect(errorMessage({ statusCode: 500 }, t)).toBe('t:errors.unknown')
  })
})
