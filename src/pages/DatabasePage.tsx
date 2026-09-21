import React, { useState, useEffect } from 'react';
import { 
  Database, 
  HardDrive, 
  ShieldCheck, 
  Save, 
  RefreshCw, 
  CheckCircle2, 
  FolderLock, 
  Layers,
  History,
  AlertCircle
} from 'lucide-react';
import { api } from '../lib/api';

export const DatabasePage: React.FC = () => {
  const [dbInfo, setDbInfo] = useState<any>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const [backingUp, setBackingUp] = useState<boolean>(false);

  useEffect(() => {
    loadInfo();
  }, []);

  const loadInfo = async () => {
    setLoading(true);
    try {
      const info = await api.getDbInfo();
      setDbInfo(info);
    } catch (err) {
      console.error('Erreur chargement info BDD:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleBackupNow = async () => {
    setBackingUp(true);
    setBackupMsg(null);
    try {
      const path = await api.backupDatabase();
      setBackupMsg(`Sauvegarde effectuée avec succès dans : ${path}`);
      await loadInfo();
    } catch (err: any) {
      alert('Erreur sauvegarde: ' + err);
    } finally {
      setBackingUp(false);
    }
  };

  return (
    <div className="space-y-6 pb-12">
      <div>
        <h1 className="text-2xl font-black text-slate-900 tracking-tight flex items-center gap-2">
          <Database className="w-6 h-6 text-indigo-600" />
          Base de Données & Gestion des Sauvegardes
        </h1>
        <p className="text-xs text-slate-500 mt-0.5">
          Dossier de stockage local sécurisé <span className="font-mono font-bold text-slate-700">./BDD/</span> avec protection continue contre la perte de données.
        </p>
      </div>

      {backupMsg && (
        <div className="p-4 rounded-xl bg-emerald-50 border border-emerald-200 text-emerald-900 text-xs font-semibold flex items-center gap-2 animate-in fade-in">
          <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
          <span>{backupMsg}</span>
        </div>
      )}

      {/* Main Info Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-5">
        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="w-10 h-10 rounded-xl bg-indigo-50 text-indigo-600 flex items-center justify-center mb-4">
            <HardDrive className="w-5 h-5" />
          </div>
          <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Emplacement Portable</span>
          <p className="text-sm font-black font-mono text-slate-900 mt-1 break-all">
            {dbInfo?.db_path || './BDD/commissions.db'}
          </p>
          <div className="mt-3 flex items-center gap-1.5 text-xs text-emerald-600 font-bold">
            <CheckCircle2 className="w-3.5 h-3.5" />
            <span>Transportable sur clé USB sans dépendance</span>
          </div>
        </div>

        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="w-10 h-10 rounded-xl bg-sky-50 text-sky-600 flex items-center justify-center mb-4">
            <Layers className="w-5 h-5" />
          </div>
          <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Version de la Base</span>
          <p className="text-2xl font-black text-slate-900 mt-1">
            Version {dbInfo?.version || 1}
          </p>
          <div className="mt-3 flex items-center gap-1.5 text-xs text-slate-500">
            <ShieldCheck className="w-3.5 h-3.5 text-sky-600" />
            <span>Mises à jour automatiques transparentes</span>
          </div>
        </div>

        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs flex flex-col justify-between">
          <div>
            <div className="w-10 h-10 rounded-xl bg-emerald-50 text-emerald-600 flex items-center justify-center mb-4">
              <History className="w-5 h-5" />
            </div>
            <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Dernière Sauvegarde</span>
            <p className="text-xs font-bold font-mono text-slate-800 mt-1">
              {dbInfo?.last_backup || 'Sauvegarde initiale automatique'}
            </p>
          </div>

          <button
            onClick={handleBackupNow}
            disabled={backingUp}
            className="mt-4 w-full py-2 px-3 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-xs flex items-center justify-center gap-2 cursor-pointer transition-all"
          >
            <Save className="w-4 h-4" />
            <span>{backingUp ? 'Sauvegarde en cours...' : 'Créer une sauvegarde maintenant'}</span>
          </button>
        </div>
      </div>

      {/* Explanatory Box on Migration Safety */}
      <div className="bg-gradient-to-br from-slate-900 to-indigo-950 text-white p-6 rounded-2xl shadow-md space-y-4">
        <h3 className="text-base font-bold flex items-center gap-2 text-indigo-200">
          <ShieldCheck className="w-5 h-5 text-emerald-400" />
          Protection & Préservation des Données
        </h3>
        <p className="text-xs text-slate-300 leading-relaxed max-w-3xl">
          À chaque mise à jour du logiciel, vos données sont automatiquement protégées et préservées :
        </p>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs">
          <div className="p-4 rounded-xl bg-white/5 border border-white/10">
            <span className="font-bold text-sky-300 block mb-1">1. Sauvegarde Automatique Préalable</span>
            <span className="text-slate-300">
              Avant toute modification, une copie de sécurité exacte horodatée est archivée dans <code className="text-amber-300 font-mono">./BDD/backups/</code>.
            </span>
          </div>
          <div className="p-4 rounded-xl bg-white/5 border border-white/10">
            <span className="font-bold text-sky-300 block mb-1">2. Suivi Automatique des Versions</span>
            <span className="text-slate-300">
              Le système garde l'historique et applique uniquement les améliorations nécessaires en toute sécurité.
            </span>
          </div>
          <div className="p-4 rounded-xl bg-white/5 border border-white/10">
            <span className="font-bold text-sky-300 block mb-1">3. Protection Garantie</span>
            <span className="text-slate-300">
              En cas d'anomalie ou d'interruption, l'opération est immédiatement annulée pour laisser vos données intactes.
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
