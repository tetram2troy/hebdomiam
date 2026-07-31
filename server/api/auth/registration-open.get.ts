// Consommé par le front pour masquer le formulaire d'inscription quand
// l'instance a déjà un compte. La vraie défense est le 403 de register.post —
// cet endpoint ne divulgue que « instance vierge ou non », sans valeur offensive.
export default defineEventHandler(async () => {
  return { open: await countUsers(useDb()) === 0 }
})
