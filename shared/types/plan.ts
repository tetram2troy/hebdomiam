import type { RecipeSummary } from './recipe'

export type MealPlanStatus = 'draft' | 'validated' | 'cooked'
export type MealPlanRecipeStatus = 'suggested' | 'accepted' | 'swapped' | 'cooked'

export interface PlanRecipeDto {
  id: string
  position: number
  status: Exclude<MealPlanRecipeStatus, 'swapped'>
  targetServings: number
  cookedAt: string | null
  recipe: RecipeSummary
}

export interface MealPlanDto {
  id: string
  weekStartDate: string
  peopleCount: number
  dishCount: number
  daysCount: number
  totalServings: number
  status: MealPlanStatus
  /** Rangées actives seulement (les « swapped » restent en base comme mémoire). */
  recipes: PlanRecipeDto[]
}

export interface PlanDefaults {
  dishCount: number
  peopleCount: number
  daysCount: number
  rotationWeeks: number
  /** Nombre de recettes actives — le client peut prévenir avant de proposer. */
  recipeCount: number
}

export interface ReplacementOption extends RecipeSummary {
  lastCookedAt: string | null
  cookedCount: number
}
