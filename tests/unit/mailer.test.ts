import { afterEach, describe, expect, it, vi } from 'vitest'
import { resetPasswordMail, sendMail, verifyEmailMail } from '../../server/utils/mailer'
import { appBaseUrl } from '../../server/utils/email-tokens'

const LINK = 'https://hebdomiam.example.com/verifier-courriel?token=abc123'

describe('verifyEmailMail', () => {
  it('produit un courriel français contenant le lien', () => {
    const mail = verifyEmailMail('fab@example.com', 'fr', LINK)
    expect(mail.to).toBe('fab@example.com')
    expect(mail.subject).toContain('confirme ton courriel')
    expect(mail.text).toContain(LINK)
  })

  it('produit un courriel anglais contenant le lien', () => {
    const mail = verifyEmailMail('fab@example.com', 'en', LINK)
    expect(mail.subject).toContain('confirm your email')
    expect(mail.text).toContain(LINK)
  })
})

describe('resetPasswordMail', () => {
  it('produit les deux locales avec le lien et la durée de validité', () => {
    const fr = resetPasswordMail('fab@example.com', 'fr', LINK)
    expect(fr.subject).toContain('réinitialise ton mot de passe')
    expect(fr.text).toContain(LINK)
    expect(fr.text).toContain('1 heure')

    const en = resetPasswordMail('fab@example.com', 'en', LINK)
    expect(en.subject).toContain('reset your password')
    expect(en.text).toContain('1 hour')
  })
})

describe('sendMail sans SMTP configuré', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('journalise le courriel au lieu de lever une erreur (mode dev)', async () => {
    // SMTP_HOST absent de l'environnement de test : le transporteur est nul.
    const info = vi.spyOn(console, 'info').mockImplementation(() => {})
    await expect(sendMail(verifyEmailMail('fab@example.com', 'fr', LINK))).resolves.toBeUndefined()
    expect(info).toHaveBeenCalledOnce()
    expect(info.mock.calls[0]![0]).toContain(LINK)
  })
})

describe('appBaseUrl', () => {
  afterEach(() => {
    delete process.env.NUXT_PUBLIC_BASE_URL
  })

  it('reprend l\'origine de la requête sans variable d\'environnement', () => {
    expect(appBaseUrl('http://127.0.0.1:3000')).toBe('http://127.0.0.1:3000')
  })

  it('privilégie NUXT_PUBLIC_BASE_URL et retire la barre oblique finale', () => {
    process.env.NUXT_PUBLIC_BASE_URL = 'https://hebdomiam.example.com/'
    expect(appBaseUrl('http://127.0.0.1:3000')).toBe('https://hebdomiam.example.com')
  })
})
