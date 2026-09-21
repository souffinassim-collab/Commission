import React from 'react';
import { 
  FileText, 
  BarChart3, 
  Sliders, 
  Users, 
  Database, 
  FileSpreadsheet, 
  CheckCircle2,
  HardDrive
} from 'lucide-react';

interface NavbarProps {
  activeTab: string;
  setActiveTab: (tab: string) => void;
  dbInfo: { version: number; total_branches: number; last_backup?: string } | null;
}

export const Navbar: React.FC<NavbarProps> = ({ activeTab, setActiveTab, dbInfo }) => {
  const navItems = [
    { id: 'dashboard', label: 'Tableau de Bord', icon: BarChart3 },
    { id: 'import', label: 'Import PDF Ciris', icon: FileSpreadsheet, badge: 'Nouveau' },
    { id: 'bordereaux', label: 'Bordereaux & Factures', icon: FileText },
    { id: 'parametres', label: 'Taux & Branches', icon: Sliders },
    { id: 'intermediaires', label: 'AGA & Courtiers', icon: Users },
    { id: 'bdd', label: 'Base de Données', icon: Database },
  ];

  return (
    <header className="bg-white border-b border-slate-200 sticky top-0 z-50 shadow-xs no-print">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Brand Logo & Name */}
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-tr from-indigo-600 to-sky-500 flex items-center justify-center text-white shadow-md shadow-indigo-100">
              <FileSpreadsheet className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="font-bold text-slate-900 text-base tracking-tight">Commissions Assurances</span>
                <span className="text-[10px] uppercase font-extrabold px-1.5 py-0.5 rounded bg-indigo-50 text-indigo-700 border border-indigo-200/60">
                  AGA & Courtier
                </span>
              </div>
              <p className="text-xs text-slate-500">Moteur de calcul & Rapprochement CIRIS</p>
            </div>
          </div>

          {/* Navigation Items */}
          <nav className="flex items-center gap-1">
            {navItems.map((item) => {
              const Icon = item.icon;
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id)}
                  className={`flex items-center gap-2 px-3.5 py-2 rounded-lg text-xs font-semibold transition-all duration-150 relative ${
                    isActive
                      ? 'bg-indigo-600 text-white shadow-sm shadow-indigo-200'
                      : 'text-slate-600 hover:text-slate-900 hover:bg-slate-100/80'
                  }`}
                >
                  <Icon className="w-4 h-4" />
                  <span>{item.label}</span>
                  {item.badge && (
                    <span className={`text-[9px] px-1.5 py-0.2 rounded-full font-bold uppercase ${
                      isActive ? 'bg-white/20 text-white' : 'bg-sky-100 text-sky-700'
                    }`}>
                      {item.badge}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>

          {/* Portable SQLite DB Badge */}
          <div className="hidden lg:flex items-center gap-2 pl-3 border-l border-slate-200">
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-emerald-50 border border-emerald-200/80 text-emerald-800 text-xs font-medium">
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-600" />
              <div className="flex flex-col">
                <span className="text-[10px] leading-none text-emerald-600 font-semibold">BDD Locale</span>
                <span className="text-[11px] leading-tight font-bold">./BDD/commissions.db</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
};
