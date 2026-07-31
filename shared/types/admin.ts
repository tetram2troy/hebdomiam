/** Vue admin d'un compte — liste blanche délibérée : jamais de données métier
    (recettes, plans…) ni de secrets, conformément à D2 « comptes privés ». */
export interface AdminUserDto {
  id: string
  email: string
  name: string
  role: 'admin' | 'user'
  emailVerified: boolean
  disabled: boolean
  createdAt: string
}
