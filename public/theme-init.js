// Applique le thème choisi avant la peinture (évite l'éclair clair→sombre).
// Fichier servi plutôt que script inline : la politique de sécurité du
// contenu (CSP) n'autorise aucun script inline écrit à la main.
(function () {
  try {
    var t = localStorage.getItem('hb-theme')
    if (t === 'light' || t === 'dark') document.documentElement.dataset.theme = t
  }
  catch (e) { /* stockage local indisponible : thème du système */ }
})()
