import nodemailer, { type Transporter } from 'nodemailer'

// Courriels transactionnels via le relais SMTP partagé du home lab (Brevo).
// Sans SMTP_* configuré (dev), les courriels sont journalisés en console.

let transporter: Transporter | null | undefined

function getTransporter(): Transporter | null {
  if (transporter !== undefined) return transporter
  const host = process.env.SMTP_HOST
  if (!host) {
    transporter = null
    return transporter
  }
  const port = Number(process.env.SMTP_PORT ?? 587)
  transporter = nodemailer.createTransport({
    host,
    port,
    secure: process.env.SMTP_SECURITY === 'tls' || port === 465,
    auth: process.env.SMTP_USERNAME
      ? { user: process.env.SMTP_USERNAME, pass: process.env.SMTP_PASSWORD }
      : undefined,
  })
  return transporter
}

export interface Mail {
  to: string
  subject: string
  text: string
}

export async function sendMail(mail: Mail): Promise<void> {
  const transport = getTransporter()
  const from = process.env.SMTP_FROM ?? 'hebdomiam@localhost'
  if (!transport) {
    console.info(`[courriel non envoyé — SMTP absent]\nÀ: ${mail.to}\nSujet: ${mail.subject}\n${mail.text}`)
    return
  }
  await transport.sendMail({ from, ...mail })
}

type MailLocale = 'fr' | 'en'

export function verifyEmailMail(to: string, locale: MailLocale, link: string): Mail {
  if (locale === 'en') {
    return {
      to,
      subject: 'Hebdomiam — confirm your email',
      text: `Hi!\n\nClick this link to confirm your email address:\n${link}\n\nThe link expires in 24 hours. If you didn't create an account, ignore this message.`,
    }
  }
  return {
    to,
    subject: 'Hebdomiam — confirme ton courriel',
    text: `Bonjour !\n\nClique ce lien pour confirmer ton adresse courriel :\n${link}\n\nLe lien expire dans 24 heures. Si tu n'as pas créé de compte, ignore ce message.`,
  }
}

export function inviteMail(to: string, locale: MailLocale, link: string): Mail {
  if (locale === 'en') {
    return {
      to,
      subject: 'Hebdomiam — you are invited',
      text: `Hi!\n\nAn administrator created an Hebdomiam account for you. Click this link to choose your password and activate it:\n${link}\n\nThe link expires in 7 days. If you weren't expecting this, ignore this message.`,
    }
  }
  return {
    to,
    subject: 'Hebdomiam — tu es invité(e)',
    text: `Bonjour !\n\nUn administrateur t'a créé un compte Hebdomiam. Clique ce lien pour choisir ton mot de passe et l'activer :\n${link}\n\nLe lien expire dans 7 jours. Si tu n'attendais pas cette invitation, ignore ce message.`,
  }
}

export function resetPasswordMail(to: string, locale: MailLocale, link: string): Mail {
  if (locale === 'en') {
    return {
      to,
      subject: 'Hebdomiam — reset your password',
      text: `Hi!\n\nClick this link to choose a new password:\n${link}\n\nThe link expires in 1 hour. If you didn't ask for this, ignore this message.`,
    }
  }
  return {
    to,
    subject: 'Hebdomiam — réinitialise ton mot de passe',
    text: `Bonjour !\n\nClique ce lien pour choisir un nouveau mot de passe :\n${link}\n\nLe lien expire dans 1 heure. Si tu n'es pas à l'origine de cette demande, ignore ce message.`,
  }
}
