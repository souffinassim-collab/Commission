import React, { useState, useEffect } from 'react';
import { Navbar } from './components/Navbar';
import { Dashboard } from './pages/Dashboard';
import { ImportCirisPage } from './pages/ImportCirisPage';
import { BordereauxPage } from './pages/BordereauxPage';
import { ParametresPage } from './pages/ParametresPage';
import { IntermediairesPage } from './pages/IntermediairesPage';
import { DatabasePage } from './pages/DatabasePage';
import { api } from './lib/api';

export function App() {
  const [activeTab, setActiveTab] = useState<string>('dashboard');
  const [dbInfo, setDbInfo] = useState<any>(null);

  useEffect(() => {
    api.getDbInfo().then((info) => setDbInfo(info)).catch(console.error);
  }, []);

  return (
    <div className="min-h-screen bg-slate-50 flex flex-col selection:bg-indigo-500 selection:text-white">
      {/* Header Navigation */}
      <Navbar activeTab={activeTab} setActiveTab={setActiveTab} dbInfo={dbInfo} />

      {/* Main Content Area */}
      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 pt-6">
        {activeTab === 'dashboard' && <Dashboard onNavigate={setActiveTab} />}
        {activeTab === 'import' && (
          <ImportCirisPage
            onBordereauSaved={() => {
              api.getDbInfo().then(setDbInfo);
            }}
          />
        )}
        {activeTab === 'bordereaux' && <BordereauxPage />}
        {activeTab === 'parametres' && <ParametresPage />}
        {activeTab === 'intermediaires' && <IntermediairesPage />}
        {activeTab === 'bdd' && <DatabasePage />}
      </main>

      {/* Footer (Hidden on print) */}
      <footer className="bg-white border-t border-slate-200 py-4 mt-auto no-print">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between text-xs text-slate-500 gap-2">
          <div>
            <span className="font-bold text-slate-700">Gestion des Commissions d'Assurance</span> — Version 1.0 Portable
          </div>
          <div className="flex items-center gap-4 text-[11px]">
            <span>Base de données locale (BDD) : <strong className="text-slate-700">./BDD/commissions.db</strong></span>
            <span>•</span>
            <span>Rapports CIRIS (INFOEL S.A.)</span>
          </div>
        </div>
      </footer>
    </div>
  );
}

export default App;
