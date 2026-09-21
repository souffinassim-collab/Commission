import React, { useState, useEffect } from 'react';
import { 
  Sliders, 
  Search, 
  Filter, 
  Edit2, 
  Check, 
  X, 
  Save, 
  RotateCcw,
  CheckCircle2,
  HelpCircle,
  TrendingUp,
  Percent
} from 'lucide-react';
import { BrancheParam } from '../types';
import { api } from '../lib/api';

export const ParametresPage: React.FC = () => {
  const [branches, setBranches] = useState<BrancheParam[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [selectedCategory, setSelectedCategory] = useState<string>('ALL');
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<BrancheParam | null>(null);
  const [savedSuccess, setSavedSuccess] = useState<boolean>(false);

  useEffect(() => {
    loadBranches();
  }, []);

  const loadBranches = async () => {
    setLoading(true);
    try {
      const data = await api.getBranches();
      setBranches(data);
    } catch (err) {
      console.error('Erreur chargement branches:', err);
    } finally {
      setLoading(false);
    }
  };

  const startEdit = (b: BrancheParam) => {
    setEditingId(b.id || null);
    setEditForm({ ...b });
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditForm(null);
  };

  const saveEdit = async () => {
    if (!editForm) return;
    try {
      await api.updateBranche(editForm);
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 3000);
      setEditingId(null);
      setEditForm(null);
      await loadBranches();
    } catch (err) {
      alert('Erreur enregistrement : ' + err);
    }
  };

  const categories = [
    { id: 'ALL', label: 'Toutes les branches' },
    { id: 'AUTO', label: 'Automobile' },
    { id: 'TRANSPORT', label: 'Maritime & Aérien' },
    { id: 'INCENDIE', label: 'Incendie & Multirisques' },
    { id: 'RC', label: 'Responsabilité Civile' },
    { id: 'AGRICOLE', label: 'Agricole' },
  ];

  const filtered = branches.filter((b) => {
    const s = searchTerm.toLowerCase();
    const matchSearch =
      b.code.toLowerCase().includes(s) ||
      b.branche.toLowerCase().includes(s) ||
      b.sous_code.toLowerCase().includes(s) ||
      b.sous_libelle.toLowerCase().includes(s);

    if (!matchSearch) return false;

    if (selectedCategory === 'ALL') return true;
    const bLower = (b.branche + ' ' + b.sous_libelle).toLowerCase();
    if (selectedCategory === 'AUTO') return bLower.includes('auto');
    if (selectedCategory === 'TRANSPORT') return bLower.includes('navire') || bLower.includes('maritime') || bLower.includes('aérien') || bLower.includes('terrestre');
    if (selectedCategory === 'INCENDIE') return bLower.includes('incendie') || bLower.includes('multirisques') || bLower.includes('degats') || bLower.includes('vol');
    if (selectedCategory === 'RC') return bLower.includes('responsabilité') || b.sous_code.startsWith('VR') || b.code.startsWith('13.');
    if (selectedCategory === 'AGRICOLE') return bLower.includes('agricole') || b.code.startsWith('9.6') || b.code.startsWith('9.7');
    return true;
  });

  return (
    <div className="space-y-6 pb-12">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-black text-slate-900 tracking-tight flex items-center gap-2">
            <Sliders className="w-6 h-6 text-indigo-600" />
            Barème des Taux & Commissions (Param)
          </h1>
          <p className="text-xs text-slate-500 mt-0.5">
            Paramétrage des 70 produits d'assurance extraits de votre feuille Excel « Param ». Modifiez les taux en temps réel.
          </p>
        </div>

        {savedSuccess && (
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-emerald-50 text-emerald-800 border border-emerald-200 text-xs font-bold animate-in fade-in">
            <CheckCircle2 className="w-4 h-4 text-emerald-600" />
            <span>Taux mis à jour avec succès en base de données !</span>
          </div>
        )}
      </div>

      {/* Filter and Search Bar */}
      <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs flex flex-wrap items-center justify-between gap-4">
        {/* Category Pills */}
        <div className="flex flex-wrap items-center gap-1.5">
          {categories.map((cat) => (
            <button
              key={cat.id}
              onClick={() => setSelectedCategory(cat.id)}
              className={`px-3 py-1.5 rounded-lg text-xs font-bold transition-all cursor-pointer ${
                selectedCategory === cat.id
                  ? 'bg-indigo-600 text-white shadow-xs'
                  : 'bg-slate-100 text-slate-600 hover:bg-slate-200'
              }`}
            >
              {cat.label}
            </button>
          ))}
        </div>

        {/* Search */}
        <div className="relative">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
          <input
            type="text"
            placeholder="Rechercher code, branche, garantie..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9 pr-4 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500 w-64"
          />
        </div>
      </div>

      {/* Rates Table */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-slate-700 font-bold uppercase tracking-wider">
                <th className="px-4 py-3">Code</th>
                <th className="px-4 py-3">Branche d'assurance</th>
                <th className="px-4 py-3">Sous-Code</th>
                <th className="px-4 py-3">Sous-Garantie / Libellé</th>
                <th className="px-4 py-3 text-center bg-indigo-50/50">Taux Apport Courtier</th>
                <th className="px-4 py-3 text-center bg-sky-50/50">Taux Apport AGA</th>
                <th className="px-4 py-3 text-center bg-emerald-50/50">Taux Gestion AGA</th>
                <th className="px-4 py-3 text-center">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading ? (
                <tr>
                  <td colSpan={8} className="text-center py-10 text-slate-400">
                    Chargement du barème...
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={8} className="text-center py-10 text-slate-400">
                    Aucun produit ne correspond à votre recherche.
                  </td>
                </tr>
              ) : (
                filtered.map((b) => {
                  const isEditing = editingId === b.id;
                  return (
                    <tr
                      key={b.id}
                      className={`hover:bg-slate-50/80 transition-colors ${
                        isEditing ? 'bg-indigo-50/30' : ''
                      }`}
                    >
                      <td className="px-4 py-2.5 font-mono font-bold text-slate-700">{b.code}</td>
                      <td className="px-4 py-2.5 font-semibold text-slate-900">{b.branche}</td>
                      <td className="px-4 py-2.5 font-mono font-bold text-indigo-600">
                        {b.sous_code || '-'}
                      </td>
                      <td className="px-4 py-2.5 text-slate-600">
                        {b.sous_libelle || b.branche}
                      </td>

                      {/* Taux Apport Courtier */}
                      <td className="px-4 py-2.5 text-center font-bold text-indigo-800 bg-indigo-50/30">
                        {isEditing && editForm ? (
                          <div className="flex items-center justify-center gap-1">
                            <input
                              type="number"
                              step="0.005"
                              value={editForm.taux_apport_courtier}
                              onChange={(e) =>
                                setEditForm({
                                  ...editForm,
                                  taux_apport_courtier: parseFloat(e.target.value) || 0,
                                })
                              }
                              className="w-16 p-1 text-center bg-white border border-indigo-300 rounded font-bold text-xs"
                            />
                            <span className="text-[10px] text-slate-500">
                              ({(editForm.taux_apport_courtier * 100).toFixed(1)}%)
                            </span>
                          </div>
                        ) : (
                          <span>{(b.taux_apport_courtier * 100).toFixed(1)}%</span>
                        )}
                      </td>

                      {/* Taux Apport AGA */}
                      <td className="px-4 py-2.5 text-center font-bold text-sky-800 bg-sky-50/30">
                        {isEditing && editForm ? (
                          <div className="flex items-center justify-center gap-1">
                            <input
                              type="number"
                              step="0.005"
                              value={editForm.taux_apport_aga}
                              onChange={(e) =>
                                setEditForm({
                                  ...editForm,
                                  taux_apport_aga: parseFloat(e.target.value) || 0,
                                })
                              }
                              className="w-16 p-1 text-center bg-white border border-sky-300 rounded font-bold text-xs"
                            />
                            <span className="text-[10px] text-slate-500">
                              ({(editForm.taux_apport_aga * 100).toFixed(1)}%)
                            </span>
                          </div>
                        ) : (
                          <span>{(b.taux_apport_aga * 100).toFixed(1)}%</span>
                        )}
                      </td>

                      {/* Taux Gestion AGA */}
                      <td className="px-4 py-2.5 text-center font-bold text-emerald-800 bg-emerald-50/30">
                        {isEditing && editForm ? (
                          <div className="flex items-center justify-center gap-1">
                            <input
                              type="number"
                              step="0.005"
                              value={editForm.taux_gestion_aga}
                              onChange={(e) =>
                                setEditForm({
                                  ...editForm,
                                  taux_gestion_aga: parseFloat(e.target.value) || 0,
                                })
                              }
                              className="w-16 p-1 text-center bg-white border border-emerald-300 rounded font-bold text-xs"
                            />
                            <span className="text-[10px] text-slate-500">
                              ({(editForm.taux_gestion_aga * 100).toFixed(1)}%)
                            </span>
                          </div>
                        ) : (
                          <span>{(b.taux_gestion_aga * 100).toFixed(1)}%</span>
                        )}
                      </td>

                      {/* Action */}
                      <td className="px-4 py-2.5 text-center">
                        {isEditing ? (
                          <div className="flex items-center justify-center gap-1">
                            <button
                              onClick={saveEdit}
                              title="Enregistrer"
                              className="p-1 rounded bg-emerald-600 hover:bg-emerald-700 text-white cursor-pointer"
                            >
                              <Check className="w-3.5 h-3.5" />
                            </button>
                            <button
                              onClick={cancelEdit}
                              title="Annuler"
                              className="p-1 rounded bg-slate-200 hover:bg-slate-300 text-slate-700 cursor-pointer"
                            >
                              <X className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        ) : (
                          <button
                            onClick={() => startEdit(b)}
                            title="Modifier les taux"
                            className="p-1.5 rounded-lg bg-slate-100 hover:bg-indigo-50 hover:text-indigo-600 text-slate-600 transition-colors cursor-pointer"
                          >
                            <Edit2 className="w-3.5 h-3.5" />
                          </button>
                        )}
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
