import React, { useState, useEffect, useRef } from 'react';
import { Navbar } from './components/Navbar';
import { Dashboard } from './pages/Dashboard';
import { ImportCirisPage } from './pages/ImportCirisPage';
import { BordereauxPage } from './pages/BordereauxPage';
import { ParametresPage } from './pages/ParametresPage';
import { IntermediairesPage } from './pages/IntermediairesPage';
import { DatabasePage } from './pages/DatabasePage';
import { api } from './lib/api';
import { ConfirmDialog, ConfirmState } from './components/Shared';

export function App() {
  const [activeTab, setActiveTab] = useState<string>('dashboard');
  const [dbInfo, setDbInfo] = useState<any>(null);
  const importDirtyRef = useRef<boolean>(false);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);

  // Quitter la page d'import avec un import non enregistré l'annulerait : on prévient l'utilisateur
  const navigate = (tab: string) => {
    if (tab === activeTab) return;
    if (activeTab === 'import' && importDirtyRef.current) {
      setConfirm({
        title: "Quitter l'import en cours ?",
        danger: true,
        message: (
          <>
            <p>Un import est en cours et son calcul n'est pas enregistré.</p>
            <p>
              Quitter cette page <strong>annulera l'import en cours</strong> : les fichiers importés et le calcul seront abandonnés.
            </p>
            <p className="font-bold">Voulez-vous annuler l'import et quitter la page ?</p>
          </>
        ),
        confirmLabel: "Oui, annuler l'import",
        cancelLabel: "Non, rester sur l'import",
        onConfirm: () => {
          importDirtyRef.current = false;
          setActiveTab(tab);
        },
      });
      return;
    }
    setActiveTab(tab);
  };

  useEffect(() => {
    api.getDbInfo().then((info) => setDbInfo(info)).catch(console.error);
  }, []);

  return (
    <div className="min-h-screen bg-slate-50 flex flex-col selection:bg-indigo-500 selection:text-white">
      {/* Header Navigation */}
      <Navbar activeTab={activeTab} setActiveTab={navigate} dbInfo={dbInfo} />

      {/* Main Content Area */}
      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 pt-6">
        {activeTab === 'dashboard' && <Dashboard onNavigate={navigate} />}
        {activeTab === 'import' && (
          <ImportCirisPage
            onBordereauSaved={() => {
              api.getDbInfo().then(setDbInfo).catch(console.error);
            }}
            onDirtyChange={(d) => {
              importDirtyRef.current = d;
            }}
          />
        )}
        {activeTab === 'bordereaux' && <BordereauxPage />}
        {activeTab === 'parametres' && <ParametresPage />}
        {activeTab === 'intermediaires' && <IntermediairesPage />}
        {activeTab === 'bdd' && <DatabasePage />}
      </main>
      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />

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
