//! Courriels transactionnels via le relais SMTP partagé du home lab (Brevo).
//! Sans SMTP configuré (dev, e2e), les courriels sont journalisés en console.

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;
use crate::domain::catalog::Locale;

#[derive(Debug, Clone, PartialEq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: String,
}

impl Mailer {
    pub fn new(smtp: Option<SmtpConfig>, from: String) -> Self {
        let transport = smtp.and_then(|config| {
            let implicit_tls = config.security == "tls" || config.port == 465;
            let builder = if config.security == "none" {
                Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host))
            } else if implicit_tls {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            };
            let mut builder = match builder {
                Ok(builder) => builder.port(config.port),
                Err(error) => {
                    crate::logger::log_error("mailer.config_invalid", &error, serde_json::json!({}));
                    return None;
                }
            };
            if let Some(username) = config.username {
                builder = builder.credentials(Credentials::new(username, config.password.unwrap_or_default()));
            }
            Some(builder.build())
        });
        Mailer { transport, from }
    }

    pub async fn send(&self, mail: Mail) -> Result<(), String> {
        let Some(transport) = &self.transport else {
            println!(
                "[courriel non envoyé — SMTP absent]\nÀ: {}\nSujet: {}\n{}",
                mail.to, mail.subject, mail.text
            );
            return Ok(());
        };
        let from: Mailbox = self.from.parse().map_err(|e| format!("expéditeur invalide : {e}"))?;
        let to: Mailbox = mail.to.parse().map_err(|e| format!("destinataire invalide : {e}"))?;
        let message = Message::builder()
            .from(from)
            .to(to)
            .subject(mail.subject)
            .body(mail.text)
            .map_err(|e| e.to_string())?;
        transport.send(message).await.map(|_| ()).map_err(|e| e.to_string())
    }
}

pub fn verify_email_mail(to: &str, locale: Locale, link: &str) -> Mail {
    match locale {
        Locale::En => Mail {
            to: to.into(),
            subject: "Hebdomiam — confirm your email".into(),
            text: format!(
                "Hi!\n\nClick this link to confirm your email address:\n{link}\n\nThe link expires in 24 hours. If you didn't create an account, ignore this message."
            ),
        },
        Locale::Fr => Mail {
            to: to.into(),
            subject: "Hebdomiam — confirme ton courriel".into(),
            text: format!(
                "Bonjour !\n\nClique ce lien pour confirmer ton adresse courriel :\n{link}\n\nLe lien expire dans 24 heures. Si tu n'as pas créé de compte, ignore ce message."
            ),
        },
    }
}

pub fn invite_mail(to: &str, locale: Locale, link: &str) -> Mail {
    match locale {
        Locale::En => Mail {
            to: to.into(),
            subject: "Hebdomiam — you are invited".into(),
            text: format!(
                "Hi!\n\nAn administrator created an Hebdomiam account for you. Click this link to choose your password and activate it:\n{link}\n\nThe link expires in 7 days. If you weren't expecting this, ignore this message."
            ),
        },
        Locale::Fr => Mail {
            to: to.into(),
            subject: "Hebdomiam — tu es invité(e)".into(),
            text: format!(
                "Bonjour !\n\nUn administrateur t'a créé un compte Hebdomiam. Clique ce lien pour choisir ton mot de passe et l'activer :\n{link}\n\nLe lien expire dans 7 jours. Si tu n'attendais pas cette invitation, ignore ce message."
            ),
        },
    }
}

pub fn reset_password_mail(to: &str, locale: Locale, link: &str) -> Mail {
    match locale {
        Locale::En => Mail {
            to: to.into(),
            subject: "Hebdomiam — reset your password".into(),
            text: format!(
                "Hi!\n\nClick this link to choose a new password:\n{link}\n\nThe link expires in 1 hour. If you didn't ask for this, ignore this message."
            ),
        },
        Locale::Fr => Mail {
            to: to.into(),
            subject: "Hebdomiam — réinitialise ton mot de passe".into(),
            text: format!(
                "Bonjour !\n\nClique ce lien pour choisir un nouveau mot de passe :\n{link}\n\nLe lien expire dans 1 heure. Si tu n'es pas à l'origine de cette demande, ignore ce message."
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gabarits_bilingues_avec_lien() {
        let link = "https://recettes.example.com/x?token=abc";
        for mail in [
            verify_email_mail("a@b.c", Locale::Fr, link),
            verify_email_mail("a@b.c", Locale::En, link),
            invite_mail("a@b.c", Locale::Fr, link),
            invite_mail("a@b.c", Locale::En, link),
            reset_password_mail("a@b.c", Locale::Fr, link),
            reset_password_mail("a@b.c", Locale::En, link),
        ] {
            assert!(mail.text.contains(link));
            assert!(mail.subject.starts_with("Hebdomiam — "));
            assert_eq!(mail.to, "a@b.c");
        }
        assert!(reset_password_mail("a@b.c", Locale::En, link).text.contains("1 hour"));
        assert!(invite_mail("a@b.c", Locale::Fr, link).text.contains("7 jours"));
    }

    #[tokio::test]
    async fn sans_smtp_journalise_sans_erreur() {
        let mailer = Mailer::new(None, "hebdomiam@localhost".into());
        assert!(mailer.send(verify_email_mail("a@b.c", Locale::Fr, "l")).await.is_ok());
    }
}
