// Charge la session avant la première navigation : les middlewares de route
// (auth, admin) lisent `loggedIn` et `user` dès le premier rendu.
export default defineNuxtPlugin(async () => {
  await useUserSession().fetch()
})
