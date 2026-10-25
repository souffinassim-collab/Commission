// Dates de l'application : toujours affichées et saisies au format JJ/MM/AAAA.

/** Affiche une date (ISO « AAAA-MM-JJ[ HH:MM:SS] » ou déjà « JJ/MM/AAAA ») au format JJ/MM/AAAA[ HH:MM] */
export const formatDate = (s?: string | null): string => {
  if (!s) return '';
  const t = s.trim();
  const iso = t.match(/^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}))?/);
  if (iso) return `${iso[3]}/${iso[2]}/${iso[1]}${iso[4] ? ` ${iso[4]}:${iso[5]}` : ''}`;
  return t;
};

/** Contrôle d'une date saisie : null si valide (ou vide), sinon le message d'erreur */
export const erreurDate = (s: string): string | null => {
  const t = s.trim();
  if (!t) return null;
  const m = t.match(/^(\d{2})\/(\d{2})\/(\d{4})$/);
  if (!m) return 'Format attendu : JJ/MM/AAAA';
  const [j, mo, a] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (mo < 1 || mo > 12) return 'Mois invalide (01 à 12)';
  if (a < 1900 || a > 2100) return 'Année invalide';
  const jours = new Date(a, mo, 0).getDate();
  if (j < 1 || j > jours) return `Jour invalide (01 à ${jours} pour ce mois)`;
  return null;
};

/** Saisie guidée : ne garde que les chiffres et place les « / » (JJ/MM/AAAA) */
export const masquerDate = (s: string): string => {
  const c = s.replace(/\D/g, '').slice(0, 8);
  if (c.length <= 2) return c;
  if (c.length <= 4) return `${c.slice(0, 2)}/${c.slice(2)}`;
  return `${c.slice(0, 2)}/${c.slice(2, 4)}/${c.slice(4)}`;
};

/** Date du jour au format JJ/MM/AAAA */
export const aujourdhui = (): string => {
  const d = new Date();
  return `${String(d.getDate()).padStart(2, '0')}/${String(d.getMonth() + 1).padStart(2, '0')}/${d.getFullYear()}`;
};
