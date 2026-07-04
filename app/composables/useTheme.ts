export type ThemeChoice = 'system' | 'light' | 'dark'

/** Bascule de thème (réglages) : persistée localement, appliquée via
    [data-theme] sur <html> — « system » retire l'attribut (prefers-color-scheme). */
export function useTheme() {
  const theme = useState<ThemeChoice>('hb-theme', () => 'system')

  if (import.meta.client) {
    const stored = localStorage.getItem('hb-theme')
    if (stored === 'light' || stored === 'dark') theme.value = stored
  }

  function apply(choice: ThemeChoice) {
    theme.value = choice
    if (!import.meta.client) return
    if (choice === 'system') {
      delete document.documentElement.dataset.theme
      localStorage.removeItem('hb-theme')
    }
    else {
      document.documentElement.dataset.theme = choice
      localStorage.setItem('hb-theme', choice)
    }
  }

  return { theme, apply }
}
