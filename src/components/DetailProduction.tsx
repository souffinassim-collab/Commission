import React, { useEffect, useMemo, useState } from 'react';
import { Save, Printer, FileSpreadsheet, Copy, AlertTriangle, CheckCircle2 } from 'lucide-react';
import { Bordereau, DetailProduction, FactureIntermediaire } from '../types';
import { api, errMsg, formatMontant } from '../lib/api';

interface Props {
  bordereau: Bordereau;
  /** Saisie possible (bordereau enregistré, non réglé) */
  modifiable: boolean;
  onSaved: () => Promise<void> | void;
  onMessage: (m: { type: 'success' | 'error'; text: string }) => void;
}

/** Montant saisi (accepte la virgule et les espaces) ; vide = non saisi */
const lireMontant = (s: string): number | undefined => {
  const t = s.replace(/\s/g, '').replace(',', '.');
  if (!t) return undefined;
  const v = Number(t);
  return Number.isFinite(v) ? v : undefined;
};

const fmt = (v?: number | null) => (v === undefined || v === null ? '' : formatMontant(v));
const pct = (t: number) => `${(Math.round(t * 10000) / 100).toLocaleString('fr-DZ')}%`;

/**
 * « Détail de production » à confirmer : le commercial saisit la facture de l'intermédiaire (commission HT
 * déclarée par branche), l'application calcule les écarts avec la commission calculée. Même présentation
 * que le modèle du service commercial, imprimable (A4 paysage) et exportable en Excel.
 */
export const DetailProductionView: React.FC<Props> = ({ bordereau, modifiable, onSaved, onMessage }) => {
  const [numero, setNumero] = useState(bordereau.facture?.numero || '');
  const [date, setDate] = useState(bordereau.facture?.date || '');
  const [commentaire, setCommentaire] = useState(bordereau.facture?.commentaire || '');
  const [saisies, setSaisies] = useState<Record<string, string>>(() =>
    Object.fromEntries(Object.entries(bordereau.facture?.montants_ht || {}).map(([k, v]) => [k, String(v).replace('.', ',')]))
  );
  const [detail, setDetail] = useState<DetailProduction | null>(null);
  const [enregistre, setEnregistre] = useState(true);
  const [saving, setSaving] = useState(false);

  const facture: FactureIntermediaire = useMemo(() => {
    const montants_ht: Record<string, number> = {};
    for (const [k, v] of Object.entries(saisies)) {
      const n = lireMontant(v);
      if (n !== undefined) montants_ht[k] = n;
    }
    return { numero: numero.trim(), date: date.trim(), commentaire: commentaire.trim(), montants_ht };
  }, [numero, date, commentaire, saisies]);

  const factureVide = !facture.numero && !facture.date && Object.keys(facture.montants_ht).length === 0;

  // Détail recalculé (côté application) à chaque saisie : mêmes chiffres à l'écran, à l'impression et en Excel
  useEffect(() => {
    let annule = false;
    const t = setTimeout(async () => {
      try {
        const d = await api.detailProduction({ ...bordereau, facture: factureVide ? undefined : facture });
        if (!annule) setDetail(d);
      } catch (e) {
        if (!annule) onMessage({ type: 'error', text: errMsg(e) });
      }
    }, 200);
    return () => {
      annule = true;
      clearTimeout(t);
    };
  }, [bordereau, facture, factureVide]);

  const modifier = (fn: () => void) => {
    fn();
    setEnregistre(false);
  };

  const enregistrer = async () => {
    if (!bordereau.id) return;
    setSaving(true);
    try {
      await api.enregistrerFactureIntermediaire(bordereau.id, factureVide ? undefined : facture);
      setEnregistre(true);
      onMessage({ type: 'success', text: 'Facture de l’intermédiaire enregistrée avec le bordereau.' });
      await onSaved();
    } catch (e) {
      onMessage({ type: 'error', text: `Facture non enregistrée : ${errMsg(e)}` });
    } finally {
      setSaving(false);
    }
  };

  const reprendreCalcul = () =>
    modifier(() =>
      setSaisies(Object.fromEntries((detail?.lignes || []).map((l) => [l.cle, String(l.com_ht_calculee).replace('.', ',')])))
    );

  const exporter = async () => {
    try {
      const path = await api.exportDetailProduction({ ...bordereau, facture: factureVide ? undefined : facture });
      onMessage({ type: 'success', text: `Détail de production exporté : ${path}` });
    } catch (e) {
      onMessage({ type: 'error', text: `Erreur export : ${errMsg(e)}` });
    }
  };

  if (!detail) return <div className="text-xs text-slate-400 p-6">Préparation du détail de production…</div>;
  const d = detail;
  const role = d.type_intermediaire === 'COURTIER' ? 'Courtier' : 'AGA';
  const ecartCls = (v?: number | null) =>
    v === undefined || v === null ? 'text-slate-300' : Math.abs(v) < 0.005 ? 'text-emerald-700' : 'text-rose-700 font-bold';
  const td = 'border border-slate-300 px-1.5 py-1';
  const tdNum = `${td} text-right font-mono whitespace-nowrap`;
  const th = 'border border-slate-300 px-1.5 py-1 bg-slate-100 font-bold text-left';
  const ecartsNonNuls = d.lignes.filter((l) => l.ecart !== undefined && l.ecart !== null && Math.abs(l.ecart) >= 0.005).length;

  return (
    <div className="space-y-4">
      <style>{'@media print { @page { size: A4 landscape; margin: 8mm; } }'}</style>

      {/* Saisie de la facture */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-xs p-4 no-print max-w-6xl mx-auto space-y-3 text-xs">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-bold text-slate-900">Facture de l’intermédiaire — confirmation</h3>
          <div className="flex flex-wrap items-center gap-2">
            {!enregistre && <span className="text-[11px] font-bold text-amber-700">Saisie non enregistrée</span>}
            {modifiable && (
              <button
                type="button"
                onClick={reprendreCalcul}
                className="px-3 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 font-bold flex items-center gap-1.5 cursor-pointer"
                title="Pré-remplir avec la commission calculée (à corriger selon la facture)"
              >
                <Copy className="w-3.5 h-3.5" /> Reprendre les montants calculés
              </button>
            )}
            {modifiable && (
              <button
                type="button"
                onClick={enregistrer}
                disabled={saving || enregistre}
                className="px-3 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 disabled:opacity-40 text-white font-bold flex items-center gap-1.5 cursor-pointer"
              >
                <Save className="w-3.5 h-3.5" /> {saving ? 'Enregistrement…' : 'Enregistrer la facture'}
              </button>
            )}
            <button type="button" onClick={exporter} className="px-3 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-bold flex items-center gap-1.5 cursor-pointer">
              <FileSpreadsheet className="w-3.5 h-3.5" /> Excel
            </button>
            <button type="button" onClick={() => window.print()} className="px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-900 text-white font-bold flex items-center gap-1.5 cursor-pointer">
              <Printer className="w-3.5 h-3.5" /> Imprimer
            </button>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
          <label className="block">
            <span className="font-semibold text-slate-600">N° de facture</span>
            <input value={numero} disabled={!modifiable} onChange={(e) => modifier(() => setNumero(e.target.value))} className="w-full border border-slate-200 rounded-lg p-2 mt-1" />
          </label>
          <label className="block">
            <span className="font-semibold text-slate-600">Date de facture</span>
            <input value={date} disabled={!modifiable} placeholder="JJ/MM/AAAA" onChange={(e) => modifier(() => setDate(e.target.value))} className="w-full border border-slate-200 rounded-lg p-2 mt-1" />
          </label>
          <label className="block md:col-span-2">
            <span className="font-semibold text-slate-600">Observation</span>
            <input value={commentaire} disabled={!modifiable} onChange={(e) => modifier(() => setCommentaire(e.target.value))} className="w-full border border-slate-200 rounded-lg p-2 mt-1" />
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          {d.lignes_non_saisies > 0 && (
            <span className="px-2 py-1 rounded-lg bg-amber-50 border border-amber-200 text-amber-900 flex items-center gap-1">
              <AlertTriangle className="w-3.5 h-3.5" /> {d.lignes_non_saisies} branche(s) sans montant de facture saisi
            </span>
          )}
          {d.lignes_non_saisies === 0 && ecartsNonNuls === 0 && (
            <span className="px-2 py-1 rounded-lg bg-emerald-50 border border-emerald-200 text-emerald-900 flex items-center gap-1">
              <CheckCircle2 className="w-3.5 h-3.5" /> Facture conforme au calcul
            </span>
          )}
          {ecartsNonNuls > 0 && (
            <span className="px-2 py-1 rounded-lg bg-rose-50 border border-rose-200 text-rose-900 flex items-center gap-1">
              <AlertTriangle className="w-3.5 h-3.5" /> {ecartsNonNuls} écart(s) — total HT {fmt(d.ecart_ht)}, TTC {fmt(d.ecart_ttc)}
            </span>
          )}
          {Math.abs(d.controle) >= 0.005 && (
            <span className="px-2 py-1 rounded-lg bg-rose-50 border border-rose-200 text-rose-900">
              Contrôle : total des branches ≠ total du détail ({fmt(d.controle)})
            </span>
          )}
        </div>
      </div>

      {/* Document « Détail de production » (présentation du modèle) */}
      <div className="bg-white p-6 md:p-8 rounded-2xl border border-slate-200 shadow-sm print:shadow-none print:border-none print:p-0 max-w-6xl mx-auto text-[11px] text-slate-900">
        <div className="font-bold">{d.succursale}</div>
        <div className="font-bold">{d.departement}</div>
        <div className="text-center font-black text-base my-2">Détail de production</div>
        <div className="grid grid-cols-[110px_1fr] gap-x-2 gap-y-0.5 font-bold mb-3">
          <span>Client :</span>
          <span>{d.clients.join(' & ') || '—'}</span>
          <span>{role} :</span>
          <span>{d.intermediaire}</span>
          <span>EXERCICE</span>
          <span>
            {d.exercice}
            {d.mois ? ` (mois ${String(d.mois).padStart(2, '0')})` : ''}
          </span>
          <span className="font-normal">Bordereau :</span>
          <span className="font-normal">{d.numero_bordereau}</span>
          {(facture.numero || facture.date) && (
            <>
              <span className="font-normal">Facture :</span>
              <span className="font-normal">
                N° {facture.numero} {facture.date && `du ${facture.date}`}
              </span>
            </>
          )}
        </div>

        {/* Détail par police */}
        <table className="w-full border-collapse mb-5">
          <thead>
            <tr>
              {['Exercice', 'Police', 'Branche', "Date d'émission", "Date d'éffet", "Date d'échéance", 'Prime Nette', ...d.colonnes_ventilation, 'CP', 'Prime ttc'].map((h) => (
                <th key={h} className={th}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {d.polices.map((p) => (
              <tr key={p.police}>
                <td className={td}>{p.exercice}</td>
                <td className={`${td} font-mono`}>{p.police}</td>
                <td className={td}>{p.branche}</td>
                <td className={`${td} text-center`}>{p.date_emission}</td>
                <td className={`${td} text-center`}>{p.date_effet}</td>
                <td className={`${td} text-center`}>{p.date_echeance}</td>
                <td className={tdNum}>{fmt(p.prime_nette)}</td>
                {p.ventilation.map((v, i) => (
                  <td key={i} className={tdNum}>
                    {fmt(v)}
                  </td>
                ))}
                <td className={tdNum}>{fmt(p.cp)}</td>
                <td className={tdNum}>{fmt(p.prime_ttc)}</td>
              </tr>
            ))}
            <tr className="font-bold bg-slate-100">
              <td className={td} colSpan={6}>
                TOTAL
              </td>
              <td className={tdNum}>{fmt(d.total_prime_nette)}</td>
              {d.total_ventilation.map((v, i) => (
                <td key={i} className={tdNum}>
                  {fmt(v)}
                </td>
              ))}
              <td className={tdNum}>{fmt(d.total_cp)}</td>
              <td className={tdNum}>{fmt(d.total_prime_ttc)}</td>
            </tr>
          </tbody>
        </table>

        {/* Rapprochement commission calculée / facture */}
        <table className="border-collapse mb-4 min-w-[640px]">
          <thead>
            <tr>
              {['Branche', 'P-NET', 'TAUX', 'COM-HT', 'VOTRE-COM-HT', 'Ecart'].map((h) => (
                <th key={h} className={th}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {d.lignes.map((l) => (
              <tr key={l.cle}>
                <td className={td}>{l.libelle}</td>
                <td className={tdNum}>{fmt(l.prime_nette)}</td>
                <td className={`${td} text-right`}>{pct(l.taux)}</td>
                <td className={tdNum}>{fmt(l.com_ht_calculee)}</td>
                <td className={`${tdNum} p-0`}>
                  <input
                    value={saisies[l.cle] ?? ''}
                    disabled={!modifiable}
                    onChange={(e) => modifier(() => setSaisies((s) => ({ ...s, [l.cle]: e.target.value })))}
                    inputMode="decimal"
                    placeholder="à saisir"
                    className="w-32 text-right font-mono px-1.5 py-1 bg-amber-50/60 focus:bg-white outline-none print:hidden"
                  />
                  <span className="hidden print:inline px-1.5">{fmt(l.com_ht_declaree)}</span>
                </td>
                <td className={`${tdNum} ${ecartCls(l.ecart)}`}>{fmt(l.ecart)}</td>
              </tr>
            ))}
            <tr className="font-bold">
              <td className={td}>TOTAL</td>
              <td className={tdNum}>{fmt(d.total_prime_lignes)}</td>
              <td className={td} colSpan={4} />
            </tr>
            <tr>
              <td className={td}>Contrôle (branches − détail)</td>
              <td className={`${tdNum} ${ecartCls(d.controle)}`}>{fmt(d.controle)}</td>
              <td className={`${td} font-bold`}>TOTAL HT</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.ht_calcule)}</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.ht_declare)}</td>
              <td className={`${tdNum} ${ecartCls(d.ecart_ht)}`}>{fmt(d.ecart_ht)}</td>
            </tr>
            <tr>
              <td className={td} colSpan={2} />
              <td className={`${td} font-bold`}>TVA {pct(d.tva_taux)}</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.tva_calculee)}</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.tva_declaree)}</td>
              <td className={`${tdNum} ${ecartCls(d.ecart_tva)}`}>{fmt(d.ecart_tva)}</td>
            </tr>
            <tr>
              <td className={td} colSpan={2} />
              <td className={`${td} font-bold`}>TOTAL T.T.C</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.ttc_calcule)}</td>
              <td className={`${tdNum} font-bold`}>{fmt(d.ttc_declare)}</td>
              <td className={`${tdNum} ${ecartCls(d.ecart_ttc)}`}>{fmt(d.ecart_ttc)}</td>
            </tr>
          </tbody>
        </table>

        {d.polices_exclues.length > 0 && (
          <p className="mb-3 text-slate-600">
            Polices exclues (non encaissées) : {d.polices_exclues.length} —{' '}
            {fmt(d.polices_exclues.reduce((s, p) => s + p[2], 0))} DZD non commissionnés ({d.polices_exclues.map((p) => p[0]).join(', ')}).
          </p>
        )}
        {facture.commentaire && <p className="mb-3">Observation : {facture.commentaire}</p>}

        <div className="text-center font-bold mt-8">Vérification et Confirmation d'encaissement par le directeur d'Agence</div>
        <div className="h-20" />
      </div>
    </div>
  );
};
