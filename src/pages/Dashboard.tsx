import React, { useEffect, useState } from 'react';
import { 
  FileSpreadsheet, 
  FileText, 
  Users, 
  Sliders, 
  Database, 
  ArrowRight, 
  TrendingUp, 
  ShieldAlert, 
  Coins, 
  Clock,
  Sparkles
} from 'lucide-react';
import { api } from '../lib/api';
import { Bordereau } from '../types';

interface DashboardProps {
  onNavigate: (tab: string) => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ onNavigate }) => {
  const [stats, setStats] = useState({
    totalBordereaux: 0,
    totalCommissionsTtc: 0,
    totalBranches: 70,
    totalIntermediaires: 0,
  });
  const [recentBordereaux, setRecentBordereaux] = useState<Bordereau[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [bList, interList, branches] = await Promise.all([
        api.getBordereaux(),
        api.getIntermediaires(),
        api.getBranches(),
      ]);

      const totTtc = bList.reduce((acc, curr) => acc + curr.total_commission_ttc, 0);

      setStats({
        totalBordereaux: bList.length,
        totalCommissionsTtc: totTtc,
        totalBranches: branches.length || 70,
        totalIntermediaires: interList.length,
      });

      setRecentBordereaux(bList.slice(0, 5));
    } catch (err) {
      console.error('Erreur chargement dashboard:', err);
    } finally {
      setLoading(false);
    }
  };

  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(val) + ' DZD';
  };

  return (
    <div className="space-y-6 pb-12">
      {/* Welcome Hero Banner */}
      <div className="bg-gradient-to-r from-indigo-900 via-indigo-800 to-slate-900 rounded-2xl p-8 text-white shadow-xl relative overflow-hidden">
        <div className="relative z-10 max-w-2xl">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-white/10 text-white text-xs font-semibold mb-3 border border-white/10">
            <Sparkles className="w-3.5 h-3.5 text-amber-300" />
            <span>Application Portable • Base de Données Intégrée</span>
          </div>
          <h1 className="text-3xl font-black tracking-tight leading-tight">
            Gestion & Calcul des Commissions d'Assurance
          </h1>
          <p className="text-sm text-indigo-200 mt-2 leading-relaxed">
            Automatisez le traitement des bordereaux de commissions pour Agents Généraux (AGA) et Courtiers. Rapprochement direct avec les rapports de production CIRIS.
          </p>

          <div className="mt-6 flex flex-wrap items-center gap-3">
            <button
              onClick={() => onNavigate('import')}
              className="px-5 py-2.5 rounded-xl bg-gradient-to-r from-indigo-500 to-indigo-600 hover:from-indigo-600 hover:to-indigo-700 text-white text-xs font-bold shadow-lg shadow-indigo-950/20 flex items-center gap-2 transition-all cursor-pointer"
            >
              <FileSpreadsheet className="w-4 h-4" />
              <span>Importer un fichier CIRIS PDF</span>
            </button>

            <button
              onClick={() => onNavigate('bordereaux')}
              className="px-4 py-2.5 rounded-xl bg-white/10 hover:bg-white/15 text-white text-xs font-bold border border-white/20 transition-all cursor-pointer flex items-center gap-2"
            >
              <FileText className="w-4 h-4" />
              <span>Consulter les Bordereaux</span>
            </button>
          </div>
        </div>
      </div>

      {/* Stats KPI */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-5">
        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Total Commissions TTC</span>
            <div className="w-8 h-8 rounded-lg bg-emerald-50 text-emerald-600 flex items-center justify-center">
              <Coins className="w-4 h-4" />
            </div>
          </div>
          <p className="text-xl font-extrabold text-slate-900 mt-2">
            {formatCurrency(stats.totalCommissionsTtc)}
          </p>
          <span className="text-[11px] text-slate-400 mt-1 block">Cumul des bordereaux enregistrés</span>
        </div>

        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Bordereaux Enregistrés</span>
            <div className="w-8 h-8 rounded-lg bg-indigo-50 text-indigo-600 flex items-center justify-center">
              <FileText className="w-4 h-4" />
            </div>
          </div>
          <p className="text-xl font-extrabold text-slate-900 mt-2">{stats.totalBordereaux}</p>
          <span className="text-[11px] text-slate-400 mt-1 block">Archivés en base de données</span>
        </div>

        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Produits / Branches</span>
            <div className="w-8 h-8 rounded-lg bg-sky-50 text-sky-600 flex items-center justify-center">
              <Sliders className="w-4 h-4" />
            </div>
          </div>
          <p className="text-xl font-extrabold text-slate-900 mt-2">{stats.totalBranches}</p>
          <span className="text-[11px] text-slate-400 mt-1 block">Taux barème (Param)</span>
        </div>

        <div className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">Intermédiaires</span>
            <div className="w-8 h-8 rounded-lg bg-amber-50 text-amber-600 flex items-center justify-center">
              <Users className="w-4 h-4" />
            </div>
          </div>
          <p className="text-xl font-extrabold text-slate-900 mt-2">{stats.totalIntermediaires}</p>
          <span className="text-[11px] text-slate-400 mt-1 block">AGA & Courtiers répertoriés</span>
        </div>
      </div>

      {/* Quick Access Feature Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div
          onClick={() => onNavigate('import')}
          className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs hover:border-indigo-300 hover:shadow-md transition-all cursor-pointer group flex flex-col justify-between"
        >
          <div>
            <div className="w-12 h-12 rounded-xl bg-indigo-50 text-indigo-600 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
              <FileSpreadsheet className="w-6 h-6" />
            </div>
            <h3 className="text-base font-bold text-slate-900 group-hover:text-indigo-600 transition-colors">
              Traitement des Rapports CIRIS
            </h3>
            <p className="text-xs text-slate-500 mt-1 leading-relaxed">
              Importez vos fichiers PDF de production mensuelle. Le système extrait les chiffres et calcule les commissions automatiquement.
            </p>
          </div>
          <div className="mt-4 flex items-center gap-1.5 text-xs font-bold text-indigo-600">
            <span>Ouvrir l'importateur</span>
            <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
          </div>
        </div>

        <div
          onClick={() => onNavigate('parametres')}
          className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs hover:border-sky-300 hover:shadow-md transition-all cursor-pointer group flex flex-col justify-between"
        >
          <div>
            <div className="w-12 h-12 rounded-xl bg-sky-50 text-sky-600 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
              <Sliders className="w-6 h-6" />
            </div>
            <h3 className="text-base font-bold text-slate-900 group-hover:text-sky-600 transition-colors">
              Barème & Taux de Commission
            </h3>
            <p className="text-xs text-slate-500 mt-1 leading-relaxed">
              Consultez et ajustez les pourcentages d'apport (Courtier / AGA) et de gestion (AGA) pour chaque branche d'assurance.
            </p>
          </div>
          <div className="mt-4 flex items-center gap-1.5 text-xs font-bold text-sky-600">
            <span>Consulter le barème</span>
            <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
          </div>
        </div>

        <div
          onClick={() => onNavigate('bdd')}
          className="bg-white p-6 rounded-2xl border border-slate-200 shadow-xs hover:border-emerald-300 hover:shadow-md transition-all cursor-pointer group flex flex-col justify-between"
        >
          <div>
            <div className="w-12 h-12 rounded-xl bg-emerald-50 text-emerald-600 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
              <Database className="w-6 h-6" />
            </div>
            <h3 className="text-base font-bold text-slate-900 group-hover:text-emerald-600 transition-colors">
              Base de Données & Sauvegardes
            </h3>
            <p className="text-xs text-slate-500 mt-1 leading-relaxed">
              Base locale dans le sous-dossier ./BDD/ avec gestionnaire de sauvegardes et protection des données.
            </p>
          </div>
          <div className="mt-4 flex items-center gap-1.5 text-xs font-bold text-emerald-600">
            <span>Gérer les sauvegardes</span>
            <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
          </div>
        </div>
      </div>
    </div>
  );
};
