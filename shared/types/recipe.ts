import type { Aisle, Unit } from '../schemas/recipe'

// Formes renvoyées par l'API recettes (couche server/utils/recipe-repo.ts).

export interface RecipeSummary {
  id: string
  title: string
  imageUrl: string | null
  baseServings: number
  prepMinutes: number | null
  cookMinutes: number | null
  freezable: boolean
  tags: string[]
}

export interface RecipeIngredientDto {
  id: string
  position: number
  quantity: number
  unit: Unit | null
  name: string
  aisle: Aisle
}

export interface RecipeStepDto {
  id: string
  position: number
  text: string
}

export interface RecipeDetail extends RecipeSummary {
  description: string | null
  sourceUrl: string | null
  notes: string | null
  ingredients: RecipeIngredientDto[]
  steps: RecipeStepDto[]
  createdAt: string
  updatedAt: string
}

export interface TagSummary {
  name: string
  recipeCount: number
}
