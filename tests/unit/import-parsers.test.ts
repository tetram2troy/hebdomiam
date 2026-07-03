import { describe, expect, it } from 'vitest'
import { guessAisle, parseIngredientLine } from '../../server/utils/import/ingredient-line'
import { extractRecipeFromHtml, parseIsoDuration } from '../../server/utils/import/schema-org'

describe('parseIngredientLine', () => {
  it('lit « quantité unité de nom » en français', () => {
    expect(parseIngredientLine('500 g de lentilles vertes')).toMatchObject({
      quantity: 500, unit: 'g', name: 'lentilles vertes', aisle: 'dry', parsed: true,
    })
    expect(parseIngredientLine('2 tasses de riz basmati')).toMatchObject({
      quantity: 2, unit: 'cup', name: 'riz basmati',
    })
    expect(parseIngredientLine('1 c. à soupe d\'huile d\'olive')).toMatchObject({
      quantity: 1, unit: 'tbsp', name: 'huile d\'olive',
    })
    expect(parseIngredientLine('2 c. à thé de cumin moulu')).toMatchObject({
      quantity: 2, unit: 'tsp', name: 'cumin moulu', aisle: 'spices',
    })
  })

  it('lit les formats anglais', () => {
    expect(parseIngredientLine('2 cups of chicken broth')).toMatchObject({
      quantity: 2, unit: 'cup', name: 'chicken broth',
    })
    expect(parseIngredientLine('1 lb ground beef')).toMatchObject({
      quantity: 1, unit: 'lb',
    })
    expect(parseIngredientLine('3 tbsp olive oil')).toMatchObject({
      quantity: 3, unit: 'tbsp', name: 'olive oil',
    })
  })

  it('gère les fractions (½, 1/2, 1 ½) et les décimales à virgule', () => {
    expect(parseIngredientLine('½ tasse de lait').quantity).toBe(0.5)
    expect(parseIngredientLine('1/2 c. à thé de sel').quantity).toBe(0.5)
    expect(parseIngredientLine('1 ½ tasse de farine').quantity).toBe(1.5)
    expect(parseIngredientLine('1,5 kg de poulet')).toMatchObject({ quantity: 1.5, unit: 'kg' })
  })

  it('convertit les centilitres en millilitres', () => {
    expect(parseIngredientLine('25 cl de crème')).toMatchObject({ quantity: 250, unit: 'ml' })
  })

  it('prend la borne basse d\'une fourchette « 2 à 3 »', () => {
    expect(parseIngredientLine('2 à 3 oignons jaunes')).toMatchObject({
      quantity: 2, unit: null, name: 'oignons jaunes', aisle: 'produce',
    })
  })

  it('sans quantité : défaut 1 et parsed=false (à relire)', () => {
    const result = parseIngredientLine('Sel et poivre au goût')
    expect(result).toMatchObject({ quantity: 1, unit: null, parsed: false })
  })

  it('devine le rayon par mots-clés', () => {
    expect(guessAisle('hauts de cuisse de poulet')).toBe('butcher')
    expect(guessAisle('filet de saumon')).toBe('fish')
    expect(guessAisle('crème 35 %')).toBe('dairy')
    expect(guessAisle('tomates en dés')).toBe('canned') // « en dés » = conserve
    expect(guessAisle('paprika fumé')).toBe('spices')
    expect(guessAisle('truc inconnu')).toBe('other')
  })
})

describe('parseIsoDuration', () => {
  it('convertit les durées ISO 8601 en minutes', () => {
    expect(parseIsoDuration('PT30M')).toBe(30)
    expect(parseIsoDuration('PT1H30M')).toBe(90)
    expect(parseIsoDuration('PT2H')).toBe(120)
    expect(parseIsoDuration('P0DT1H15M')).toBe(75)
  })

  it('rejette l\'illisible', () => {
    expect(parseIsoDuration('bientôt')).toBeNull()
    expect(parseIsoDuration(null)).toBeNull()
    expect(parseIsoDuration('PT')).toBeNull()
  })
})

describe('extractRecipeFromHtml (JSON-LD)', () => {
  const SIMPLE = `<html><head>
    <script type="application/ld+json">
    {
      "@context": "https://schema.org",
      "@type": "Recipe",
      "name": "Chili v&eacute;g&#233;tarien",
      "description": "Un classique réconfortant.",
      "image": ["https://exemple.com/chili.jpg"],
      "recipeYield": "6 portions",
      "prepTime": "PT15M",
      "cookTime": "PT45M",
      "recipeIngredient": ["500 g de lentilles vertes", "2 oignons jaunes", "Sel au goût"],
      "recipeInstructions": [
        {"@type": "HowToStep", "text": "Hacher les oignons."},
        {"@type": "HowToStep", "text": "Tout mijoter 45 minutes."}
      ],
      "keywords": "végé, mijoteuse",
      "recipeCuisine": "Tex-Mex"
    }
    </script></head><body></body></html>`

  it('extrait un brouillon complet d\'un JSON-LD simple', () => {
    const draft = extractRecipeFromHtml(SIMPLE, 'https://exemple.com/chili')!
    expect(draft).not.toBeNull()
    expect(draft.title).toBe('Chili végétarien')
    expect(draft.baseServings).toBe(6)
    expect(draft.prepMinutes).toBe(15)
    expect(draft.cookMinutes).toBe(45)
    expect(draft.imageUrl).toBe('https://exemple.com/chili.jpg')
    expect(draft.ingredients).toHaveLength(3)
    expect(draft.ingredients[0]).toMatchObject({ quantity: 500, unit: 'g', name: 'lentilles vertes' })
    expect(draft.steps).toEqual(['Hacher les oignons.', 'Tout mijoter 45 minutes.'])
    expect(draft.tags).toEqual(['végé', 'mijoteuse', 'tex-mex'])
    expect(draft.sourceUrl).toBe('https://exemple.com/chili')
    expect(draft.provider).toBe('schema-org')
    // « Sel au goût » n'a pas de quantité → avertissement de relecture
    expect(draft.warnings.some(w => w.includes('Sel au goût'))).toBe(true)
  })

  it('trouve la recette dans un @graph et des instructions en sections', () => {
    const html = `<script type="application/ld+json">
    {"@context":"https://schema.org","@graph":[
      {"@type":"WebPage","name":"Page"},
      {"@type":["Recipe","Thing"],"name":"Soupe orge",
       "recipeYield": 4,
       "totalTime": "PT1H",
       "prepTime": "PT20M",
       "recipeIngredient": ["250 g d'orge mondé"],
       "recipeInstructions": [{"@type":"HowToSection","itemListElement":[
         {"@type":"HowToStep","text":"Rincer l'orge."},
         {"@type":"HowToStep","text":"Cuire 40 minutes."}
       ]}]}
    ]}
    </script>`
    const draft = extractRecipeFromHtml(html, 'https://exemple.com/soupe')!
    expect(draft.title).toBe('Soupe orge')
    expect(draft.steps).toHaveLength(2)
    // cookTime absent → déduit de totalTime - prepTime
    expect(draft.cookMinutes).toBe(40)
  })

  it('retourne null sans JSON-LD Recipe ou sans ingrédients/étapes', () => {
    expect(extractRecipeFromHtml('<html><body>Rien ici</body></html>', 'https://x.test')).toBeNull()
    const incomplete = `<script type="application/ld+json">
      {"@type":"Recipe","name":"Vide","recipeIngredient":[],"recipeInstructions":[]}
    </script>`
    expect(extractRecipeFromHtml(incomplete, 'https://x.test')).toBeNull()
  })

  it('survit à un premier bloc JSON-LD invalide', () => {
    const html = `<script type="application/ld+json">{pas du json</script>${SIMPLE}`
    expect(extractRecipeFromHtml(html, 'https://exemple.com/chili')).not.toBeNull()
  })
})
