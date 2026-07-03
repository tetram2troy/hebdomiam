import type { Aisle, Unit } from '#shared/schemas/recipe'

// Parseur de lignes d'ingrédients (« 500 g de lentilles vertes ») — pur, testé.
// schema.org fournit les ingrédients en texte libre FR/EN ; on en tire
// quantité + unité + nom + rayon deviné. Ce qui ne se parse pas devient
// « 1 × nom » avec un avertissement (l'utilisateur relit le brouillon).

const UNIT_SYNONYMS: Array<[RegExp, Unit, number]> = [
  // [motif, unité canonique, facteur multiplicatif]
  [/^(?:cuill[eè]res? à soupe|c\.?\s*à\s*s(?:oupe)?\.?|càs|tbsp|tablespoons?)/i, 'tbsp', 1],
  [/^(?:cuill[eè]res? à (?:thé|café)|c\.?\s*à\s*(?:t(?:hé)?|c(?:afé)?)\.?|càc|càt|tsp|teaspoons?)/i, 'tsp', 1],
  [/^(?:tasses?|cups?)/i, 'cup', 1],
  [/^(?:kilogrammes?|kilos?|kg)/i, 'kg', 1],
  [/^(?:grammes?|gr?s?)\b/i, 'g', 1],
  [/^(?:millilitres?|ml)/i, 'ml', 1],
  [/^(?:centilitres?|cl)/i, 'ml', 10],
  [/^(?:litres?|liters?|l)\b/i, 'l', 1],
  [/^(?:onces?|oz|ounces?)/i, 'oz', 1],
  [/^(?:livres?|lbs?|pounds?)/i, 'lb', 1],
  [/^(?:pinc[ée]es?|pinch(?:es)?)/i, 'pinch', 1],
  [/^(?:unit[ée]s?)/i, 'unit', 1],
]

const AISLE_KEYWORDS: Array<[RegExp, Aisle]> = [
  // Les motifs spécifiques (conserves, surgelés) passent avant les génériques :
  // « tomates en dés » = conserve même si « tomate » est un mot de fruits et légumes.
  [/en conserve|en dés|bouillon|lait de coco|pâte de tomate|sauce tomate|pois chiches?|haricots? (?:noirs|rouges|blancs)/i, 'canned'],
  [/surgelé|congelé/i, 'frozen'],
  [/tomate|oignon|ail\b|échalote|carotte|céleri|poivron|courge|patate|pomme de terre|salade|laitue|épinard|brocoli|chou|champignon|citron|lime|gingembre|coriandre|persil|basilic frais|menthe|fruit|banane|pomme\b|avocat|concombre|zucchini|courgette|légume/i, 'produce'],
  [/poulet|bœuf|boeuf|porc|veau|agneau|dinde|jambon|bacon|saucisse|viande|steak|côtelette/i, 'butcher'],
  [/poisson|saumon|thon|crevette|morue|tilapia|fruits de mer|pétoncle/i, 'fish'],
  [/lait\b|crème|beurre|fromage|yogourt|yaourt|œufs?|oeufs?|parmesan|mozzarella|cheddar|feta/i, 'dairy'],
  [/farine|riz\b|pâtes|spaghetti|macaroni|lentille|quinoa|orge|couscous|sucre|avoine|céréale|noix|amande|arachide|chapelure/i, 'dry'],
  [/pain\b|baguette|tortilla|pita|croûton/i, 'bakery'],
  [/sel\b|poivre|épice|cumin|paprika|cannelle|origan|thym|laurier|curcuma|cari|curry|chili en poudre|muscade|vanille|herbes de provence/i, 'spices'],
  [/eau\b|jus\b|vin\b|bière|café\b|thé\b/i, 'beverages'],
]

export interface ParsedIngredientLine {
  quantity: number
  unit: Unit | null
  name: string
  aisle: Aisle
  /** false quand la quantité n'a pas pu être lue (défaut 1). */
  parsed: boolean
}

const VULGAR_FRACTIONS: Record<string, number> = {
  '½': 0.5, '⅓': 1 / 3, '⅔': 2 / 3, '¼': 0.25, '¾': 0.75, '⅛': 0.125,
}

/** Consomme une fraction (« ½ » ou « 1/2 ») en tête de chaîne. */
function readFraction(text: string): { value: number, rest: string } | null {
  const vulgar = text.match(/^([½⅓⅔¼¾⅛])\s*/)
  if (vulgar) {
    return { value: VULGAR_FRACTIONS[vulgar[1]!]!, rest: text.slice(vulgar[0].length) }
  }
  const written = text.match(/^(\d+)\s*\/\s*(\d+)\s*/)
  if (written && Number.parseInt(written[2]!) !== 0) {
    return {
      value: Number.parseInt(written[1]!) / Number.parseInt(written[2]!),
      rest: text.slice(written[0].length),
    }
  }
  return null
}

/** Lit une quantité en tête de chaîne : « 2 », « 1,5 », « 1/2 », « 1 ½ », « 2 à 3 ». */
function readQuantity(text: string): { value: number, rest: string } | null {
  let value = 0
  let rest = text.trim()
  let found = false

  // Fraction d'abord : sinon « 1/2 » serait lu comme l'entier 1.
  const leadingFraction = readFraction(rest)
  if (leadingFraction) {
    value = leadingFraction.value
    rest = leadingFraction.rest
    found = true
  }
  else {
    const integer = rest.match(/^(\d+(?:[.,]\d+)?)\s*/)
    if (integer) {
      value = Number.parseFloat(integer[1]!.replace(',', '.'))
      rest = rest.slice(integer[0].length)
      found = true
      // Nombre mixte : « 1 ½ » ou « 1 1/2 ».
      const mixed = readFraction(rest)
      if (mixed) {
        value += mixed.value
        rest = mixed.rest
      }
    }
  }

  if (!found) return null

  // Fourchette « 2 à 3 » / « 2-3 » : on prend la borne basse.
  const range = rest.match(/^(?:à|-|–|to)\s*\d+(?:[.,]\d+)?\s*/)
  if (range) rest = rest.slice(range[0].length)

  return value > 0 ? { value: Math.round(value * 100) / 100, rest } : null
}

export function guessAisle(name: string): Aisle {
  for (const [pattern, aisle] of AISLE_KEYWORDS) {
    if (pattern.test(name)) return aisle
  }
  return 'other'
}

export function parseIngredientLine(line: string): ParsedIngredientLine {
  const cleaned = line.replace(/\s+/g, ' ').trim()
  const quantity = readQuantity(cleaned)

  if (!quantity) {
    const name = cleaned || 'ingrédient'
    return { quantity: 1, unit: null, name, aisle: guessAisle(name), parsed: false }
  }

  let rest = quantity.rest
  let unit: Unit | null = null
  let value = quantity.value

  for (const [pattern, canonical, factor] of UNIT_SYNONYMS) {
    const match = rest.match(pattern)
    if (match) {
      unit = canonical
      value = Math.round(value * factor * 100) / 100
      rest = rest.slice(match[0].length)
      break
    }
  }

  // « 200 g de riz » / « 2 cups of rice » → retirer le connecteur.
  const name = rest.replace(/^\s*(?:de\s+|d'|of\s+)?/i, '').trim() || 'ingrédient'

  return { quantity: value, unit, name, aisle: guessAisle(name), parsed: true }
}
