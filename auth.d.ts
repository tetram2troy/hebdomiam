declare module '#auth-utils' {
  interface User {
    id: string
    email: string
    name: string
  }

  interface UserSession {
    /** Mot de passe validé, code TOTP attendu — pas encore connecté. */
    pendingTotpUserId?: string
  }
}

export {}
