declare module '#auth-utils' {
  interface User {
    id: string
    email: string
    name: string
    /** Copie du rôle pour le front (nav, middleware) — le serveur relit la base. */
    role: 'admin' | 'user'
  }

  interface UserSession {
    /** Mot de passe validé, code TOTP attendu — pas encore connecté. */
    pendingTotpUserId?: string
  }
}

export {}
