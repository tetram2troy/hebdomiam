/** Forme de comparaison tolérante : casse et accents ignorés, pour que
    « vege » trouve « végé » sur un clavier de téléphone. */
export function normalizeForSearch(text: string): string {
  return text.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase().trim()
}

/** Tags dont le nom contient la recherche ; l'ordre d'entrée (du plus utilisé
    au moins utilisé, tel que renvoyé par l'API) est conservé. */
export function filterTags<T extends { name: string }>(tags: T[], query: string): T[] {
  const needle = normalizeForSearch(query)
  if (!needle) return tags
  return tags.filter(tag => normalizeForSearch(tag.name).includes(needle))
}
