import type { Aisle, Unit } from '../schemas/recipe'

/** Brouillon d'extraction (URL ou photo) — jamais écrit en base directement :
    il pré-remplit le formulaire de recette que l'utilisateur relit (PRD F3/F4). */
export interface ImportDraft {
  title: string
  description: string | null
  imageUrl: string | null
  baseServings: number
  prepMinutes: number | null
  cookMinutes: number | null
  freezable: boolean
  ingredients: Array<{
    quantity: number
    unit: Unit | null
    name: string
    aisle: Aisle
  }>
  steps: string[]
  tags: string[]
  sourceUrl: string | null
  /** Points à vérifier, montrés dans le bandeau de relecture. */
  warnings: string[]
  /** Fournisseur qui a produit le brouillon. */
  provider: 'schema-org' | 'claude'
}
