import type { Aisle, Unit } from '../schemas/recipe'

export interface GroceryItemSourceDto {
  recipeTitle: string
  quantity: number
  unit: Unit | null
}

export interface GroceryItemDto {
  id: string
  label: string
  /** Quantité déjà convertie dans le système d'unités de l'utilisateur. */
  quantity: number | null
  unit: Unit | null
  aisle: Aisle
  checked: boolean
  isManual: boolean
  sources: GroceryItemSourceDto[]
}

export interface GroceryListDto {
  id: string
  planId: string
  weekStartDate: string
  checkedCount: number
  totalCount: number
  /** Triés par ordre de rayon (ordre du magasin) puis par libellé. */
  items: GroceryItemDto[]
}
