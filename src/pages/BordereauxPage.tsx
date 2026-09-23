import React, { useState, useEffect, useRef } from 'react';
import {
  FileText,
  Printer,
  FileSpreadsheet,
  Trash2,
  Eye,
  EyeOff,
  Search,
  ArrowLeft,
  Paperclip,
  ExternalLink,
  Save,
  CheckCircle2,
  RotateCcw,
  BadgeCheck,
  Lock,
} from 'lucide-react';
import { Bordereau, FichierBordereau, StatutBordereau } from '../types';
import { api, errMsg, formatDZD, formatMontant, readFileAsDataUrl } from '../lib/api';
import {
  AnomaliesBadges,
  AnomaliesPanel,
  ConfirmDialog,
  ConfirmState,
  PolicesPanel,
  policeKey,
  recomputeTotals,
  StatutBadge,
} from '../components/Shared';

const FILE_TYPE_LABEL: Record<string, string> = {
  CIRIS_PDF: 'Rapport CIRIS',
  FLOTTE_PDF: 'PDF Flotte',
  EXCEL_PRODUCTION: 'Registre Excel',
};

const taille = (n: number) => (n > 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} Mo` : `${Math.max(1, Math.round(n / 1024))} Ko`);

export const BordereauxPage: React.FC = () => {
  const [bordereaux, setBordereaux] = useState<Bordereau[]>([]);
  const [selected, setSelected] = useState<Bordereau | null>(null);
  const [modifie, setModifie] = useState<boolean>(false);
  const [loading, setLoading] = useState<boolean>(true);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [filterType, setFilterType] = useState<string>('ALL');
  const [filterStatut, setFilterStatut] = useState<string>('ALL');
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const pjInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadBordereaux();
  }, []);

  const loadBordereaux = async () => {
    setLoading(true);
    try {
      setBordereaux(await api.getBordereaux());
    } catch (err) {
      setMessage({ type: 'error', text: errMsg(err) });
    } finally {
      setLoading(false);
    }
  };

  const reload = async (id: number) => {
    const full = await api.getBordereau(id);
    setSelected(full);
    setModifie(false);
  };

  const handleView = async (id: number) => {
    try {
      setMessage(null);
      await reload(id);
    } catch (err) {
      setMessage({ type: 'error', text: errMsg(err) });
    }
  };

  const backToList = () => {
    const go = () => {
      setSelected(null);
      setModifie(false);
      loadBordereaux();
    };
    if (modifie) {
      setConfirm({
        title: 'Modifications non enregistrées',
        danger: true,
        message: <p>Les lignes exclues / réintégrées n'ont pas été enregistrées. Quitter sans enregistrer ?</p>,
        confirmLabel: 'Quitter sans enregistrer',
        cancelLabel: 'Rester',
        onConfirm: go,
      });
    } else go();
  };

  const handleDelete = (b: Bordereau) => {
    setConfirm({
      title: 'Supprimer ce bordereau ?',
      danger: true,
      message: (
        <p>
          Le bordereau <strong>{b.numero_bordereau}</strong>, ses lignes et <strong>tous ses fichiers rattachés</strong> seront
          définitivement supprimés de la base.
        </p>
      ),
      confirmLabel: 'Supprimer définitivement',
      onConfirm: async () => {
        try {
          await api.deleteBordereau(b.id!);
          if (selected?.id === b.id) setSelected(null);
          setMessage({ type: 'success', text: `Bordereau ${b.numero_bordereau} supprimé.` });
          await loadBordereaux();
        } catch (err) {
          setMessage({ type: 'error', text: errMsg(err) });
        }
      },
    });
  };

  const handleStatut = (statut: StatutBordereau) => {
    if (!selected?.id) return;
    const libelles: Record<StatutBordereau, string> = {
      BROUILLON: 'repasser en brouillon (modifiable)',
      VALIDE: 'valider (le bordereau ne sera plus modifiable)',
      REGLE: 'marquer comme réglé',
    };
    setConfirm({
      title: 'Changer le statut',
      message: <p>Voulez-vous {libelles[statut]} le bordereau {selected.numero_bordereau} ?</p>,
      confirmLabel: 'Confirmer',
      onConfirm: async () => {
        try {
          await api.updateBordereauStatut(selected.id!, statut);
          await reload(selected.id!);
          setMessage({ type: 'success', text: `Statut mis à jour : ${statut}.` });
        } catch (err) {
          setMessage({ type: 'error', text: errMsg(err) });
        }
      },
    });
  };

  const toggleLigne = (index: number) => {
    if (!selected || selected.statut !== 'BROUILLON') return;
    setSelected(recomputeTotals(selected, selected.lignes.map((l, i) => (i === index ? { ...l, desactive: !l.desactive } : l))));
    setModifie(true);
  };

  // Exclusion / réintégration de polices non encaissées (bordereau en brouillon uniquement)
  const togglePolices = (keys: string[], exclue: boolean) => {
    if (!selected || selected.statut !== 'BROUILLON') return;
    const set = new Set(keys);
    const lignes = selected.lignes.map((l) => ({
      ...l,
      contributions: (l.contributions || []).map((c) =>
        c.police && set.has(policeKey(c.fichier_source, c.police)) ? { ...c, exclue } : c
      ),
    }));
    setSelected(recomputeTotals(selected, lignes));
    setModifie(true);
  };

  const saveModifications = async () => {
    if (!selected) return;
    try {
      // Les taux figés des lignes sont conservés : seuls les montants/totaux sont recalculés
      await api.saveBordereau(selected);
      await reload(selected.id!);
      setMessage({ type: 'success', text: 'Modifications enregistrées.' });
    } catch (err) {
      setMessage({ type: 'error', text: `Enregistrement refusé : ${errMsg(err)}` });
    }
  };

  const handleExportExcel = async (b: Bordereau) => {
    try {
      const full = b.lignes.length === 0 && b.id ? await api.getBordereau(b.id) : b;
      if (!full) throw new Error('Bordereau introuvable en base');
      const path = await api.exportExcel(full);
      setMessage({ type: 'success', text: `Bordereau exporté : ${path}` });
    } catch (err) {
      setMessage({ type: 'error', text: `Erreur export : ${errMsg(err)}` });
    }
  };

  const ouvrirFichier = async (f: FichierBordereau) => {
    try {
      const path = await api.ouvrirFichier(f.id!);
      setMessage({ type: 'success', text: `Fichier extrait et ouvert : ${path}` });
    } catch (err) {
      setMessage({ type: 'error', text: errMsg(err) });
    }
  };

  const ajouterPieces = async (list: FileList | null) => {
    if (!list || !selected?.id) return;
    try {
      for (const f of Array.from(list)) {
        const b64 = await readFileAsDataUrl(f);
        await api.addPieceJointeBase64(selected.id, f.name, b64);
      }
      await reload(selected.id);
      setMessage({ type: 'success', text: `${list.length} pièce(s) jointe(s) ajoutée(s).` });
    } catch (err) {
      setMessage({ type: 'error', text: errMsg(err) });
    }
  };

  const supprimerPiece = (f: FichierBordereau) => {
    setConfirm({
      title: 'Supprimer la pièce jointe ?',
      danger: true,
      message: <p>Supprimer définitivement « {f.nom_fichier} » ?</p>,
      confirmLabel: 'Supprimer',
      onConfirm: async () => {
        try {
          await api.deletePieceJointe(f.id!);
          await reload(selected!.id!);
        } catch (err) {
          setMessage({ type: 'error', text: errMsg(err) });
        }
      },
    });
  };

  const filtered = bordereaux.filter((b) => {
    const q = searchTerm.toLowerCase();
    const matchSearch =
      b.numero_bordereau.toLowerCase().includes(q) ||
      b.intermediaire_nom.toLowerCase().includes(q) ||
      b.intermediaire_code.toLowerCase().includes(q);
    return matchSearch && (filterType === 'ALL' || b.type_intermediaire === filterType) && (filterStatut === 'ALL' || b.statut === filterStatut);
  });

  const msgBox = message && (
    <div
      className={`p-3 rounded-xl text-xs font-medium border no-print ${
        message.type === 'success' ? 'bg-emerald-50 border-emerald-200 text-emerald-900' : 'bg-rose-50 border-rose-200 text-rose-900'
      }`}
    >
      {message.text}
    </div>
  );

  if (selected) {
    const b = selected;
    const courtier = b.type_intermediaire === 'COURTIER';
    const editable = b.statut === 'BROUILLON';
    const imports = (b.fichiers || []).filter((f) => f.type_piece === 'IMPORT');
    const pieces = (b.fichiers || []).filter((f) => f.type_piece === 'PIECE_JOINTE');
    return (
      <div className="space-y-6 pb-12">
        <div className="flex flex-wrap items-center justify-between gap-3 bg-white p-4 rounded-xl border border-slate-200 shadow-xs no-print">
          <button onClick={backToList} className="px-3.5 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-2 cursor-pointer">
            <ArrowLeft className="w-4 h-4" /> Retour à la liste
          </button>
          <div className="flex flex-wrap items-center gap-2">
            <StatutBadge statut={b.statut} />
            {b.statut === 'BROUILLON' && (
              <button onClick={() => handleStatut('VALIDE')} disabled={modifie} className="px-3 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 disabled:opacity-40 text-white text-xs font-bold flex items-center gap-1.5 cursor-pointer">
                <BadgeCheck className="w-4 h-4" /> Valider
              </button>
            )}
            {b.statut === 'VALIDE' && (
              <>
                <button onClick={() => handleStatut('BROUILLON')} className="px-3 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-1.5 cursor-pointer">
                  <RotateCcw className="w-4 h-4" /> Repasser en brouillon
                </button>
                <button onClick={() => handleStatut('REGLE')} className="px-3 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold flex items-center gap-1.5 cursor-pointer">
                  <CheckCircle2 className="w-4 h-4" /> Marquer réglé
                </button>
              </>
            )}
            {b.statut === 'REGLE' && (
              <button onClick={() => handleStatut('VALIDE')} className="px-3 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-1.5 cursor-pointer">
                <RotateCcw className="w-4 h-4" /> Annuler le règlement
              </button>
            )}
            {modifie && (
              <button onClick={saveModifications} className="px-3 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 text-xs font-bold flex items-center gap-1.5 cursor-pointer">
                <Save className="w-4 h-4" /> Enregistrer les modifications
              </button>
            )}
            <button onClick={() => handleExportExcel(b)} className="px-3 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold flex items-center gap-1.5 cursor-pointer">
              <FileSpreadsheet className="w-4 h-4" /> Excel
            </button>
            <button onClick={() => window.print()} className="px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-900 text-white text-xs font-bold flex items-center gap-1.5 cursor-pointer">
              <Printer className="w-4 h-4" /> Imprimer
            </button>
            <button
              onClick={() => handleDelete(b)}
              disabled={b.statut === 'REGLE'}
              title={b.statut === 'REGLE' ? 'Un bordereau réglé ne peut pas être supprimé' : 'Supprimer'}
              className="p-2 rounded-lg bg-rose-50 hover:bg-rose-100 disabled:opacity-40 text-rose-700 cursor-pointer"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        </div>
        {msgBox}
        {!editable && (
          <div className="p-3 rounded-xl bg-slate-100 border border-slate-200 text-slate-700 text-xs flex items-center gap-2 no-print">
            <Lock className="w-4 h-4" /> Bordereau {b.statut === 'VALIDE' ? 'validé' : 'réglé'} : lecture seule. Les taux et montants affichés sont ceux enregistrés.
          </div>
        )}

        {/* Feuille officielle */}
        <div className="bg-white p-8 md:p-12 rounded-2xl border border-slate-200 shadow-sm print:shadow-none print:border-none print:p-0 max-w-5xl mx-auto">
          <div className="border-b-2 border-slate-900 pb-4 mb-6">
            <div className="flex justify-between items-start">
              <div>
                <h2 className="text-xl font-black tracking-tight text-slate-900 uppercase">{b.succursale}</h2>
                <p className="text-sm font-semibold text-slate-600">{b.departement}</p>
              </div>
              <div className="text-right">
                <div className="text-base font-black text-indigo-900">{b.numero_bordereau}</div>
                <div className="text-xs text-slate-500">Date : {b.date_generation}</div>
              </div>
            </div>
            <div className="mt-4 py-2 bg-slate-100 text-center rounded border border-slate-300">
              <h1 className="text-lg font-black text-slate-900 uppercase tracking-wide">Commission {courtier ? 'COURTIER' : 'AGA'}</h1>
            </div>
            <div className="grid grid-cols-2 gap-4 mt-4 text-xs">
              <div className="space-y-1">
                <p>
                  <span className="font-bold text-slate-700">{courtier ? 'COURTIER :' : 'AGA :'}</span> <span className="font-semibold">{b.intermediaire_nom}</span>
                </p>
                <p>
                  <span className="font-bold text-slate-700">Code :</span> <span className="font-semibold">{b.intermediaire_code}</span>
                </p>
                {courtier && b.code_client && (
                  <p>
                    <span className="font-bold text-slate-700">Code client :</span> <span className="font-semibold">{b.code_client}</span>
                  </p>
                )}
              </div>
              <div className="space-y-1 text-right">
                {b.mois && (
                  <p>
                    <span className="font-bold text-slate-700">Mois :</span> <span className="font-semibold">{String(b.mois).padStart(2, '0')}</span>
                  </p>
                )}
                <p>
                  <span className="font-bold text-slate-700">EXERCICE :</span> <span className="font-semibold">{b.exercice}</span>
                </p>
              </div>
            </div>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs border border-slate-300">
              <thead>
                <tr className="bg-slate-200 border-b border-slate-300 text-slate-900 font-bold uppercase">
                  <th className="p-2 border-r border-slate-300">Branche / Garantie</th>
                  <th className="p-2 border-r border-slate-300 text-right">Prime nette</th>
                  <th className="p-2 border-r border-slate-300 text-center">{courtier ? "Taux d'apport" : 'Apport'}</th>
                  {!courtier && <th className="p-2 border-r border-slate-300 text-center">Gestion</th>}
                  <th className="p-2 border-r border-slate-300 text-right">Commission H.T</th>
                  <th className="p-2 border-r border-slate-300 text-right">TVA ({(b.tva_taux * 100).toFixed(0)} %)</th>
                  <th className="p-2 text-right">Commission T.T.C</th>
                  {editable && <th className="p-2 text-center no-print">Inclure</th>}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200">
                {b.lignes.map((l, idx) => {
                  const st = l.desactive ? 'line-through text-slate-400' : '';
                  return (
                    <tr key={idx} className={l.desactive ? 'bg-slate-100/80 italic' : ''}>
                      <td className="p-2 border-r border-slate-200">
                        <span className={st}>
                          {l.code_garantie && l.code_garantie !== l.libelle_garantie && <span className="font-bold">{l.code_garantie} - </span>}
                          {l.libelle_garantie}
                        </span>
                        {l.desactive && <span className="ml-2 text-[10px] font-bold not-italic">(exclue du calcul)</span>}
                      </td>
                      <td className={`p-2 border-r border-slate-200 text-right font-mono ${st}`}>{formatMontant(l.prime_nette)}</td>
                      <td className={`p-2 border-r border-slate-200 text-right font-mono ${st}`}>
                        {courtier ? `${(l.taux_apport * 100).toFixed(2)} %` : `${formatMontant(l.montant_apport)} (${(l.taux_apport * 100).toFixed(2)} %)`}
                      </td>
                      {!courtier && (
                        <td className={`p-2 border-r border-slate-200 text-right font-mono ${st}`}>
                          {formatMontant(l.montant_gestion)} ({(l.taux_gestion * 100).toFixed(2)} %)
                        </td>
                      )}
                      <td className={`p-2 border-r border-slate-200 text-right font-mono font-bold ${st}`}>{formatMontant(l.montant_ht)}</td>
                      <td className={`p-2 border-r border-slate-200 text-right font-mono ${st}`}>{formatMontant(l.montant_tva)}</td>
                      <td className={`p-2 text-right font-mono font-bold ${st}`}>{formatMontant(l.montant_ttc)}</td>
                      {editable && (
                        <td className="p-2 text-center no-print">
                          <button onClick={() => toggleLigne(idx)} className="p-1 rounded hover:bg-slate-200 cursor-pointer" title={l.desactive ? 'Réintégrer' : 'Exclure'}>
                            {l.desactive ? <EyeOff className="w-4 h-4 text-slate-400" /> : <Eye className="w-4 h-4 text-indigo-600" />}
                          </button>
                        </td>
                      )}
                    </tr>
                  );
                })}
              </tbody>
              <tfoot>
                <tr className="bg-slate-200 border-t-2 border-slate-400 font-black text-slate-900">
                  <td className="p-2.5 border-r border-slate-300 uppercase">Total général</td>
                  <td className="p-2.5 border-r border-slate-300 text-right font-mono">{formatMontant(b.total_prime_nette)}</td>
                  <td className="p-2.5 border-r border-slate-300 text-right font-mono">{courtier ? '' : formatMontant(b.total_commission_apport)}</td>
                  {!courtier && <td className="p-2.5 border-r border-slate-300 text-right font-mono">{formatMontant(b.total_commission_gestion)}</td>}
                  <td className="p-2.5 border-r border-slate-300 text-right font-mono">{formatMontant(b.total_commission_ht)}</td>
                  <td className="p-2.5 border-r border-slate-300 text-right font-mono">{formatMontant(b.total_tva)}</td>
                  <td className="p-2.5 text-right font-mono text-sm">{formatDZD(b.total_commission_ttc)}</td>
                  {editable && <td className="no-print" />}
                </tr>
              </tfoot>
            </table>
          </div>

          <div className="grid grid-cols-3 gap-8 mt-12 pt-8 text-xs text-center border-t border-slate-200">
            {['Le Chef de Service', 'Le Directeur Commercial', "L'Intermédiaire"].map((t) => (
              <div key={t}>
                <p className="font-bold text-slate-700 uppercase">{t}</p>
                <div className="h-16" />
                <p className="text-slate-400 italic">{t === "L'Intermédiaire" ? b.intermediaire_nom : 'Signature & Cachet'}</p>
              </div>
            ))}
          </div>
        </div>

        <div className="max-w-5xl mx-auto">
          <PolicesPanel lignes={b.lignes} readOnly={!editable} onToggle={togglePolices} />
        </div>

        {/* Fichiers rattachés */}
        <div className="bg-white rounded-2xl border border-slate-200 shadow-xs p-5 space-y-4 no-print max-w-5xl mx-auto">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-bold text-slate-900 flex items-center gap-2">
              <Paperclip className="w-4 h-4 text-indigo-600" /> Fichiers rattachés ({(b.fichiers || []).length})
            </h3>
            <input ref={pjInputRef} type="file" multiple className="hidden" onChange={async (e) => { await ajouterPieces(e.target.files); e.target.value = ''; }} />
            <button onClick={() => pjInputRef.current?.click()} className="px-3 py-1.5 rounded-lg bg-indigo-50 hover:bg-indigo-100 text-indigo-700 text-xs font-bold cursor-pointer">
              + Ajouter une pièce jointe
            </button>
          </div>

          <div className="space-y-2">
            <div className="text-[11px] font-black uppercase tracking-wider text-slate-500">Fichiers sources de l'import (justificatifs du calcul)</div>
            {imports.length === 0 && <p className="text-xs text-slate-400">Aucun fichier source enregistré pour ce bordereau.</p>}
            {imports.map((f) => (
              <details key={f.id} className="rounded-xl border border-slate-200 bg-slate-50 text-xs">
                <summary className="flex items-center justify-between gap-3 p-3 cursor-pointer">
                  <span className="flex items-center gap-2 flex-wrap">
                    <strong>{f.nom_fichier}</strong>
                    <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-indigo-100 text-indigo-800">{FILE_TYPE_LABEL[f.type_fichier] || f.type_fichier}</span>
                    <AnomaliesBadges anomalies={f.anomalies} />
                    <span className="text-slate-500">
                      {f.client_nom && `${f.client_nom} `}
                      {f.periode && `• ${f.periode} `}• {f.nb_lignes} lignes • {taille(f.taille)} • ajouté le {f.date_ajout}
                    </span>
                  </span>
                  <span className="flex items-center gap-2 shrink-0">
                    <span className="font-mono font-bold">{formatDZD(f.total_prime_nette)}</span>
                    <button onClick={(e) => { e.preventDefault(); ouvrirFichier(f); }} className="p-1.5 rounded-lg hover:bg-slate-200 cursor-pointer" title="Ouvrir">
                      <ExternalLink className="w-4 h-4 text-indigo-600" />
                    </button>
                  </span>
                </summary>
                {f.anomalies.length > 0 && (
                  <div className="px-3 pb-3">
                    <AnomaliesPanel anomalies={f.anomalies} />
                  </div>
                )}
              </details>
            ))}
          </div>

          <div className="space-y-2">
            <div className="text-[11px] font-black uppercase tracking-wider text-slate-500">Pièces jointes</div>
            {pieces.length === 0 && <p className="text-xs text-slate-400">Aucune pièce jointe (ex : bordereau signé, justificatif de règlement).</p>}
            {pieces.map((f) => (
              <div key={f.id} className="flex items-center justify-between p-3 rounded-xl border border-slate-200 text-xs">
                <span>
                  <strong>{f.nom_fichier}</strong> <span className="text-slate-500">• {taille(f.taille)} • {f.date_ajout}</span>
                </span>
                <span className="flex items-center gap-1">
                  <button onClick={() => ouvrirFichier(f)} className="p-1.5 rounded-lg hover:bg-slate-100 cursor-pointer" title="Ouvrir">
                    <ExternalLink className="w-4 h-4 text-indigo-600" />
                  </button>
                  <button onClick={() => supprimerPiece(f)} className="p-1.5 rounded-lg hover:bg-rose-50 cursor-pointer" title="Supprimer">
                    <Trash2 className="w-4 h-4 text-rose-600" />
                  </button>
                </span>
              </div>
            ))}
          </div>

          {(b.anomalies || []).length > 0 && (
            <details>
              <summary className="text-xs font-bold text-slate-700 cursor-pointer">Contrôles enregistrés lors du calcul ({(b.anomalies || []).length})</summary>
              <div className="mt-2">
                <AnomaliesPanel anomalies={b.anomalies || []} lignes={b.lignes} />
              </div>
            </details>
          )}
        </div>
        <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
      </div>
    );
  }

  return (
    <div className="space-y-6 pb-12">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-black text-slate-900 tracking-tight">Historique des bordereaux</h1>
          <p className="text-xs text-slate-500 mt-0.5">
            Rouvrez un bordereau enregistré : taux et montants sont relus tels qu'enregistrés, avec ses fichiers sources.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
            <input
              type="text"
              placeholder="Numéro, nom, code…"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="pl-9 pr-4 py-2 bg-white border border-slate-200 rounded-lg text-xs font-medium w-56"
            />
          </div>
          <select value={filterType} onChange={(e) => setFilterType(e.target.value)} className="bg-white border border-slate-200 rounded-lg px-3 py-2 text-xs font-semibold">
            <option value="ALL">Tous types</option>
            <option value="AGA">AGA</option>
            <option value="COURTIER">Courtiers</option>
          </select>
          <select value={filterStatut} onChange={(e) => setFilterStatut(e.target.value)} className="bg-white border border-slate-200 rounded-lg px-3 py-2 text-xs font-semibold">
            <option value="ALL">Tous statuts</option>
            <option value="BROUILLON">Brouillons</option>
            <option value="VALIDE">Validés</option>
            <option value="REGLE">Réglés</option>
          </select>
        </div>
      </div>
      {msgBox}

      <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-slate-600 font-bold uppercase tracking-wider">
                <th className="px-4 py-3">Numéro</th>
                <th className="px-4 py-3">Intermédiaire</th>
                <th className="px-4 py-3">Type</th>
                <th className="px-4 py-3 text-center">Période</th>
                <th className="px-4 py-3 text-center">Statut</th>
                <th className="px-4 py-3 text-right">Prime nette</th>
                <th className="px-4 py-3 text-right">Commission HT</th>
                <th className="px-4 py-3 text-right">Commission TTC</th>
                <th className="px-4 py-3 text-center">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading ? (
                <tr>
                  <td colSpan={9} className="text-center py-8 text-slate-400">Chargement…</td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={9} className="text-center py-12 text-slate-400">
                    <FileText className="w-8 h-8 mx-auto text-slate-300 mb-2" /> Aucun bordereau.
                  </td>
                </tr>
              ) : (
                filtered.map((b) => (
                  <tr key={b.id} className="hover:bg-slate-50/80">
                    <td className="px-4 py-3 font-mono font-bold text-indigo-600">{b.numero_bordereau}</td>
                    <td className="px-4 py-3">
                      <div className="font-bold text-slate-900">{b.intermediaire_nom}</div>
                      <div className="text-[10px] text-slate-400 font-mono">Code : {b.intermediaire_code}</div>
                    </td>
                    <td className="px-4 py-3 font-bold">{b.type_intermediaire}</td>
                    <td className="px-4 py-3 text-center">
                      {b.mois ? String(b.mois).padStart(2, '0') + '/' : ''}
                      {b.exercice}
                    </td>
                    <td className="px-4 py-3 text-center">
                      <StatutBadge statut={b.statut} />
                    </td>
                    <td className="px-4 py-3 text-right font-mono">{formatMontant(b.total_prime_nette)}</td>
                    <td className="px-4 py-3 text-right font-mono font-semibold text-indigo-900">{formatMontant(b.total_commission_ht)}</td>
                    <td className="px-4 py-3 text-right font-mono font-extrabold text-emerald-700">{formatMontant(b.total_commission_ttc)}</td>
                    <td className="px-4 py-3">
                      <div className="flex items-center justify-center gap-1.5">
                        <button onClick={() => handleView(b.id!)} title="Ouvrir" className="p-1.5 rounded-lg bg-indigo-50 hover:bg-indigo-100 text-indigo-700 cursor-pointer">
                          <Eye className="w-3.5 h-3.5" />
                        </button>
                        <button onClick={() => handleExportExcel(b)} title="Exporter Excel" className="p-1.5 rounded-lg bg-emerald-50 hover:bg-emerald-100 text-emerald-700 cursor-pointer">
                          <FileSpreadsheet className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => handleDelete(b)}
                          disabled={b.statut === 'REGLE'}
                          title={b.statut === 'REGLE' ? 'Bordereau réglé : suppression impossible' : 'Supprimer'}
                          className="p-1.5 rounded-lg bg-rose-50 hover:bg-rose-100 disabled:opacity-40 text-rose-700 cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
    </div>
  );
};
