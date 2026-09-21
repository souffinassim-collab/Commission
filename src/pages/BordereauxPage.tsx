import React, { useState, useEffect } from 'react';
import { 
  FileText, 
  Printer, 
  FileSpreadsheet, 
  Trash2, 
  Eye, 
  Search, 
  Filter, 
  ArrowLeft,
  Calendar,
  CheckCircle2,
  Building,
  DollarSign
} from 'lucide-react';
import { Bordereau } from '../types';
import { api } from '../lib/api';

export const BordereauxPage: React.FC = () => {
  const [bordereaux, setBordereaux] = useState<Bordereau[]>([]);
  const [selectedBordereau, setSelectedBordereau] = useState<Bordereau | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [filterType, setFilterType] = useState<string>('ALL');

  useEffect(() => {
    loadBordereaux();
  }, []);

  const loadBordereaux = async () => {
    setLoading(true);
    try {
      const list = await api.getBordereaux();
      setBordereaux(list);
    } catch (err) {
      console.error('Erreur chargement bordereaux:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleViewBordereau = async (id: number) => {
    try {
      const full = await api.getBordereau(id);
      if (full) setSelectedBordereau(full);
    } catch (err) {
      console.error('Erreur chargement détails:', err);
    }
  };

  const handleDelete = async (id: number) => {
    if (window.confirm('Êtes-vous sûr de vouloir supprimer ce bordereau de la base de données ?')) {
      try {
        await api.deleteBordereau(id);
        if (selectedBordereau?.id === id) setSelectedBordereau(null);
        await loadBordereaux();
      } catch (err) {
        alert('Erreur suppression: ' + err);
      }
    }
  };

  const handleExportExcel = async (b: Bordereau) => {
    try {
      const path = await api.exportExcel(b);
      alert(`Bordereau exporté avec succès vers :\n${path}`);
    } catch (err) {
      alert('Erreur export: ' + err);
    }
  };

  const filtered = bordereaux.filter((b) => {
    const matchSearch =
      b.numero_bordereau.toLowerCase().includes(searchTerm.toLowerCase()) ||
      b.intermediaire_nom.toLowerCase().includes(searchTerm.toLowerCase()) ||
      b.intermediaire_code.toLowerCase().includes(searchTerm.toLowerCase());
    const matchType = filterType === 'ALL' || b.type_intermediaire === filterType;
    return matchSearch && matchType;
  });

  const formatCurrency = (v: number) =>
    new Intl.NumberFormat('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(v) + ' DZD';

  return (
    <div className="space-y-6 pb-12">
      {/* If a Bordereau is selected for view / print */}
      {selectedBordereau ? (
        <div className="space-y-6">
          {/* Top Back & Action Bar (Hidden on print) */}
          <div className="flex items-center justify-between bg-white p-4 rounded-xl border border-slate-200 shadow-xs no-print">
            <button
              onClick={() => setSelectedBordereau(null)}
              className="px-3.5 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-2 cursor-pointer transition-all"
            >
              <ArrowLeft className="w-4 h-4" />
              <span>Retour à la liste</span>
            </button>

            <div className="flex items-center gap-3">
              <button
                onClick={() => handleExportExcel(selectedBordereau)}
                className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all"
              >
                <FileSpreadsheet className="w-4 h-4" />
                <span>Télécharger Excel (.xlsx)</span>
              </button>

              <button
                onClick={() => window.print()}
                className="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all"
              >
                <Printer className="w-4 h-4" />
                <span>Imprimer / Exporter PDF</span>
              </button>
            </div>
          </div>

          {/* Official Printable Sheet (Matches Exact Excel Layout) */}
          <div className="bg-white p-8 md:p-12 rounded-2xl border border-slate-200 shadow-sm print:shadow-none print:border-none print:p-0 max-w-5xl mx-auto">
            {/* Header / Succursale */}
            <div className="border-b-2 border-slate-900 pb-4 mb-6">
              <div className="flex justify-between items-start">
                <div>
                  <h2 className="text-xl font-black tracking-tight text-slate-900 uppercase">
                    {selectedBordereau.succursale || 'SUCCURSALE SUD'}
                  </h2>
                  <p className="text-sm font-semibold text-slate-600">
                    {selectedBordereau.departement || 'Département Commercial'}
                  </p>
                </div>
                <div className="text-right">
                  <span className="text-xs font-bold text-slate-400 uppercase tracking-wider">État Officiel</span>
                  <div className="text-base font-black text-indigo-900">
                    {selectedBordereau.numero_bordereau}
                  </div>
                  <div className="text-xs text-slate-500">Date : {selectedBordereau.date_generation}</div>
                </div>
              </div>

              {/* Sheet Title */}
              <div className="mt-4 py-2 bg-slate-100 text-center rounded border border-slate-300">
                <h1 className="text-lg font-black text-slate-900 uppercase tracking-wide">
                  {selectedBordereau.type_intermediaire === 'COURTIER' ? 'Commission COURTIER' : 'Commission AGA'}
                </h1>
              </div>

              {/* Beneficiary Meta Info */}
              <div className="grid grid-cols-2 gap-4 mt-4 text-xs">
                <div className="space-y-1">
                  <p>
                    <span className="font-bold text-slate-700">
                      {selectedBordereau.type_intermediaire === 'COURTIER' ? 'COURTIER :' : 'AGA :'}
                    </span>{' '}
                    <span className="font-semibold text-slate-900">{selectedBordereau.intermediaire_nom}</span>
                  </p>
                  <p>
                    <span className="font-bold text-slate-700">Code :</span>{' '}
                    <span className="font-semibold text-slate-900">{selectedBordereau.intermediaire_code}</span>
                  </p>
                  {selectedBordereau.type_intermediaire === 'COURTIER' && selectedBordereau.code_client && (
                    <p>
                      <span className="font-bold text-slate-700">Code Client :</span>{' '}
                      <span className="font-semibold text-slate-900">{selectedBordereau.code_client}</span>
                    </p>
                  )}
                </div>
                <div className="space-y-1 text-right">
                  {selectedBordereau.mois && (
                    <p>
                      <span className="font-bold text-slate-700">Mois :</span>{' '}
                      <span className="font-semibold text-slate-900">
                        {String(selectedBordereau.mois).padStart(2, '0')}
                      </span>
                    </p>
                  )}
                  <p>
                    <span className="font-bold text-slate-700">EXERCICE :</span>{' '}
                    <span className="font-semibold text-slate-900">{selectedBordereau.exercice}</span>
                  </p>
                </div>
              </div>
            </div>

            {/* Official Table */}
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs border border-slate-300">
                <thead>
                  <tr className="bg-slate-200 border-b border-slate-300 text-slate-900 font-bold uppercase">
                    <th className="p-2 border-r border-slate-300">Branche / Garantie</th>
                    <th className="p-2 border-r border-slate-300 text-right">Prime net</th>
                    <th className="p-2 border-r border-slate-300 text-center">
                      {selectedBordereau.type_intermediaire === 'COURTIER' ? "Taux d'apport" : 'Apport'}
                    </th>
                    {selectedBordereau.type_intermediaire === 'AGA' && (
                      <th className="p-2 border-r border-slate-300 text-center">Gestion</th>
                    )}
                    <th className="p-2 border-r border-slate-300 text-right">Commission H.T</th>
                    <th className="p-2 border-r border-slate-300 text-right">TVA</th>
                    <th className="p-2 text-right">Commission T.T.C</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200">
                  {selectedBordereau.lignes.map((l, idx) => (
                    <tr key={idx} className="hover:bg-slate-50">
                      <td className="p-2 border-r border-slate-200">
                        <span className="font-bold">{l.code_garantie}</span> - {l.libelle_garantie}
                      </td>
                      <td className="p-2 border-r border-slate-200 text-right font-mono font-medium">
                        {l.prime_nette.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="p-2 border-r border-slate-200 text-right font-mono">
                        {selectedBordereau.type_intermediaire === 'COURTIER'
                          ? `${(l.taux_apport * 100).toFixed(1)}%`
                          : l.montant_apport.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      {selectedBordereau.type_intermediaire === 'AGA' && (
                        <td className="p-2 border-r border-slate-200 text-right font-mono">
                          {l.montant_gestion.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                        </td>
                      )}
                      <td className="p-2 border-r border-slate-200 text-right font-mono font-bold">
                        {l.montant_ht.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="p-2 border-r border-slate-200 text-right font-mono">
                        {l.montant_tva.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                      <td className="p-2 text-right font-mono font-bold">
                        {l.montant_ttc.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr className="bg-slate-200 border-t-2 border-slate-400 font-black text-slate-900">
                    <td className="p-2.5 border-r border-slate-300 uppercase">Total Général</td>
                    <td className="p-2.5 border-r border-slate-300 text-right font-mono">
                      {selectedBordereau.total_prime_nette.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="p-2.5 border-r border-slate-300 text-right font-mono">
                      {selectedBordereau.total_commission_apport.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    {selectedBordereau.type_intermediaire === 'AGA' && (
                      <td className="p-2.5 border-r border-slate-300 text-right font-mono">
                        {selectedBordereau.total_commission_gestion.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                      </td>
                    )}
                    <td className="p-2.5 border-r border-slate-300 text-right font-mono">
                      {selectedBordereau.total_commission_ht.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="p-2.5 border-r border-slate-300 text-right font-mono">
                      {selectedBordereau.total_tva.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                    </td>
                    <td className="p-2.5 text-right font-mono text-sm">
                      {selectedBordereau.total_commission_ttc.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })} DZD
                    </td>
                  </tr>
                </tfoot>
              </table>
            </div>

            {/* Signature Block for Print */}
            <div className="grid grid-cols-3 gap-8 mt-12 pt-8 text-xs text-center border-t border-slate-200">
              <div>
                <p className="font-bold text-slate-700 uppercase">Le Chef de Service</p>
                <div className="h-16"></div>
                <p className="text-slate-400 italic">Signature & Cachet</p>
              </div>
              <div>
                <p className="font-bold text-slate-700 uppercase">Le Directeur Commercial</p>
                <div className="h-16"></div>
                <p className="text-slate-400 italic">Signature & Cachet</p>
              </div>
              <div>
                <p className="font-bold text-slate-700 uppercase">L'Intermédiaire</p>
                <div className="h-16"></div>
                <p className="text-slate-400 italic">{selectedBordereau.intermediaire_nom}</p>
              </div>
            </div>
          </div>
        </div>
      ) : (
        /* List View */
        <div className="space-y-6">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <h1 className="text-2xl font-black text-slate-900 tracking-tight">Historique des Bordereaux</h1>
              <p className="text-xs text-slate-500 mt-0.5">
                Consultez, rééditez ou exportez les bordereaux sauvegardés en base de données.
              </p>
            </div>

            {/* Search & Filter */}
            <div className="flex items-center gap-3">
              <div className="relative">
                <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
                <input
                  type="text"
                  placeholder="Rechercher numéro, nom, code..."
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                  className="pl-9 pr-4 py-2 bg-white border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500 w-64"
                />
              </div>

              <select
                value={filterType}
                onChange={(e) => setFilterType(e.target.value)}
                className="bg-white border border-slate-200 rounded-lg px-3 py-2 text-xs font-semibold text-slate-700 focus:outline-hidden"
              >
                <option value="ALL">Tous les types</option>
                <option value="AGA">AGA uniquement</option>
                <option value="COURTIER">Courtiers uniquement</option>
              </select>
            </div>
          </div>

          {/* Table of Saved Bordereaux */}
          <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead>
                  <tr className="bg-slate-50 border-b border-slate-200 text-slate-600 font-bold uppercase tracking-wider">
                    <th className="px-4 py-3">Numéro</th>
                    <th className="px-4 py-3">Bénéficiaire</th>
                    <th className="px-4 py-3">Type</th>
                    <th className="px-4 py-3 text-center">Période</th>
                    <th className="px-4 py-3 text-right">Prime Nette</th>
                    <th className="px-4 py-3 text-right">Commission HT</th>
                    <th className="px-4 py-3 text-right">Commission TTC</th>
                    <th className="px-4 py-3 text-center">Actions</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {loading ? (
                    <tr>
                      <td colSpan={8} className="text-center py-8 text-slate-400">
                        Chargement des bordereaux...
                      </td>
                    </tr>
                  ) : filtered.length === 0 ? (
                    <tr>
                      <td colSpan={8} className="text-center py-12 text-slate-400">
                        <FileText className="w-8 h-8 mx-auto text-slate-300 mb-2" />
                        Aucun bordereau enregistré pour le moment.
                      </td>
                    </tr>
                  ) : (
                    filtered.map((b) => (
                      <tr key={b.id} className="hover:bg-slate-50/80 transition-colors">
                        <td className="px-4 py-3 font-mono font-bold text-indigo-600">{b.numero_bordereau}</td>
                        <td className="px-4 py-3">
                          <div className="font-bold text-slate-900">{b.intermediaire_nom}</div>
                          <div className="text-[10px] text-slate-400 font-mono">Code : {b.intermediaire_code}</div>
                        </td>
                        <td className="px-4 py-3">
                          <span
                            className={`px-2 py-0.5 rounded text-[10px] font-extrabold uppercase ${
                              b.type_intermediaire === 'AGA'
                                ? 'bg-indigo-50 text-indigo-700 border border-indigo-200'
                                : 'bg-emerald-50 text-emerald-700 border border-emerald-200'
                            }`}
                          >
                            {b.type_intermediaire}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-center font-medium text-slate-600">
                          {b.mois ? String(b.mois).padStart(2, '0') + '/' : ''}
                          {b.exercice}
                        </td>
                        <td className="px-4 py-3 text-right font-mono text-slate-700">
                          {b.total_prime_nette.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                        </td>
                        <td className="px-4 py-3 text-right font-mono text-indigo-900 font-semibold">
                          {b.total_commission_ht.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                        </td>
                        <td className="px-4 py-3 text-right font-mono font-extrabold text-emerald-700">
                          {b.total_commission_ttc.toLocaleString('fr-DZ', { minimumFractionDigits: 2 })}
                        </td>
                        <td className="px-4 py-3 text-center">
                          <div className="flex items-center justify-center gap-1.5">
                            <button
                              onClick={() => handleViewBordereau(b.id!)}
                              title="Voir & Imprimer"
                              className="p-1.5 rounded-lg bg-indigo-50 hover:bg-indigo-100 text-indigo-700 transition-colors cursor-pointer"
                            >
                              <Eye className="w-3.5 h-3.5" />
                            </button>
                            <button
                              onClick={() => handleExportExcel(b)}
                              title="Exporter Excel"
                              className="p-1.5 rounded-lg bg-emerald-50 hover:bg-emerald-100 text-emerald-700 transition-colors cursor-pointer"
                            >
                              <FileSpreadsheet className="w-3.5 h-3.5" />
                            </button>
                            <button
                              onClick={() => handleDelete(b.id!)}
                              title="Supprimer"
                              className="p-1.5 rounded-lg bg-rose-50 hover:bg-rose-100 text-rose-700 transition-colors cursor-pointer"
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
        </div>
      )}
    </div>
  );
};
