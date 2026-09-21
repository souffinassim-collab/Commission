import React, { useState, useRef, useEffect } from 'react';
import { 
  UploadCloud, 
  FileText, 
  CheckCircle, 
  AlertCircle, 
  Save, 
  Printer, 
  FileSpreadsheet, 
  RotateCcw,
  Percent,
  Coins,
  ShieldCheck,
  Building2,
  FolderOpen
} from 'lucide-react';
import { CirisPdfExtractedData, Bordereau, IntermediaireType } from '../types';
import { api, isTauri } from '../lib/api';

interface ImportCirisPageProps {
  onBordereauSaved?: (id: number) => void;
  onNavigateToBordereau?: (bordereau: Bordereau) => void;
}

export const ImportCirisPage: React.FC<ImportCirisPageProps> = ({ onBordereauSaved, onNavigateToBordereau }) => {
  const [extractedData, setExtractedData] = useState<CirisPdfExtractedData | null>(null);
  const [calculatedBordereau, setCalculatedBordereau] = useState<Bordereau | null>(null);
  const [typeIntermediaire, setTypeIntermediaire] = useState<IntermediaireType>('AGA');
  const [tvaTaux, setTvaTaux] = useState<number>(0.19);
  const [succursale, setSuccursale] = useState<string>('SUCCURSALE SUD');
  const [departement, setDepartement] = useState<string>('Département Commercial');
  const [loading, setLoading] = useState<boolean>(false);
  const [saving, setSaving] = useState<boolean>(false);
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  // Traitement centralisé des données extraites du PDF
  const applyExtractedData = async (
    data: CirisPdfExtractedData,
    fileName: string,
    curType = typeIntermediaire,
    curTva = tvaTaux,
    curSucc = succursale,
    curDept = departement
  ) => {
    setExtractedData(data);

    // Détection automatique du type d'intermédiaire si mentionné dans le nom
    let detectedType = curType;
    if (data.agence_nom.toUpperCase().includes('COURTIER') || data.agence_code.toUpperCase().includes('CRT')) {
      detectedType = 'COURTIER';
      setTypeIntermediaire('COURTIER');
    } else if (data.agence_nom.toUpperCase().includes('AGA')) {
      detectedType = 'AGA';
      setTypeIntermediaire('AGA');
    }

    const computed = await api.calculateBordereauFromPdf(data, detectedType, curTva, curSucc, curDept);
    setCalculatedBordereau(computed);
    setMessage({
      type: 'success',
      text: `Fichier "${fileName}" analysé avec succès pour ${data.agence_nom} (Période : ${data.periode_debut} au ${data.periode_fin}) — ${computed.lignes.length} garanties calculées !`,
    });
  };

  // Traitement à partir d'un chemin de fichier natif (drag & drop Windows ou dialogue fichier)
  const processFilePath = async (filePath: string) => {
    setLoading(true);
    setMessage(null);
    try {
      const fileName = filePath.split('\\').pop() || filePath.split('/').pop() || filePath;
      const data = await api.parseCirisPdf(filePath);
      await applyExtractedData(data, fileName);
    } catch (err: any) {
      setMessage({ type: 'error', text: `Erreur d'analyse du fichier : ${err.message || err}` });
    } finally {
      setLoading(false);
    }
  };

  // Écouteur pour le Drag & Drop natif de Windows via Tauri v2
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupTauriDrop = async () => {
      try {
        if (isTauri()) {
          const { getCurrentWebview } = await import('@tauri-apps/api/webview');
          const webview = getCurrentWebview();
          unlisten = await webview.onDragDropEvent(async (event) => {
            if (event.payload.type === 'over') {
              setIsDragging(true);
            } else if (event.payload.type === 'drop') {
              setIsDragging(false);
              const paths = event.payload.paths;
              if (paths && paths.length > 0) {
                const firstPath = paths[0];
                if (firstPath.toLowerCase().endsWith('.pdf')) {
                  await processFilePath(firstPath);
                } else {
                  setMessage({
                    type: 'error',
                    text: 'Format invalide. Veuillez déposer un document PDF issu de CIRIS (Statistiques de production).',
                  });
                }
              }
            } else {
              setIsDragging(false);
            }
          });
        }
      } catch (err) {
        console.warn('Tauri onDragDropEvent listener non disponible:', err);
      }
    };

    setupTauriDrop();

    return () => {
      if (unlisten) unlisten();
    };
  }, [typeIntermediaire, tvaTaux, succursale, departement]);

  // Gestion de la sélection par fichier ou drag&drop navigateur Web
  const handleFileDrop = async (e: React.DragEvent<HTMLDivElement> | React.ChangeEvent<HTMLInputElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    let file: File | null = null;
    if ('dataTransfer' in e) {
      if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
        file = e.dataTransfer.files[0];
      }
    } else if (e.target.files && e.target.files.length > 0) {
      file = e.target.files[0];
    }

    if (!file) return;

    if (!file.name.toLowerCase().endsWith('.pdf')) {
      setMessage({ type: 'error', text: 'Veuillez sélectionner un fichier PDF issu du logiciel CIRIS.' });
      return;
    }

    setLoading(true);
    setMessage(null);

    try {
      const reader = new FileReader();
      reader.onload = async () => {
        try {
          const base64 = reader.result as string;
          const data = await api.parseCirisPdfBase64(base64);
          await applyExtractedData(data, file?.name || 'document.pdf');
        } catch (err: any) {
          setMessage({ type: 'error', text: `Erreur lors de l'analyse : ${err.message || err}` });
        } finally {
          setLoading(false);
        }
      };
      reader.readAsDataURL(file);
    } catch (err: any) {
      setMessage({ type: 'error', text: `Impossible de lire le fichier : ${err.message || err}` });
      setLoading(false);
    }
  };

  // Recalcul en direct si les paramètres changent
  const handleParamChange = async (newType: IntermediaireType, newTva: number, newSucc: string, newDept: string) => {
    setTypeIntermediaire(newType);
    setTvaTaux(newTva);
    setSuccursale(newSucc);
    setDepartement(newDept);

    if (extractedData) {
      try {
        const computed = await api.calculateBordereauFromPdf(extractedData, newType, newTva, newSucc, newDept);
        setCalculatedBordereau(computed);
      } catch (err) {
        console.error('Erreur recalcul:', err);
      }
    }
  };

  // Sauvegarde dans SQLite
  const handleSaveToDb = async () => {
    if (!calculatedBordereau) return;
    setSaving(true);
    try {
      const id = await api.saveBordereau(calculatedBordereau);
      setMessage({ type: 'success', text: `Bordereau N° ${calculatedBordereau.numero_bordereau} enregistré avec succès en base de données (ID #${id}) !` });
      if (onBordereauSaved) onBordereauSaved(id);
    } catch (err: any) {
      setMessage({ type: 'error', text: `Erreur d'enregistrement : ${err}` });
    } finally {
      setSaving(false);
    }
  };

  // Export Excel
  const handleExportExcel = async () => {
    if (!calculatedBordereau) return;
    try {
      const path = await api.exportExcel(calculatedBordereau);
      setMessage({ type: 'success', text: `Bordereau exporté en fichier Excel avec succès : ${path}` });
    } catch (err: any) {
      setMessage({ type: 'error', text: `Erreur export Excel: ${err}` });
    }
  };

  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(val) + ' DZD';
  };

  return (
    <div className="space-y-6 pb-12">
      {/* Top Banner */}
      <div className="bg-gradient-to-r from-indigo-900 via-indigo-800 to-slate-900 rounded-2xl p-6 text-white shadow-xl">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <span className="px-2 py-0.5 rounded text-[11px] font-bold uppercase bg-sky-400/20 text-sky-300 border border-sky-400/30">
                Module CIRIS - Production par Garantie
              </span>
              <span className="text-xs text-slate-300">Rapport RCCBPG 1.23.5</span>
            </div>
            <h1 className="text-2xl font-black tracking-tight">Import & Calcul Automatique des Commissions</h1>
            <p className="text-sm text-indigo-200 mt-1 max-w-2xl">
              Glissez-déposez l'état mensuel de production CIRIS. Le moteur identifie l'agence, extrait les primes nettes par garantie, applique le barème de la feuille Param et dresse le bordereau officiel au centime près.
            </p>
          </div>
          <div className="flex items-center gap-3 shrink-0">
            <button
              onClick={() => fileInputRef.current?.click()}
              disabled={loading}
              className="px-4 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-bold shadow-lg shadow-indigo-950/20 flex items-center gap-2 transition-all cursor-pointer"
            >
              <FolderOpen className="w-4 h-4" />
              <span>Sélectionner un fichier PDF</span>
            </button>
          </div>
        </div>
      </div>

      {/* Notification Message */}
      {message && (
        <div
          className={`p-4 rounded-xl text-sm flex items-center gap-3 border shadow-xs animate-in fade-in duration-200 ${
            message.type === 'success'
              ? 'bg-emerald-50 border-emerald-200 text-emerald-900'
              : 'bg-rose-50 border-rose-200 text-rose-900'
          }`}
        >
          {message.type === 'success' ? (
            <CheckCircle className="w-5 h-5 text-emerald-600 shrink-0" />
          ) : (
            <AlertCircle className="w-5 h-5 text-rose-600 shrink-0" />
          )}
          <span className="font-medium">{message.text}</span>
        </div>
      )}

      {/* Dropzone & Settings Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* PDF Dropzone */}
        <div className="lg:col-span-2">
          {/* Hidden File Input */}
          <input
            ref={fileInputRef}
            type="file"
            accept=".pdf"
            onChange={handleFileDrop}
            className="hidden"
          />

          <div
            onClick={() => fileInputRef.current?.click()}
            onDragEnter={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setIsDragging(true);
            }}
            onDragOver={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setIsDragging(true);
            }}
            onDragLeave={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setIsDragging(false);
            }}
            onDrop={handleFileDrop}
            className={`border-2 border-dashed transition-all duration-200 rounded-2xl p-8 flex flex-col items-center justify-center text-center cursor-pointer min-h-[230px] select-none group relative ${
              isDragging
                ? 'border-indigo-600 bg-indigo-100/90 ring-4 ring-indigo-500/25 scale-[1.01] shadow-lg shadow-indigo-200/50'
                : 'border-indigo-200 hover:border-indigo-400 bg-indigo-50/40 hover:bg-indigo-50/70'
            }`}
          >
            <div
              className={`w-16 h-16 rounded-2xl shadow-md flex items-center justify-center transition-all duration-200 mb-3 ${
                isDragging
                  ? 'bg-indigo-600 text-white scale-110 shadow-indigo-400'
                  : 'bg-white text-indigo-600 group-hover:scale-105'
              }`}
            >
              <UploadCloud className={`w-8 h-8 ${isDragging ? 'animate-bounce' : ''}`} />
            </div>
            <h3
              className={`text-base font-bold transition-colors duration-200 ${
                isDragging ? 'text-indigo-900 scale-105' : 'text-slate-800'
              }`}
            >
              {isDragging ? 'Relâchez votre fichier PDF ici pour le calculer !' : 'Déposez votre fichier CIRIS PDF ici'}
            </h3>
            <p className="text-xs text-slate-500 mt-1 max-w-md">
              Rapport officiel CIRIS (Statistiques de la Production Par Garantie - RCCBPG)
            </p>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                fileInputRef.current?.click();
              }}
              className="mt-4 text-xs font-bold px-5 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white transition-all cursor-pointer shadow-xs flex items-center gap-2"
            >
              <FolderOpen className="w-4 h-4" />
              <span>Parcourir les fichiers</span>
            </button>
          </div>
        </div>

        {/* Configuration Options */}
        <div className="bg-white rounded-2xl p-6 border border-slate-200 shadow-xs flex flex-col justify-between">
          <div>
            <h3 className="text-sm font-bold text-slate-800 flex items-center gap-2 mb-4">
              <Building2 className="w-4 h-4 text-indigo-600" />
              Paramètres du Bordereau
            </h3>

            <div className="space-y-3.5 text-xs">
              <div>
                <label className="block font-semibold text-slate-600 mb-1">Type d'Intermédiaire</label>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => handleParamChange('AGA', tvaTaux, succursale, departement)}
                    className={`py-2 px-3 rounded-lg font-bold border transition-all text-xs ${
                      typeIntermediaire === 'AGA'
                        ? 'bg-indigo-600 border-indigo-600 text-white shadow-xs'
                        : 'bg-slate-50 border-slate-200 text-slate-700 hover:bg-slate-100'
                    }`}
                  >
                    AGA (Apport + Gestion)
                  </button>
                  <button
                    type="button"
                    onClick={() => handleParamChange('COURTIER', tvaTaux, succursale, departement)}
                    className={`py-2 px-3 rounded-lg font-bold border transition-all text-xs ${
                      typeIntermediaire === 'COURTIER'
                        ? 'bg-indigo-600 border-indigo-600 text-white shadow-xs'
                        : 'bg-slate-50 border-slate-200 text-slate-700 hover:bg-slate-100'
                    }`}
                  >
                    Courtier (Apport seul)
                  </button>
                </div>
              </div>

              <div>
                <label className="block font-semibold text-slate-600 mb-1">Taux TVA / TAV applicable</label>
                <div className="flex items-center gap-2">
                  <select
                    value={tvaTaux}
                    onChange={(e) => handleParamChange(typeIntermediaire, parseFloat(e.target.value), succursale, departement)}
                    className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-medium text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  >
                    <option value={0.19}>19.0% (TVA Standard Algérie)</option>
                    <option value={0.09}>9.0% (Taux Réduit)</option>
                    <option value={0.10}>10.0% (TAV Spécifique)</option>
                    <option value={0.00}>Exonéré (0%)</option>
                  </select>
                </div>
              </div>

              <div>
                <label className="block font-semibold text-slate-600 mb-1">Succursale & Département</label>
                <input
                  type="text"
                  value={succursale}
                  onChange={(e) => handleParamChange(typeIntermediaire, tvaTaux, e.target.value, departement)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-medium text-slate-800 mb-2 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  placeholder="Succursale"
                />
                <input
                  type="text"
                  value={departement}
                  onChange={(e) => handleParamChange(typeIntermediaire, tvaTaux, succursale, e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-medium text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  placeholder="Département"
                />
              </div>
            </div>
          </div>

          {extractedData && (
            <div className="mt-4 pt-4 border-t border-slate-100 text-xs text-slate-500">
              <span className="font-semibold text-slate-700">Période extraite : </span>
              {extractedData.periode_debut} au {extractedData.periode_fin} (Mois {extractedData.mois} / {extractedData.exercice})
            </div>
          )}
        </div>
      </div>

      {/* Calculated Results Area */}
      {calculatedBordereau && (
        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-3 duration-300">
          {/* Summary Stat Cards */}
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
            <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs">
              <span className="text-[11px] font-semibold text-slate-500 uppercase tracking-wider">Primes Nettes</span>
              <p className="text-base font-extrabold text-slate-900 mt-1">
                {formatCurrency(calculatedBordereau.total_prime_nette)}
              </p>
              <span className="text-[10px] text-emerald-600 font-bold">100% conforme CIRIS</span>
            </div>

            <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs">
              <span className="text-[11px] font-semibold text-slate-500 uppercase tracking-wider">Commission Apport</span>
              <p className="text-base font-extrabold text-indigo-600 mt-1">
                {formatCurrency(calculatedBordereau.total_commission_apport)}
              </p>
              <span className="text-[10px] text-slate-400">Taux selon Param</span>
            </div>

            <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs">
              <span className="text-[11px] font-semibold text-slate-500 uppercase tracking-wider">Commission Gestion</span>
              <p className="text-base font-extrabold text-sky-600 mt-1">
                {formatCurrency(calculatedBordereau.total_commission_gestion)}
              </p>
              <span className="text-[10px] text-slate-400">{typeIntermediaire === 'AGA' ? 'Spécifique AGA' : '0 DZD (Courtier)'}</span>
            </div>

            <div className="bg-white p-4 rounded-xl border border-indigo-200 bg-indigo-50/20 shadow-xs">
              <span className="text-[11px] font-bold text-indigo-700 uppercase tracking-wider">Commission H.T</span>
              <p className="text-base font-extrabold text-indigo-950 mt-1">
                {formatCurrency(calculatedBordereau.total_commission_ht)}
              </p>
              <span className="text-[10px] text-indigo-600 font-semibold">Apport + Gestion</span>
            </div>

            <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs">
              <span className="text-[11px] font-semibold text-slate-500 uppercase tracking-wider">TVA ({(tvaTaux * 100).toFixed(0)}%)</span>
              <p className="text-base font-extrabold text-slate-800 mt-1">
                {formatCurrency(calculatedBordereau.total_tva)}
              </p>
              <span className="text-[10px] text-slate-400">Taux légal</span>
            </div>

            <div className="bg-gradient-to-br from-emerald-600 to-teal-700 p-4 rounded-xl text-white shadow-md shadow-emerald-900/10">
              <span className="text-[11px] font-bold text-emerald-100 uppercase tracking-wider">Commission T.T.C</span>
              <p className="text-lg font-black mt-1">
                {formatCurrency(calculatedBordereau.total_commission_ttc)}
              </p>
              <span className="text-[10px] text-emerald-200 font-semibold">Net à percevoir</span>
            </div>
          </div>

          {/* Action Bar */}
          <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs flex flex-wrap items-center justify-between gap-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg bg-indigo-50 text-indigo-600 flex items-center justify-center font-bold text-xs">
                {calculatedBordereau.mois ? String(calculatedBordereau.mois).padStart(2, '0') : '00'}
              </div>
              <div>
                <h4 className="text-sm font-bold text-slate-900">
                  {calculatedBordereau.numero_bordereau} — {calculatedBordereau.intermediaire_nom}
                </h4>
                <p className="text-xs text-slate-500">
                  Exercice {calculatedBordereau.exercice} • {calculatedBordereau.lignes.length} garanties calculées
                </p>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-2.5">
              <button
                type="button"
                onClick={handleSaveToDb}
                disabled={saving}
                className="px-4 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all"
              >
                <Save className="w-4 h-4" />
                <span>{saving ? 'Enregistrement...' : 'Enregistrer le bordereau'}</span>
              </button>

              <button
                type="button"
                onClick={handleExportExcel}
                className="px-4 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all"
              >
                <FileSpreadsheet className="w-4 h-4" />
                <span>Exporter en Excel (.xlsx)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  if (onNavigateToBordereau && calculatedBordereau) {
                    onNavigateToBordereau(calculatedBordereau);
                  } else {
                    window.print();
                  }
                }}
                className="px-4 py-2.5 rounded-xl bg-slate-800 hover:bg-slate-900 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all"
              >
                <FileText className="w-4 h-4" />
                <span>Voir Bordereau Officiel</span>
              </button>

              <button
                type="button"
                onClick={() => window.print()}
                className="px-3.5 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-2 cursor-pointer transition-all"
              >
                <Printer className="w-4 h-4" />
                <span>Imprimer</span>
              </button>
            </div>
          </div>

          {/* Detailed Accounting Table */}
          <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between">
              <h3 className="text-sm font-bold text-slate-900 flex items-center gap-2">
                <FileText className="w-4 h-4 text-indigo-600" />
                Détail des Lignes de Commissions par Garantie
              </h3>
              <span className="text-xs font-semibold text-slate-500">
                Format officiel conforme au fichier Excel de référence
              </span>
            </div>

            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead>
                  <tr className="bg-slate-50 border-b border-slate-200 text-slate-700 font-bold uppercase tracking-wider">
                    <th className="px-4 py-3">Code</th>
                    <th className="px-4 py-3">Branche / Garantie</th>
                    <th className="px-4 py-3 text-right">Prime Nette</th>
                    <th className="px-4 py-3 text-center">Taux Apport</th>
                    {typeIntermediaire === 'AGA' && <th className="px-4 py-3 text-center">Taux Gestion</th>}
                    <th className="px-4 py-3 text-right">Com. Apport</th>
                    {typeIntermediaire === 'AGA' && <th className="px-4 py-3 text-right">Com. Gestion</th>}
                    <th className="px-4 py-3 text-right">Total H.T</th>
                    <th className="px-4 py-3 text-right">TVA</th>
                    <th className="px-4 py-3 text-right">Total T.T.C</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {calculatedBordereau.lignes.map((l, index) => (
                    <tr key={index} className="hover:bg-indigo-50/30 transition-colors">
                      <td className="px-4 py-2.5 font-mono font-bold text-indigo-600">{l.code_garantie}</td>
                      <td className="px-4 py-2.5">
                        <div className="font-semibold text-slate-900">{l.libelle_garantie}</div>
                        <div className="text-[10px] text-slate-400">{l.branche_nom}</div>
                      </td>
                      <td className="px-4 py-2.5 text-right font-semibold font-mono text-slate-800">
                        {l.prime_nette.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="px-4 py-2.5 text-center font-bold text-indigo-700">
                        {(l.taux_apport * 100).toFixed(1)}%
                      </td>
                      {typeIntermediaire === 'AGA' && (
                        <td className="px-4 py-2.5 text-center font-bold text-sky-700">
                          {(l.taux_gestion * 100).toFixed(1)}%
                        </td>
                      )}
                      <td className="px-4 py-2.5 text-right font-mono text-indigo-900 font-semibold">
                        {l.montant_apport.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      {typeIntermediaire === 'AGA' && (
                        <td className="px-4 py-2.5 text-right font-mono text-sky-900 font-semibold">
                          {l.montant_gestion.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                        </td>
                      )}
                      <td className="px-4 py-2.5 text-right font-mono font-bold text-slate-950">
                        {l.montant_ht.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="px-4 py-2.5 text-right font-mono text-slate-500">
                        {l.montant_tva.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="px-4 py-2.5 text-right font-mono font-extrabold text-emerald-800">
                        {l.montant_ttc.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr className="bg-slate-100 border-t-2 border-slate-300 font-black text-slate-900">
                    <td colSpan={2} className="px-4 py-3 uppercase tracking-wider">
                      Total Général ({typeIntermediaire})
                    </td>
                    <td className="px-4 py-3 text-right font-mono">
                      {calculatedBordereau.total_prime_nette.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="px-4 py-3 text-center">-</td>
                    {typeIntermediaire === 'AGA' && <td className="px-4 py-3 text-center">-</td>}
                    <td className="px-4 py-3 text-right font-mono text-indigo-900">
                      {calculatedBordereau.total_commission_apport.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    {typeIntermediaire === 'AGA' && (
                      <td className="px-4 py-3 text-right font-mono text-sky-900">
                        {calculatedBordereau.total_commission_gestion.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                    )}
                    <td className="px-4 py-3 text-right font-mono text-indigo-950">
                      {calculatedBordereau.total_commission_ht.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="px-4 py-3 text-right font-mono">
                      {calculatedBordereau.total_tva.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="px-4 py-3 text-right font-mono text-emerald-900 text-sm">
                      {calculatedBordereau.total_commission_ttc.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })} DZD
                    </td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
