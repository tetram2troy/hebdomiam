// Garde d'affichage seulement : le rôle vient du cookie de session. Les routes
// /api/admin/* revérifient toujours le rôle en base (requireAdminSession).
export default defineNuxtRouteMiddleware(() => {
  const { user } = useUserSession()
  if (user.value?.role !== 'admin') {
    const localePath = useLocalePath()
    return navigateTo(localePath('/'))
  }
})
