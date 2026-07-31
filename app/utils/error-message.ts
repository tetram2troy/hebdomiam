// Convertit une erreur $fetch en message traduit. Le serveur renvoie des clés
// i18n (ex. « errors.emailTaken ») dans le champ message.
const KNOWN_KEYS = new Set([
  'errors.invalidCredentials',
  'errors.emailTaken',
  'errors.tooManyRequests',
  'errors.notFound',
  'errors.planExists',
  'errors.notEnoughRecipes',
  'errors.noMoreSuggestions',
  'errors.invalidState',
  'errors.importUnreachable',
  'errors.importFailed',
  'errors.importUnavailable',
  'errors.importProviderError',
  'errors.fileTooLarge',
  'errors.invalidImage',
  'errors.invalidPassword',
  'errors.invalidCode',
  'errors.invalidToken',
  'errors.forbidden',
  'errors.registrationClosed',
  'errors.accountDisabled',
  'errors.lastAdmin',
  'errors.adminSelf',
  'errors.inviteAlreadyActive',
])

export function errorMessage(err: unknown, t: (key: string) => string): string {
  const fetchError = err as { statusCode?: number, data?: { message?: string } }
  const key = fetchError.data?.message
  if (key && KNOWN_KEYS.has(key)) {
    return t(key)
  }
  if (fetchError.statusCode === 400) {
    return t('errors.invalidInput')
  }
  return t('errors.unknown')
}
