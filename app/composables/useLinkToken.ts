// Jeton d'un lien reçu par courriel (invitation, réinitialisation,
// vérification). Il voyage dans le FRAGMENT de l'URL (`#token=…`) : le
// navigateur n'envoie jamais le fragment au serveur, donc le jeton n'apparaît
// ni dans les journaux d'accès (Cloudflare, Traefik) ni dans un en-tête
// Referer. Une fois lu, il est retiré de la barre d'adresse et de l'historique.
export function useLinkToken(): string {
  const route = useRoute()
  const token = new URLSearchParams(route.hash.replace(/^#/, '')).get('token') ?? ''

  onMounted(() => {
    if (route.hash) {
      history.replaceState(history.state, '', route.path)
    }
  })

  return token
}
