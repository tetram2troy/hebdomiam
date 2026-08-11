import { describe, expect, it } from 'vitest'
import { htmlToText } from '../../server/utils/import/claude-provider'

describe('htmlToText', () => {
  it('supprime scripts, styles et blocs de navigation, garde le contenu', () => {
    const html = `<html><head><style>.x{color:red}</style></head><body>
      <nav class="menu"><a href="/">Accueil</a></nav>
      <h1>Chili végé</h1><p>Un classique réconfortant.</p>
      <script>console.log('pub')</script>
      <footer>© Site</footer></body></html>`
    const text = htmlToText(html)
    expect(text).toContain('Chili végé')
    expect(text).toContain('classique réconfortant')
    expect(text).not.toContain('console.log')
    expect(text).not.toContain('color:red')
    expect(text).not.toContain('Accueil')
  })

  it('ne se laisse pas contourner par des balises fermantes farfelues (js/bad-tag-filter)', () => {
    // « </script > » et « </script foo> » sont des fermetures valides pour les
    // navigateurs : le filtre doit les reconnaître aussi.
    const html = `<p>Avant</p><script>malicieux()</script ><p>Milieu</p>
      <script type="text/javascript">autre()</script foo="bar"><p>Après</p>`
    const text = htmlToText(html)
    expect(text).not.toContain('malicieux')
    expect(text).not.toContain('autre()')
    expect(text).toContain('Avant')
    expect(text).toContain('Milieu')
    expect(text).toContain('Après')
  })

  it('convertit les fins de bloc en sauts de ligne et compacte les vides', () => {
    const text = htmlToText('<p>Un</p><p>Deux</p><br>Trois')
    expect(text.split('\n').map(line => line.trim())).toEqual(['Un', 'Deux', 'Trois'])
  })
})
