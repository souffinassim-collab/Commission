// Saisie au clavier dans les fiches : Tabulation (natif) ou Entrée pour passer au champ suivant.

const CHAMPS =
  'input:not([type=hidden]):not([type=checkbox]):not([type=radio]):not([disabled]):not([readonly]), select:not([disabled]), textarea:not([disabled]), button[type=submit]:not([disabled])';

const visible = (el: HTMLElement) => el.offsetParent !== null && el.tabIndex !== -1;

/**
 * Entrée dans un champ de saisie : passe au champ suivant de la fiche (formulaire ou fenêtre ouverte).
 * Sur le dernier champ, Entrée garde son comportement normal (validation du formulaire).
 * Un champ peut garder son propre usage d'Entrée (recherche, ajout rapide) en appelant preventDefault()
 * dans son gestionnaire, ou avec l'attribut data-entree-native.
 */
export const champSuivantSurEntree = (e: KeyboardEvent) => {
  if (e.key !== 'Enter' || e.defaultPrevented || e.shiftKey || e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return;
  const cible = e.target as HTMLElement | null;
  if (!cible || !(cible instanceof HTMLInputElement || cible instanceof HTMLSelectElement)) return;
  if (cible.closest('[data-entree-native]')) return;
  if (cible instanceof HTMLInputElement && ['checkbox', 'radio', 'file', 'button', 'submit'].includes(cible.type)) return;

  // Fiche courante : le formulaire, sinon la fenêtre (modale) ou la page
  const fiche = (cible.closest('form') || cible.closest('.fixed') || document.body) as HTMLElement;
  const champs = Array.from(fiche.querySelectorAll<HTMLElement>(CHAMPS)).filter(visible);
  const i = champs.indexOf(cible);
  const suivant = i >= 0 ? champs[i + 1] : undefined;
  if (!suivant) return; // dernier champ : comportement normal (ex: validation du formulaire)
  e.preventDefault();
  suivant.focus();
  if (suivant instanceof HTMLInputElement && suivant.type !== 'number') suivant.select();
};
