import React from 'react';
import { AlertTriangle, AlertCircle, Info, X, Search, CheckCircle2, Ban } from 'lucide-react';
import { Anomalie, BordereauLigne, StatutBordereau } from '../types';
import { erreurDate, masquerDate, aujourdhui } from '../lib/dates';

const round2 = (v: number) => Math.round(v * 100) / 100;

/**
 * Recalcule les montants de chaque ligne à partir de ses propres taux puis les totaux (lignes exclues ignorées).
 * Même formule que CalculationEngine::recompute_bordereau côté Rust. Les taux ne sont jamais modifiés.
 */
export function recomputeTotals<T extends { tva_taux: number; lignes: BordereauLigne[] }>(b: T, lignes: BordereauLigne[]): T {
  const recalc = lignes.map((l) => {
    // Prime de la ligne = somme des polices (contributions) non exclues
    const contribs = l.contributions || [];
    const prime = round2(contribs.length ? contribs.filter((c) => !c.exclue).reduce((s, c) => s + c.montant, 0) : l.prime_nette);
    const apport = round2(prime * l.taux_apport);
    const gestion = round2(prime * l.taux_gestion);
    const ht = round2(apport + gestion);
    const tva = round2(ht * b.tva_taux);
    return { ...l, prime_nette: prime, montant_apport: apport, montant_gestion: gestion, montant_ht: ht, montant_tva: tva, montant_ttc: round2(ht + tva) };
  });
  const active = recalc.filter((l) => !l.desactive);
  const totalApport = round2(active.reduce((s, l) => s + l.montant_apport, 0));
  const totalGestion = round2(active.reduce((s, l) => s + l.montant_gestion, 0));
  const totalHt = round2(totalApport + totalGestion);
  const totalTva = round2(totalHt * b.tva_taux);
  return {
    ...b,
    lignes: recalc,
    total_prime_nette: round2(active.reduce((s, l) => s + l.prime_nette, 0)),
    total_commission_apport: totalApport,
    total_commission_gestion: totalGestion,
    total_commission_ht: totalHt,
    total_tva: totalTva,
    total_commission_ttc: round2(totalHt + totalTva),
  };
}

/** Ligne prise en compte : non désactivée et au moins une police non exclue */
export const ligneActive = (l: BordereauLigne) =>
  !l.desactive && (!l.contributions?.length || l.contributions.some((c) => !c.exclue));

/** Anomalies bloquantes : une anomalie rattachée à une ligne ne bloque plus si la ligne est exclue */
export function anomaliesBloquantes(anomalies: Anomalie[] | undefined, lignes: BordereauLigne[]): Anomalie[] {
  return (anomalies || []).filter(
    (a) => a.niveau === 'ERREUR' && (!a.ligne || lignes.some((l) => l.code_garantie === a.ligne && ligneActive(l)))
  );
}

/** Clé d'une police : fichier source + numéro de police */
export const policeKey = (fichier: string, police: string) => `${fichier}|${police}`;

/** Applique l'ensemble des polices exclues aux contributions des lignes (sans recalcul) */
export const appliquerPolicesExclues = (lignes: BordereauLigne[], exclues: Set<string>): BordereauLigne[] =>
  lignes.map((l) => ({
    ...l,
    contributions: (l.contributions || []).map((c) => ({
      ...c,
      // Une police déjà facturée ailleurs reste exclue, quoi qu'il arrive (double facturation interdite)
      exclue: !!c.deja_facturee || (!!c.police && exclues.has(policeKey(c.fichier_source, c.police))),
    })),
  }));

/** Police présente dans les lignes d'un bordereau (une police peut couvrir plusieurs garanties) */
export interface PoliceResume {
  key: string;
  police: string;
  fichier: string;
  client?: string;
  clientCode?: string;
  montant: number;
  nbGaranties: number;
  exclue: boolean;
  /** Déjà facturée dans un autre bordereau (origine) : verrouillée */
  dejaFacturee?: string;
}

/** Polices déjà facturées dans un autre bordereau, avec l'origine de la première facturation */
export const policesDejaFacturees = (lignes: BordereauLigne[]): PoliceResume[] =>
  listerPolices(lignes).filter((p) => !!p.dejaFacturee);

/** Alerte « Double facturation » : polices déjà facturées, verrouillées et non comptées */
export const AlerteDoubleFacturation: React.FC<{ lignes: BordereauLigne[] }> = ({ lignes }) => {
  const polices = policesDejaFacturees(lignes);
  if (!polices.length) return null;
  const parOrigine = new Map<string, PoliceResume[]>();
  for (const p of polices) parOrigine.set(p.dejaFacturee!, [...(parOrigine.get(p.dejaFacturee!) || []), p]);
  const total = round2(polices.reduce((s, p) => s + p.montant, 0));
  const fmt = (v: number) => v.toLocaleString('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return (
    <div className="bg-rose-50 border-2 border-rose-400 rounded-2xl p-4 space-y-2 text-xs text-rose-950 no-print">
      <div className="flex items-center gap-2 font-black text-sm">
        <Ban className="w-5 h-5 text-rose-600" /> Double facturation bloquée : {polices.length} police(s) déjà facturée(s)
      </div>
      <p>
        Ces polices ont déjà été facturées dans un autre bordereau. Elles sont <strong>verrouillées et non comptées</strong> ({fmt(total)} DZD de
        prime) : elles ne peuvent pas être refacturées.
      </p>
      {[...parOrigine.entries()].map(([origine, ps]) => (
        <div key={origine} className="bg-white/70 border border-rose-200 rounded-lg p-2">
          <div className="font-bold">Déjà facturée(s) — {origine}</div>
          <div className="font-mono text-[11px] text-rose-800 break-words">
            {ps.slice(0, 30).map((p) => p.police).join(' • ')}
            {ps.length > 30 && ` … (+${ps.length - 30})`}
          </div>
        </div>
      ))}
    </div>
  );
};

export const listerPolices = (lignes: BordereauLigne[]): PoliceResume[] => {
  const map = new Map<string, PoliceResume>();
  for (const l of lignes) {
    for (const c of l.contributions || []) {
      if (!c.police) continue;
      const key = policeKey(c.fichier_source, c.police);
      const p = map.get(key) || {
        key,
        police: c.police,
        fichier: c.fichier_source,
        client: c.client,
        clientCode: c.client_code,
        montant: 0,
        nbGaranties: 0,
        exclue: true,
        dejaFacturee: c.deja_facturee,
      };
      p.montant = round2(p.montant + c.montant);
      p.nbGaranties += 1;
      p.exclue = p.exclue && !!c.exclue;
      p.dejaFacturee = p.dejaFacturee || c.deja_facturee;
      map.set(key, p);
    }
  }
  return [...map.values()].sort((a, b) => a.police.localeCompare(b.police));
};

/**
 * Liste des polices importées : le commercial exclut les polices dont la prime n'est pas encaissée.
 * Une police exclue n'est pas commissionnée (sa prime est retirée des lignes et des totaux).
 */
export const PolicesPanel: React.FC<{
  lignes: BordereauLigne[];
  readOnly?: boolean;
  onToggle?: (keys: string[], exclue: boolean) => void;
}> = ({ lignes, readOnly, onToggle }) => {
  const [q, setQ] = React.useState('');
  const [filtre, setFiltre] = React.useState<'TOUTES' | 'EXCLUES'>('TOUTES');
  const polices = listerPolices(lignes);
  if (!polices.length) return null;
  // Les polices déjà facturées ailleurs sont verrouillées : elles ne comptent pas parmi les « non encaissées »
  const exclues = polices.filter((p) => p.exclue && !p.dejaFacturee);
  const verrouillees = polices.filter((p) => p.dejaFacturee);
  const montantExclu = round2(exclues.reduce((s, p) => s + p.montant, 0));
  const ql = q.toLowerCase();
  const visibles = polices.filter(
    (p) => (filtre === 'TOUTES' || p.exclue) && (!ql || p.police.toLowerCase().includes(ql) || (p.client || '').toLowerCase().includes(ql) || (p.clientCode || '').toLowerCase().includes(ql))
  );
  const fmt = (v: number) => v.toLocaleString('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return (
    <details className="bg-white rounded-2xl border border-slate-200 shadow-xs p-4 no-print" open>
      <summary className="cursor-pointer text-xs font-bold text-slate-800 flex flex-wrap items-center gap-2">
        Polices importées — vérification de l'encaissement ({polices.length} police{polices.length > 1 ? 's' : ''})
        {exclues.length > 0 ? (
          <span className="px-2 py-0.5 rounded bg-rose-100 text-rose-800 border border-rose-200 text-[10px]">
            {exclues.length} non encaissée(s) exclue(s) — {fmt(montantExclu)} DZD non commissionnés
          </span>
        ) : (
          <span className="px-2 py-0.5 rounded bg-emerald-100 text-emerald-800 border border-emerald-200 text-[10px]">
            toutes commissionnées
          </span>
        )}
        {verrouillees.length > 0 && (
          <span className="px-2 py-0.5 rounded bg-rose-600 text-white text-[10px]">
            {verrouillees.length} déjà facturée(s) — verrouillée(s)
          </span>
        )}
      </summary>
      <div className="mt-3 space-y-2 text-xs">
        {!readOnly && (
          <p className="text-[11px] text-slate-500">
            Vérifiez l'encaissement de chaque police : une police non encaissée doit être exclue, sa prime ne sera pas commissionnée.
          </p>
        )}
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-2.5" />
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="N° de police ou client…"
              className="pl-8 pr-3 py-1.5 border border-slate-200 rounded-lg w-60"
            />
          </div>
          <select
            value={filtre}
            onChange={(e) => setFiltre(e.target.value as 'TOUTES' | 'EXCLUES')}
            className="border border-slate-200 rounded-lg px-2 py-1.5"
          >
            <option value="TOUTES">Toutes les polices</option>
            <option value="EXCLUES">Exclues seulement</option>
          </select>
          {!readOnly && exclues.length > 0 && (
            <button
              type="button"
              onClick={() => onToggle?.(exclues.map((p) => p.key), false)}
              className="px-2.5 py-1.5 rounded-lg bg-slate-100 hover:bg-slate-200 font-bold cursor-pointer"
            >
              Tout réintégrer
            </button>
          )}
        </div>
        <div className="max-h-80 overflow-y-auto divide-y divide-slate-100 border border-slate-100 rounded-xl">
          {visibles.map((p) => (
            <div key={p.key} className={`flex items-center justify-between gap-3 px-3 py-2 ${p.exclue ? 'bg-rose-50/60' : ''}`}>
              <div className="min-w-0">
                <div className={`font-mono font-bold ${p.exclue ? 'line-through text-slate-400' : 'text-indigo-800'}`}>{p.police}</div>
                <div className="text-[10px] text-slate-500 truncate">
                  {(p.clientCode || p.client) && `${[p.clientCode, p.client].filter(Boolean).join(' ')} • `}
                  {p.fichier} • {p.nbGaranties} garantie(s)
                </div>
                {p.dejaFacturee && <div className="text-[10px] font-bold text-rose-700">Déjà facturée — {p.dejaFacturee}</div>}
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <span className={`font-mono font-bold ${p.exclue ? 'line-through text-slate-400' : ''}`}>{fmt(p.montant)}</span>
                {p.dejaFacturee ? (
                  <span
                    className="px-2 py-1 rounded-lg text-[11px] font-bold flex items-center gap-1 bg-rose-100 text-rose-800 border border-rose-300 cursor-not-allowed"
                    title={`Déjà facturée — ${p.dejaFacturee} : refacturation interdite`}
                  >
                    <Ban className="w-3 h-3" /> Déjà facturée
                  </span>
                ) : readOnly ? (
                  <span className={`text-[10px] font-bold ${p.exclue ? 'text-rose-700' : 'text-emerald-700'}`}>
                    {p.exclue ? 'Non encaissée (exclue)' : 'Commissionnée'}
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => onToggle?.([p.key], !p.exclue)}
                    className={`px-2 py-1 rounded-lg text-[11px] font-bold flex items-center gap-1 cursor-pointer ${
                      p.exclue
                        ? 'bg-rose-600 text-white hover:bg-rose-700'
                        : 'bg-emerald-50 text-emerald-800 border border-emerald-200 hover:bg-emerald-100'
                    }`}
                    title={p.exclue ? 'Réintégrer (prime encaissée)' : 'Exclure (prime non encaissée)'}
                  >
                    {p.exclue ? <Ban className="w-3 h-3" /> : <CheckCircle2 className="w-3 h-3" />}
                    {p.exclue ? 'Non encaissée' : 'Encaissée'}
                  </button>
                )}
              </div>
            </div>
          ))}
          {visibles.length === 0 && <div className="py-6 text-center text-slate-400">Aucune police.</div>}
        </div>
      </div>
    </details>
  );
};


const STYLES: Record<Anomalie['niveau'], { box: string; icon: React.ReactNode; label: string }> = {
  ERREUR: {
    box: 'bg-rose-50 border-rose-200 text-rose-900',
    icon: <AlertCircle className="w-3.5 h-3.5 text-rose-600 shrink-0 mt-0.5" />,
    label: 'Bloquant',
  },
  AVERTISSEMENT: {
    box: 'bg-amber-50 border-amber-200 text-amber-900',
    icon: <AlertTriangle className="w-3.5 h-3.5 text-amber-600 shrink-0 mt-0.5" />,
    label: 'À vérifier',
  },
  INFO: {
    box: 'bg-sky-50 border-sky-200 text-sky-900',
    icon: <Info className="w-3.5 h-3.5 text-sky-600 shrink-0 mt-0.5" />,
    label: 'Information',
  },
};

export const AnomaliesPanel: React.FC<{ anomalies: Anomalie[]; title?: string; lignes?: BordereauLigne[] }> = ({
  anomalies,
  title,
  lignes,
}) => {
  if (!anomalies.length) return null;
  const order: Anomalie['niveau'][] = ['ERREUR', 'AVERTISSEMENT', 'INFO'];
  const sorted = [...anomalies].sort((a, b) => order.indexOf(a.niveau) - order.indexOf(b.niveau));
  return (
    <div className="space-y-1.5">
      {title && <div className="text-[11px] font-black uppercase tracking-wider text-slate-600">{title}</div>}
      {sorted.map((a, i) => {
        const neutralisee = a.niveau === 'ERREUR' && !!a.ligne && !!lignes && !lignes.some((l) => l.code_garantie === a.ligne && !l.desactive);
        const st = STYLES[a.niveau] || STYLES.INFO;
        return (
          <div key={i} className={`flex items-start gap-2 p-2 rounded-lg border text-[11px] ${neutralisee ? 'bg-slate-50 border-slate-200 text-slate-500 line-through' : st.box}`}>
            {st.icon}
            <span className="font-bold shrink-0">{neutralisee ? 'Ligne exclue' : st.label} :</span>
            <span>{a.message}</span>
          </div>
        );
      })}
    </div>
  );
};

export const AnomaliesBadges: React.FC<{ anomalies: Anomalie[] }> = ({ anomalies }) => {
  const count = (n: Anomalie['niveau']) => anomalies.filter((a) => a.niveau === n).length;
  const e = count('ERREUR');
  const w = count('AVERTISSEMENT');
  if (!e && !w) {
    return <span className="px-1.5 py-0.5 rounded text-[10px] font-bold bg-emerald-100 text-emerald-800 border border-emerald-200">Contrôles OK</span>;
  }
  return (
    <span className="inline-flex gap-1">
      {e > 0 && <span className="px-1.5 py-0.5 rounded text-[10px] font-bold bg-rose-100 text-rose-800 border border-rose-200">{e} bloquante{e > 1 ? 's' : ''}</span>}
      {w > 0 && <span className="px-1.5 py-0.5 rounded text-[10px] font-bold bg-amber-100 text-amber-800 border border-amber-200">{w} à vérifier</span>}
    </span>
  );
};

export interface ConfirmState {
  title: string;
  message: React.ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel?: () => void;
}

export const ConfirmDialog: React.FC<{ state: ConfirmState | null; onClose: () => void }> = ({ state, onClose }) => {
  if (!state) return null;
  const cancel = () => {
    state.onCancel?.();
    onClose();
  };
  return (
    <div className="fixed inset-0 z-[60] bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4">
      <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-md overflow-hidden">
        <div className={`px-5 py-3.5 flex items-center justify-between text-white ${state.danger ? 'bg-rose-700' : 'bg-indigo-800'}`}>
          <div className="flex items-center gap-2 font-bold text-sm">
            <AlertTriangle className="w-4 h-4" />
            {state.title}
          </div>
          <button type="button" onClick={cancel} className="p-1 rounded hover:bg-white/10 cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="p-5 text-xs text-slate-700 space-y-2 leading-relaxed">{state.message}</div>
        <div className="px-5 py-3 bg-slate-50 border-t border-slate-100 flex justify-end gap-2">
          <button type="button" onClick={cancel} className="px-4 py-2 rounded-xl bg-white border border-slate-300 hover:bg-slate-100 text-slate-700 text-xs font-bold cursor-pointer">
            {state.cancelLabel || 'Non, revenir'}
          </button>
          <button
            type="button"
            onClick={async () => {
              const fn = state.onConfirm;
              onClose();
              await fn();
            }}
            className={`px-4 py-2 rounded-xl text-white text-xs font-bold cursor-pointer ${state.danger ? 'bg-rose-600 hover:bg-rose-700' : 'bg-indigo-600 hover:bg-indigo-700'}`}
          >
            {state.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};

const STATUT_STYLE: Record<StatutBordereau, string> = {
  BROUILLON: 'bg-slate-100 text-slate-700 border-slate-300',
  VALIDE: 'bg-indigo-100 text-indigo-800 border-indigo-300',
  REGLE: 'bg-emerald-100 text-emerald-800 border-emerald-300',
};
const STATUT_LABEL: Record<StatutBordereau, string> = { BROUILLON: 'Brouillon', VALIDE: 'Validé', REGLE: 'Réglé' };

export const StatutBadge: React.FC<{ statut: StatutBordereau }> = ({ statut }) => (
  <span className={`px-2 py-0.5 rounded text-[10px] font-extrabold uppercase border ${STATUT_STYLE[statut] || STATUT_STYLE.BROUILLON}`}>
    {STATUT_LABEL[statut] || statut}
  </span>
);

/**
 * Champ date au format JJ/MM/AAAA : les « / » sont placés automatiquement, la date est contrôlée
 * (jour existant). `onValidite` signale au formulaire si la saisie peut être enregistrée.
 */
export const DateInput: React.FC<{
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
  className?: string;
  onValidite?: (ok: boolean) => void;
}> = ({ value, onChange, disabled, className, onValidite }) => {
  const erreur = erreurDate(value);
  React.useEffect(() => {
    onValidite?.(!erreur);
  }, [erreur]);
  return (
    <div>
      <div className="flex gap-1">
        <input
          value={value}
          disabled={disabled}
          inputMode="numeric"
          maxLength={10}
          placeholder="JJ/MM/AAAA"
          onChange={(e) => onChange(masquerDate(e.target.value))}
          className={`${className || ''} ${erreur ? 'border-rose-400 bg-rose-50' : ''}`}
        />
        {!disabled && (
          <button
            type="button"
            tabIndex={-1}
            onClick={() => onChange(aujourdhui())}
            title="Date du jour"
            className="px-2 rounded-lg border border-slate-200 text-[10px] font-bold text-slate-600 hover:bg-slate-100 cursor-pointer mt-1"
          >
            Auj.
          </button>
        )}
      </div>
      {erreur && <div className="text-[10px] text-rose-700 mt-0.5">{erreur}</div>}
    </div>
  );
};
