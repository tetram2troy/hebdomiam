export interface ProfileDto {
  name: string
  email: string
  emailVerifiedAt: string | null
  locale: 'fr' | 'en'
  unitSystem: 'metric' | 'imperial'
  defaultDishCount: number
  defaultPeopleCount: number
  defaultDaysCount: number
  rotationWeeks: number
  totpEnabled: boolean
}
