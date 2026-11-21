import type { Directive } from 'vue'

// v-autogrow : une zone de texte grandit avec son contenu. Un texte importé
// (description, étapes, nom d'ingrédient long) doit se relire d'un coup d'œil,
// pas en faisant défiler un petit cadre. `field-sizing: content` n'est pas
// encore disponible dans tous les navigateurs ciblés, d'où le calcul en JS.

interface GrowState {
  value: string
  width: number
  observer: ResizeObserver
}

const states = new WeakMap<HTMLTextAreaElement, GrowState>()

function fit(el: HTMLTextAreaElement) {
  el.style.height = 'auto'
  // scrollHeight exclut les bordures alors que box-sizing: border-box les compte.
  const borders = el.offsetHeight - el.clientHeight
  el.style.height = `${el.scrollHeight + borders}px`
}

function onInput(event: Event) {
  const el = event.target as HTMLTextAreaElement
  const state = states.get(el)
  if (state) state.value = el.value
  fit(el)
}

const autogrow: Directive<HTMLTextAreaElement> = {
  mounted(el) {
    el.style.overflowY = 'hidden'
    // Le retour à la ligne change avec la largeur (fenêtre redimensionnée,
    // colonne qui se replie) : on ne recalcule que sur un changement de
    // largeur, sinon notre propre changement de hauteur relancerait l'observateur.
    const observer = new ResizeObserver(() => {
      const state = states.get(el)
      if (state && el.clientWidth !== state.width) {
        state.width = el.clientWidth
        fit(el)
      }
    })
    states.set(el, { value: el.value, width: el.clientWidth, observer })
    observer.observe(el)
    el.addEventListener('input', onInput)
    fit(el)
    // Les polices web chargées après le montage changent la largeur des lignes.
    document.fonts?.ready.then(() => fit(el))
  },
  updated(el) {
    // Valeur changée par le code (étape déplacée, brouillon chargé) : `updated`
    // passe à chaque rendu du formulaire, d'où le test pour éviter un reflow
    // (et un saut de défilement) à chaque frappe dans un autre champ.
    const state = states.get(el)
    if (state && state.value !== el.value) {
      state.value = el.value
      fit(el)
    }
  },
  unmounted(el) {
    states.get(el)?.observer.disconnect()
    states.delete(el)
    el.removeEventListener('input', onInput)
  },
}

export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.vueApp.directive('autogrow', autogrow)
})
