// Session côté client. Le cookie de session est httpOnly et scellé par l'API
// Rust : le front n'y lit rien, il interroge GET /api/session. Le rôle ne sert
// qu'à l'affichage (nav, middleware) — l'API le relit toujours en base.

export interface SessionUser {
  id: string
  email: string
  name: string
  role: 'admin' | 'user'
}

interface SessionState {
  user?: SessionUser
}

export function useUserSession() {
  const session = useState<SessionState>('hb-session', () => ({}))
  const user = computed(() => session.value.user ?? null)
  const loggedIn = computed(() => user.value !== null)

  /** Relit la session auprès de l'API (après connexion, au démarrage). */
  async function fetch() {
    try {
      session.value = await $fetch<SessionState>('/api/session')
    }
    catch {
      // 401 : compte désactivé ou supprimé — l'API a purgé le cookie.
      session.value = {}
    }
  }

  /** Oublie la session locale (le cookie est effacé par l'API : logout, suppression). */
  async function clear() {
    session.value = {}
  }

  return { session, user, loggedIn, fetch, clear }
}
