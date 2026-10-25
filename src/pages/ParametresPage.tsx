import React, { useState, useEffect } from 'react';
import { 
  Sliders, 
  Search, 
  Plus, 
  Edit2, 
  Trash2, 
  Copy, 
  Check, 
  X, 
  RotateCcw,
  CheckCircle2,
  TrendingUp,
  Percent,
  AlertTriangle,
  Layers,
  ArrowUpDown,
  BookOpen
} from 'lucide-react';
import { BrancheParam, Categorie } from '../types';
import { api, errMsg } from '../lib/api';
import { ConfirmDialog, ConfirmState } from '../components/Shared';

const DEFAULT_FORM: BrancheParam = {
  code: '',
  branche: '',
  sous_code: '',
  sous_libelle: '',
  taux_apport_courtier: 0,
  taux_apport_aga: 0,
  taux_gestion_aga: 0,
  alias_import: '',
  groupe_bordereau: '',
  ordre_bordereau: 999,
  categorie_id: null,
};

export const ParametresPage: React.FC = () => {
  const [branches, setBranches] = useState<BrancheParam[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [selectedCategory, setSelectedCategory] = useState<string>('ALL');
  const [categoriesDb, setCategoriesDb] = useState<Categorie[]>([]);
  const [showCategories, setShowCategories] = useState<boolean>(false);
  const [nouvelleCategorie, setNouvelleCategorie] = useState<string>('');
  const [confirmCat, setConfirmCat] = useState<ConfirmState | null>(null);
  
  // Quick inline edit
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<BrancheParam | null>(null);

  // Modal create / edit
  const [showModal, setShowModal] = useState<boolean>(false);
  const [modalMode, setModalMode] = useState<'create' | 'edit'>('create');
  const [modalForm, setModalForm] = useState<BrancheParam>(DEFAULT_FORM);
  const [modalRatesPercent, setModalRatesPercent] = useState({
    courtier: 0,
    apportAga: 0,
    gestionAga: 0,
  });

  // Modal delete confirmation
  const [itemToDelete, setItemToDelete] = useState<BrancheParam | null>(null);
  const [showDeleteModal, setShowDeleteModal] = useState<boolean>(false);

  // Modal reset confirmation
  const [showResetModal, setShowResetModal] = useState<boolean>(false);

  // Alerts
  const [notification, setNotification] = useState<{ type: 'success' | 'info' | 'error'; message: string } | null>(null);

  const showToast = (message: string, type: 'success' | 'info' | 'error' = 'success') => {
    setNotification({ type, message });
    setTimeout(() => {
      setNotification(null);
    }, 3500);
  };

  useEffect(() => {
    loadBranches();
  }, []);

  const loadBranches = async () => {
    setLoading(true);
    try {
      const [data, cats] = await Promise.all([api.getBranches(), api.getCategories()]);
      setBranches(data);
      setCategoriesDb(cats);
    } catch (err) {
      console.error('Erreur chargement branches:', err);
      showToast('Erreur lors du chargement des branches', 'error');
    } finally {
      setLoading(false);
    }
  };

  // --- Actions Modal Create / Edit / Duplicate ---

  const openCreateModal = () => {
    setModalMode('create');
    setModalForm({ ...DEFAULT_FORM });
    setModalRatesPercent({
      courtier: 0,
      apportAga: 0,
      gestionAga: 0,
    });
    setShowModal(true);
  };

  const openEditModal = (b: BrancheParam) => {
    setModalMode('edit');
    setModalForm({ ...b });
    setModalRatesPercent({
      courtier: Math.round(b.taux_apport_courtier * 1000) / 10,
      apportAga: Math.round(b.taux_apport_aga * 1000) / 10,
      gestionAga: Math.round(b.taux_gestion_aga * 1000) / 10,
    });
    setShowModal(true);
  };

  const duplicateBranche = (b: BrancheParam) => {
    setModalMode('create');
    setModalForm({
      code: b.code,
      branche: b.branche,
      categorie_id: b.categorie_id ?? null,
      sous_code: `${b.sous_code}_COPIE`,
      sous_libelle: `${b.sous_libelle} (Copie)`,
      taux_apport_courtier: b.taux_apport_courtier,
      taux_apport_aga: b.taux_apport_aga,
      taux_gestion_aga: b.taux_gestion_aga,
      alias_import: '',
      groupe_bordereau: b.groupe_bordereau,
      ordre_bordereau: b.ordre_bordereau,
    });
    setModalRatesPercent({
      courtier: Math.round(b.taux_apport_courtier * 1000) / 10,
      apportAga: Math.round(b.taux_apport_aga * 1000) / 10,
      gestionAga: Math.round(b.taux_gestion_aga * 1000) / 10,
    });
    setShowModal(true);
  };

  const handleModalSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!modalForm.code.trim() || !modalForm.branche.trim()) {
      alert('Veuillez renseigner au moins le Code et le nom de la Branche.');
      return;
    }

    const payload: BrancheParam = {
      ...modalForm,
      code: modalForm.code.trim(),
      branche: modalForm.branche.trim(),
      sous_code: modalForm.sous_code.trim().toUpperCase(),
      sous_libelle: modalForm.sous_libelle.trim() || modalForm.branche.trim(),
      alias_import: modalForm.alias_import.trim().toUpperCase(),
      groupe_bordereau: modalForm.groupe_bordereau.trim(),
      ordre_bordereau: Number(modalForm.ordre_bordereau) || 999,
      taux_apport_courtier: Math.round(modalRatesPercent.courtier * 100) / 10000,
      taux_apport_aga: Math.round(modalRatesPercent.apportAga * 100) / 10000,
      taux_gestion_aga: Math.round(modalRatesPercent.gestionAga * 100) / 10000,
    };

    try {
      await api.saveBranche(payload);
      setShowModal(false);
      showToast(
        modalMode === 'create'
          ? `Barème « ${payload.sous_code || payload.code} » ajouté avec succès.`
          : `Barème « ${payload.sous_code || payload.code} » mis à jour avec succès.`
      );
      await loadBranches();
    } catch (err) {
      alert('Erreur enregistrement : ' + err);
    }
  };

  // --- Inline Edit ---

  const startInlineEdit = (b: BrancheParam) => {
    setEditingId(b.id || null);
    setEditForm({ ...b });
  };

  const cancelInlineEdit = () => {
    setEditingId(null);
    setEditForm(null);
  };

  const saveInlineEdit = async () => {
    if (!editForm) return;
    try {
      await api.saveBranche(editForm);
      showToast(`Taux de « ${editForm.sous_code || editForm.branche} » mis à jour.`);
      setEditingId(null);
      setEditForm(null);
      await loadBranches();
    } catch (err) {
      alert('Erreur enregistrement : ' + err);
    }
  };

  // --- Deletion ---

  const confirmDelete = (b: BrancheParam) => {
    setItemToDelete(b);
    setShowDeleteModal(true);
  };

  const handleDelete = async () => {
    if (!itemToDelete || !itemToDelete.id) return;
    try {
      await api.deleteBranche(itemToDelete.id);
      setShowDeleteModal(false);
      const name = itemToDelete.sous_code || itemToDelete.branche;
      setItemToDelete(null);
      showToast(`Barème « ${name} » supprimé avec succès.`);
      await loadBranches();
    } catch (err) {
      alert('Erreur lors de la suppression : ' + err);
    }
  };

  // --- Reset to Default ---

  const handleResetToDefault = async () => {
    try {
      await api.resetBranchesToDefault();
      setShowResetModal(false);
      showToast('Barème d’origine (70 produits) réinitialisé avec succès !');
      await loadBranches();
    } catch (err) {
      alert('Erreur réinitialisation : ' + err);
    }
  };

  // --- Categories & Filtering ---
  const zeroRateCount = branches.filter((b) => b.taux_apport_courtier === 0 || b.taux_apport_aga === 0).length;

  const sansCategorie = branches.filter((b) => !b.categorie_id).length;
  const categories = [
    { id: 'ALL', label: `Toutes les branches (${branches.length})` },
    ...(zeroRateCount > 0 ? [{ id: 'ZERO_RATE', label: `⚠️ Taux nuls (${zeroRateCount})` }] : []),
    ...categoriesDb.map((c) => ({ id: `CAT_${c.id}`, label: `${c.libelle} (${branches.filter((b) => b.categorie_id === c.id).length})` })),
    ...(sansCategorie > 0 ? [{ id: 'SANS', label: `Sans catégorie (${sansCategorie})` }] : []),
  ];

  const filtered = branches.filter((b) => {
    const s = searchTerm.toLowerCase().trim();
    const matchSearch =
      !s ||
      b.code.toLowerCase().includes(s) ||
      b.branche.toLowerCase().includes(s) ||
      b.sous_code.toLowerCase().includes(s) ||
      b.sous_libelle.toLowerCase().includes(s);

    if (!matchSearch) return false;

    if (selectedCategory === 'ALL') return true;
    if (selectedCategory === 'ZERO_RATE') return b.taux_apport_courtier === 0 || b.taux_apport_aga === 0;
    if (selectedCategory === 'SANS') return !b.categorie_id;
    return `CAT_${b.categorie_id}` === selectedCategory;
  });

  // --- CRUD des catégories ---
  const enregistrerCategorie = async (c: Categorie) => {
    try {
      await api.saveCategorie(c);
      await loadBranches();
      return true;
    } catch (err) {
      showToast(errMsg(err), 'error');
      return false;
    }
  };

  const ajouterCategorie = async () => {
    const libelle = nouvelleCategorie.trim();
    if (!libelle) return;
    const ordre = (categoriesDb.reduce((m, c) => Math.max(m, c.ordre), 0) || 0) + 10;
    if (await enregistrerCategorie({ libelle, ordre })) {
      setNouvelleCategorie('');
      showToast(`Catégorie « ${libelle} » ajoutée.`);
    }
  };

  const supprimerCategorie = (c: Categorie) => {
    setConfirmCat({
      title: 'Supprimer la catégorie ?',
      danger: true,
      message: (
        <p>
          Supprimer « <strong>{c.libelle}</strong> » ? Ses {c.nb_branches ?? 0} ligne(s) de paramètres passeront « Sans catégorie »
          (taux inchangés).
        </p>
      ),
      confirmLabel: 'Supprimer',
      onConfirm: async () => {
        try {
          await api.deleteCategorie(c.id!);
          if (selectedCategory === `CAT_${c.id}`) setSelectedCategory('ALL');
          await loadBranches();
          showToast(`Catégorie « ${c.libelle} » supprimée.`);
        } catch (err) {
          showToast(errMsg(err), 'error');
        }
      },
    });
  };

  /** Range toutes les lignes affichées (filtre courant) dans une catégorie */
  const rangerAffichees = async (categorieId: number | null) => {
    try {
      await api.affecterCategorie(filtered.map((b) => b.id!).filter(Boolean), categorieId);
      await loadBranches();
      showToast(`${filtered.length} ligne(s) rangée(s).`);
    } catch (err) {
      showToast(errMsg(err), 'error');
    }
  };

  // KPI Calculations
  const totalCount = branches.length;
  const avgCourtier = totalCount > 0 ? (branches.reduce((acc, b) => acc + b.taux_apport_courtier, 0) / totalCount) * 100 : 0;
  const avgApportAga = totalCount > 0 ? (branches.reduce((acc, b) => acc + b.taux_apport_aga, 0) / totalCount) * 100 : 0;
  const avgGestAga = totalCount > 0 ? (branches.reduce((acc, b) => acc + b.taux_gestion_aga, 0) / totalCount) * 100 : 0;

  return (
    <div className="space-y-6 pb-12">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed bottom-6 right-6 z-50 animate-in fade-in slide-in-from-bottom-5">
          <div
            className={`flex items-center gap-3 px-4 py-3 rounded-xl shadow-lg border text-xs font-bold ${
              notification.type === 'success'
                ? 'bg-emerald-600 text-white border-emerald-500'
                : notification.type === 'error'
                ? 'bg-rose-600 text-white border-rose-500'
                : 'bg-indigo-600 text-white border-indigo-500'
            }`}
          >
            <CheckCircle2 className="w-4 h-4 shrink-0" />
            <span>{notification.message}</span>
          </div>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-black text-slate-900 tracking-tight flex items-center gap-2.5">
            <Sliders className="w-6 h-6 text-indigo-600" />
            Barème des Taux & Commissions
          </h1>
          <p className="text-xs text-slate-500 mt-1">
            Configuration et gestion CRUD des taux de commission (Courtiers & AGA) par branche d’assurance et code garantie.
          </p>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={() => setShowResetModal(true)}
            title="Restaurer le barème officiel d'origine"
            className="px-3.5 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold transition-all flex items-center gap-1.5 cursor-pointer border border-slate-200 shadow-2xs"
          >
            <RotateCcw className="w-3.5 h-3.5 text-slate-500" />
            <span>Réinitialiser</span>
          </button>

          <button
            onClick={openCreateModal}
            className="px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold transition-all flex items-center gap-2 cursor-pointer shadow-xs"
          >
            <Plus className="w-4 h-4" />
            <span>Nouveau Barème</span>
          </button>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-2xs">
          <div className="flex items-center justify-between text-slate-500 text-[11px] font-bold uppercase tracking-wider mb-1">
            <span>Produits Référencés</span>
            <Layers className="w-4 h-4 text-indigo-500" />
          </div>
          <p className="text-2xl font-black text-slate-900">{totalCount}</p>
          <span className="text-[10px] text-slate-400">Garanties paramétrées</span>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-indigo-100 bg-indigo-50/20 shadow-2xs">
          <div className="flex items-center justify-between text-indigo-700 text-[11px] font-bold uppercase tracking-wider mb-1">
            <span>Moyenne Courtier</span>
            <Percent className="w-4 h-4 text-indigo-600" />
          </div>
          <p className="text-2xl font-black text-indigo-900">{avgCourtier.toFixed(1)}%</p>
          <span className="text-[10px] text-indigo-500">Taux apport courtier</span>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-sky-100 bg-sky-50/20 shadow-2xs">
          <div className="flex items-center justify-between text-sky-700 text-[11px] font-bold uppercase tracking-wider mb-1">
            <span>Moyenne Apport AGA</span>
            <TrendingUp className="w-4 h-4 text-sky-600" />
          </div>
          <p className="text-2xl font-black text-sky-900">{avgApportAga.toFixed(1)}%</p>
          <span className="text-[10px] text-sky-500">Taux commission apport</span>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-emerald-100 bg-emerald-50/20 shadow-2xs">
          <div className="flex items-center justify-between text-emerald-700 text-[11px] font-bold uppercase tracking-wider mb-1">
            <span>Moyenne Gestion AGA</span>
            <Percent className="w-4 h-4 text-emerald-600" />
          </div>
          <p className="text-2xl font-black text-emerald-900">{avgGestAga.toFixed(1)}%</p>
          <span className="text-[10px] text-emerald-500">Taux gestion agence</span>
        </div>
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
          <button
            onClick={() => setShowCategories(true)}
            title="Ajouter, renommer, ordonner ou supprimer les catégories"
            className="px-3 py-1.5 rounded-lg text-xs font-bold border border-dashed border-indigo-300 text-indigo-700 hover:bg-indigo-50 cursor-pointer flex items-center gap-1"
          >
            <Edit2 className="w-3 h-3" /> Gérer les catégories
          </button>
        </div>

        {/* Search */}
        <div className="relative">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
          <input
            type="text"
            placeholder="Rechercher code, branche, garantie..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9 pr-4 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500 w-64 sm:w-72"
          />
        </div>
      </div>

      {/* Rates Table */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
        <div className="px-5 py-3 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
          <div className="flex items-center gap-2">
            <span className="text-xs font-bold text-slate-700">Liste des Garanties et Taux</span>
            <span className="text-[11px] font-bold px-2 py-0.5 rounded-full bg-indigo-50 text-indigo-700 border border-indigo-100">
              {filtered.length} affichés sur {branches.length}
            </span>
          </div>
          <span className="text-[11px] text-slate-400">
            Astuce : vous pouvez ajuster les taux directement ou utiliser le crayon pour une édition complète.
          </span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-slate-700 font-bold uppercase tracking-wider">
                <th className="px-4 py-3">Code</th>
                <th className="px-4 py-3">Branche d'assurance</th>
                <th className="px-4 py-3">Sous-Code</th>
                <th className="px-4 py-3">Sous-Garantie / Libellé</th>
                <th className="px-4 py-3">Alias import / Groupe bordereau</th>
                <th className="px-4 py-3 text-center bg-indigo-50/50">Taux Courtier</th>
                <th className="px-4 py-3 text-center bg-sky-50/50">Taux Apport AGA</th>
                <th className="px-4 py-3 text-center bg-emerald-50/50">Taux Gestion AGA</th>
                <th className="px-4 py-3 text-center">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading ? (
                <tr>
                  <td colSpan={9} className="text-center py-12 text-slate-400">
                    Chargement du barème en cours...
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={9} className="text-center py-12 text-slate-400">
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
                      {/* Code */}
                      <td className="px-4 py-2.5 font-mono font-bold text-slate-800">
                        {b.code}
                      </td>

                      {/* Branche */}
                      <td className="px-4 py-2.5 font-semibold text-slate-900">
                        {b.branche}
                      </td>

                      {/* Sous-Code */}
                      <td className="px-4 py-2.5">
                        <span className="font-mono font-bold text-indigo-700 bg-indigo-50 px-2 py-0.5 rounded border border-indigo-200">
                          {b.sous_code || '-'}
                        </span>
                      </td>

                      {/* Sous-Libelle */}
                      <td className="px-4 py-2.5 text-slate-600">
                        {b.sous_libelle || b.branche}
                      </td>

                      {/* Alias d'import / groupe */}
                      <td className="px-4 py-2.5 text-[11px]">
                        {b.alias_import && (
                          <span className="font-mono font-bold text-amber-800 bg-amber-50 px-1.5 py-0.5 rounded border border-amber-200">
                            {b.alias_import}
                          </span>
                        )}
                        {b.groupe_bordereau && (
                          <div className="text-slate-500 mt-0.5">
                            {b.groupe_bordereau} <span className="text-slate-400">(ordre {b.ordre_bordereau})</span>
                          </div>
                        )}
                        {!b.alias_import && !b.groupe_bordereau && <span className="text-slate-300">-</span>}
                      </td>

                      {/* Taux Apport Courtier */}
                      <td className="px-4 py-2.5 text-center font-bold text-indigo-900 bg-indigo-50/20">
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
                        ) : b.taux_apport_courtier === 0 ? (
                          <span className="px-2 py-0.5 rounded-md bg-rose-100 text-rose-800 border border-rose-200 font-bold">
                            0.0% (À définir)
                          </span>
                        ) : (
                          <span className="px-2 py-0.5 rounded-md bg-indigo-100/60 text-indigo-800">
                            {(b.taux_apport_courtier * 100).toFixed(1)}%
                          </span>
                        )}
                      </td>

                      {/* Taux Apport AGA */}
                      <td className="px-4 py-2.5 text-center font-bold text-sky-900 bg-sky-50/20">
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
                        ) : b.taux_apport_aga === 0 ? (
                          <span className="px-2 py-0.5 rounded-md bg-rose-100 text-rose-800 border border-rose-200 font-bold">
                            0.0% (À définir)
                          </span>
                        ) : (
                          <span className="px-2 py-0.5 rounded-md bg-sky-100/60 text-sky-800">
                            {(b.taux_apport_aga * 100).toFixed(1)}%
                          </span>
                        )}
                      </td>

                      {/* Taux Gestion AGA */}
                      <td className="px-4 py-2.5 text-center font-bold text-emerald-900 bg-emerald-50/20">
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
                        ) : b.taux_gestion_aga === 0 ? (
                          <span className="px-2 py-0.5 rounded-md bg-rose-100 text-rose-800 border border-rose-200 font-bold">
                            0.0% (À définir)
                          </span>
                        ) : (
                          <span className="px-2 py-0.5 rounded-md bg-emerald-100/60 text-emerald-800">
                            {(b.taux_gestion_aga * 100).toFixed(1)}%
                          </span>
                        )}
                      </td>

                      {/* Actions */}
                      <td className="px-4 py-2.5 text-center">
                        {isEditing ? (
                          <div className="flex items-center justify-center gap-1">
                            <button
                              onClick={saveInlineEdit}
                              title="Enregistrer"
                              className="p-1.5 rounded bg-emerald-600 hover:bg-emerald-700 text-white cursor-pointer shadow-2xs"
                            >
                              <Check className="w-3.5 h-3.5" />
                            </button>
                            <button
                              onClick={cancelInlineEdit}
                              title="Annuler"
                              className="p-1.5 rounded bg-slate-200 hover:bg-slate-300 text-slate-700 cursor-pointer"
                            >
                              <X className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center justify-center gap-1">
                            {/* Inline quick edit */}
                            <button
                              onClick={() => startInlineEdit(b)}
                              title="Édition rapide des taux"
                              className="p-1.5 rounded-lg bg-slate-100 hover:bg-indigo-50 hover:text-indigo-600 text-slate-600 transition-colors cursor-pointer"
                            >
                              <Percent className="w-3.5 h-3.5" />
                            </button>

                            {/* Full Edit Modal */}
                            <button
                              onClick={() => openEditModal(b)}
                              title="Modifier la garantie et libellés"
                              className="p-1.5 rounded-lg bg-slate-100 hover:bg-amber-50 hover:text-amber-600 text-slate-600 transition-colors cursor-pointer"
                            >
                              <Edit2 className="w-3.5 h-3.5" />
                            </button>

                            {/* Duplicate */}
                            <button
                              onClick={() => duplicateBranche(b)}
                              title="Dupliquer pour créer une nouvelle garantie"
                              className="p-1.5 rounded-lg bg-slate-100 hover:bg-sky-50 hover:text-sky-600 text-slate-600 transition-colors cursor-pointer"
                            >
                              <Copy className="w-3.5 h-3.5" />
                            </button>

                            {/* Delete */}
                            <button
                              onClick={() => confirmDelete(b)}
                              title="Supprimer cette garantie"
                              className="p-1.5 rounded-lg bg-slate-100 hover:bg-rose-50 hover:text-rose-600 text-slate-600 transition-colors cursor-pointer"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
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

      {/* --- MODAL AJOUT / ÉDITION COMPLÈTE --- */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 animate-in fade-in">
          <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-lg w-full overflow-hidden animate-in zoom-in-95">
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
              <h3 className="font-extrabold text-slate-900 flex items-center gap-2 text-sm">
                {modalMode === 'create' ? (
                  <>
                    <Plus className="w-4 h-4 text-indigo-600" />
                    <span>Ajouter un Nouveau Barème / Garantie</span>
                  </>
                ) : (
                  <>
                    <Edit2 className="w-4 h-4 text-indigo-600" />
                    <span>Modifier le Barème : {modalForm.sous_code || modalForm.code}</span>
                  </>
                )}
              </h3>
              <button
                onClick={() => setShowModal(false)}
                className="p-1 rounded-lg hover:bg-slate-200 text-slate-400 hover:text-slate-600 cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleModalSave} className="p-6 space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                    Code Branche *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="Ex: 3.1.0.1"
                    value={modalForm.code}
                    onChange={(e) => setModalForm({ ...modalForm, code: e.target.value })}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono font-bold focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                    Sous-Code Garantie
                  </label>
                  <input
                    type="text"
                    placeholder="Ex: VRC, CANAT..."
                    value={modalForm.sous_code}
                    onChange={(e) => setModalForm({ ...modalForm, sous_code: e.target.value.toUpperCase() })}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono font-bold uppercase focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                  />
                </div>
              </div>

              <div>
                <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                  Nom de la Branche Principale *
                </label>
                <input
                  type="text"
                  required
                  placeholder="Ex: AUTOMOBILE PARTICULIER"
                  value={modalForm.branche}
                  onChange={(e) => setModalForm({ ...modalForm, branche: e.target.value })}
                  className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                  Libellé de la Garantie / Sous-Garantie
                </label>
                <input
                  type="text"
                  placeholder="Ex: Responsabilité Civile"
                  value={modalForm.sous_libelle}
                  onChange={(e) => setModalForm({ ...modalForm, sous_libelle: e.target.value })}
                  className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">Catégorie</label>
                <select
                  value={modalForm.categorie_id ?? ''}
                  onChange={(e) => setModalForm({ ...modalForm, categorie_id: e.target.value ? Number(e.target.value) : null })}
                  className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                >
                  <option value="">Sans catégorie</option>
                  {categoriesDb.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.libelle}
                    </option>
                  ))}
                </select>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                    Alias d'import
                  </label>
                  <input
                    type="text"
                    placeholder="Ex: VP,VO,VR"
                    value={modalForm.alias_import}
                    onChange={(e) => setModalForm({ ...modalForm, alias_import: e.target.value.toUpperCase() })}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono font-bold focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                  />
                </div>
                <div>
                  <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                    Groupe bordereau
                  </label>
                  <input
                    type="text"
                    placeholder="Ex: RC AUTOMOBILE"
                    value={modalForm.groupe_bordereau}
                    onChange={(e) => setModalForm({ ...modalForm, groupe_bordereau: e.target.value })}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                  />
                </div>
                <div>
                  <label className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-1">
                    Ordre
                  </label>
                  <input
                    type="number"
                    value={modalForm.ordre_bordereau}
                    onChange={(e) => setModalForm({ ...modalForm, ordre_bordereau: parseInt(e.target.value, 10) || 999 })}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                  />
                </div>
              </div>
              <p className="text-[10px] text-slate-500 -mt-2">
                Sous-code : code garantie des rapports CIRIS / PDF flotte. Alias d'import : codes branche des registres Excel
                (séparés par des virgules). Groupe : ligne cumulée du bordereau courtier.
              </p>

              {/* Taux Section */}
              <div className="border-t border-slate-100 pt-3">
                <span className="block text-[11px] font-bold text-slate-700 uppercase tracking-wider mb-2">
                  Taux de commission applicables (%)
                </span>

                <div className="grid grid-cols-3 gap-3">
                  <div className="bg-indigo-50/50 p-3 rounded-xl border border-indigo-100">
                    <label className="block text-[10px] font-black text-indigo-900 mb-1">
                      Apport Courtier
                    </label>
                    <div className="relative">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="100"
                        value={modalRatesPercent.courtier}
                        onChange={(e) =>
                          setModalRatesPercent({
                            ...modalRatesPercent,
                            courtier: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full pl-2 pr-5 py-1.5 bg-white border border-indigo-200 rounded-lg text-xs font-bold text-indigo-900 text-center"
                      />
                      <span className="absolute right-2 top-2 text-[10px] font-bold text-indigo-400">%</span>
                    </div>
                  </div>

                  <div className="bg-sky-50/50 p-3 rounded-xl border border-sky-100">
                    <label className="block text-[10px] font-black text-sky-900 mb-1">
                      Apport AGA
                    </label>
                    <div className="relative">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="100"
                        value={modalRatesPercent.apportAga}
                        onChange={(e) =>
                          setModalRatesPercent({
                            ...modalRatesPercent,
                            apportAga: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full pl-2 pr-5 py-1.5 bg-white border border-sky-200 rounded-lg text-xs font-bold text-sky-900 text-center"
                      />
                      <span className="absolute right-2 top-2 text-[10px] font-bold text-sky-400">%</span>
                    </div>
                  </div>

                  <div className="bg-emerald-50/50 p-3 rounded-xl border border-emerald-100">
                    <label className="block text-[10px] font-black text-emerald-900 mb-1">
                      Gestion AGA
                    </label>
                    <div className="relative">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="100"
                        value={modalRatesPercent.gestionAga}
                        onChange={(e) =>
                          setModalRatesPercent({
                            ...modalRatesPercent,
                            gestionAga: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full pl-2 pr-5 py-1.5 bg-white border border-emerald-200 rounded-lg text-xs font-bold text-emerald-900 text-center"
                      />
                      <span className="absolute right-2 top-2 text-[10px] font-bold text-emerald-400">%</span>
                    </div>
                  </div>
                </div>

                {/* Simulation preview */}
                <div className="mt-3 p-2.5 bg-slate-50 rounded-xl border border-slate-200 text-[11px] text-slate-600">
                  <div className="font-bold text-slate-800 mb-0.5">Simulation pour 100 000 DZD de prime :</div>
                  <div className="flex items-center justify-between text-[11px]">
                    <span>Courtier : <strong className="text-indigo-700">{((100000 * modalRatesPercent.courtier) / 100).toLocaleString()} DZD</strong></span>
                    <span>AGA : <strong className="text-emerald-700">{((100000 * (modalRatesPercent.apportAga + modalRatesPercent.gestionAga)) / 100).toLocaleString()} DZD</strong></span>
                  </div>
                </div>
              </div>

              <div className="flex items-center justify-end gap-2.5 pt-2 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setShowModal(false)}
                  className="px-4 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold cursor-pointer"
                >
                  Annuler
                </button>
                <button
                  type="submit"
                  className="px-5 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-xs cursor-pointer"
                >
                  {modalMode === 'create' ? 'Ajouter au barème' : 'Enregistrer les modifications'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* --- MODAL CONFIRMATION SUPPRESSION --- */}
      {showDeleteModal && itemToDelete && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 animate-in fade-in">
          <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-md w-full p-6 space-y-4 animate-in zoom-in-95">
            <div className="flex items-start gap-3">
              <div className="p-2.5 rounded-xl bg-rose-50 text-rose-600 border border-rose-100 shrink-0">
                <AlertTriangle className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-extrabold text-slate-900">
                  Confirmer la suppression
                </h3>
                <p className="text-xs text-slate-500 mt-1">
                  Êtes-vous certain de vouloir supprimer le barème suivant ?
                </p>
              </div>
            </div>

            <div className="p-3 bg-slate-50 rounded-xl border border-slate-200 text-xs space-y-1">
              <div><span className="text-slate-400 font-medium">Code :</span> <span className="font-mono font-bold text-slate-800">{itemToDelete.code}</span></div>
              <div><span className="text-slate-400 font-medium">Garantie :</span> <span className="font-bold text-slate-900">{itemToDelete.sous_code || '-'}</span> ({itemToDelete.sous_libelle || itemToDelete.branche})</div>
              <div><span className="text-slate-400 font-medium">Branche :</span> {itemToDelete.branche}</div>
            </div>

            <p className="text-[11px] text-amber-600 bg-amber-50 p-2.5 rounded-lg border border-amber-200">
              Note : L’historique des calculs des bordereaux déjà émis sera préservé.
            </p>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => {
                  setShowDeleteModal(false);
                  setItemToDelete(null);
                }}
                className="px-4 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold cursor-pointer"
              >
                Annuler
              </button>
              <button
                onClick={handleDelete}
                className="px-4 py-2 rounded-xl bg-rose-600 hover:bg-rose-700 text-white text-xs font-bold shadow-xs cursor-pointer"
              >
                Supprimer définitivement
              </button>
            </div>
          </div>
        </div>
      )}

      {/* --- MODAL CONFIRMATION RÉINITIALISATION --- */}
      {showResetModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 animate-in fade-in">
          <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-md w-full p-6 space-y-4 animate-in zoom-in-95">
            <div className="flex items-start gap-3">
              <div className="p-2.5 rounded-xl bg-indigo-50 text-indigo-600 border border-indigo-100 shrink-0">
                <RotateCcw className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-extrabold text-slate-900">
                  Réinitialiser le barème de référence
                </h3>
                <p className="text-xs text-slate-500 mt-1">
                  Cette action rechargera les 70 taux officiels issus du référentiel initial Excel « Param ».
                </p>
              </div>
            </div>

            <p className="text-xs text-slate-600 bg-slate-50 p-3 rounded-xl border border-slate-200">
              Toutes les modifications personnalisées et nouvelles garanties ajoutées seront réinitialisées à leur état d'origine.
            </p>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => setShowResetModal(false)}
                className="px-4 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold cursor-pointer"
              >
                Annuler
              </button>
              <button
                onClick={handleResetToDefault}
                className="px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-xs cursor-pointer"
              >
                Confirmer la réinitialisation
              </button>
            </div>
          </div>
        </div>
      )}
      {showCategories && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 p-4">
          <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-lg w-full overflow-hidden text-xs">
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
              <h3 className="font-extrabold text-slate-900 text-sm">Catégories de paramètres</h3>
              <button onClick={() => setShowCategories(false)} className="p-1 rounded-lg hover:bg-slate-200 text-slate-400 cursor-pointer">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-5 space-y-3">
              <p className="text-slate-500">
                Les catégories servent au classement de l'écran Paramètres (sans effet sur les calculs). Le libellé et l'ordre
                sont enregistrés en quittant le champ.
              </p>
              <div className="divide-y divide-slate-100 border border-slate-200 rounded-xl">
                {categoriesDb.map((c) => (
                  <div key={c.id} className="flex items-center gap-2 p-2">
                    <input
                      type="number"
                      defaultValue={c.ordre}
                      title="Ordre d'affichage"
                      onBlur={(e) => {
                        const ordre = parseInt(e.target.value, 10) || 0;
                        if (ordre !== c.ordre) enregistrerCategorie({ ...c, ordre });
                      }}
                      className="w-16 px-2 py-1.5 border border-slate-200 rounded-lg text-center"
                    />
                    <input
                      defaultValue={c.libelle}
                      onBlur={(e) => {
                        const libelle = e.target.value.trim();
                        if (libelle && libelle !== c.libelle) enregistrerCategorie({ ...c, libelle });
                        else e.target.value = c.libelle;
                      }}
                      className="flex-1 px-2 py-1.5 border border-slate-200 rounded-lg font-semibold"
                    />
                    <span className="text-slate-400 w-16 text-right">{c.nb_branches ?? 0} ligne(s)</span>
                    <button onClick={() => supprimerCategorie(c)} title="Supprimer" className="p-1.5 rounded-lg hover:bg-rose-50 text-rose-600 cursor-pointer">
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))}
                {categoriesDb.length === 0 && <div className="p-4 text-center text-slate-400">Aucune catégorie.</div>}
              </div>
              <div className="flex gap-2">
                <input
                  value={nouvelleCategorie}
                  onChange={(e) => setNouvelleCategorie(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      ajouterCategorie();
                    }
                  }}
                  placeholder="Nouvelle catégorie…"
                  className="flex-1 px-3 py-2 border border-slate-200 rounded-lg"
                />
                <button onClick={ajouterCategorie} className="px-3 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white font-bold cursor-pointer flex items-center gap-1">
                  <Plus className="w-3.5 h-3.5" /> Ajouter
                </button>
              </div>
              {filtered.length > 0 && (
                <div className="border-t border-slate-100 pt-3 flex flex-wrap items-center gap-2">
                  <span className="text-slate-600">Ranger les {filtered.length} ligne(s) actuellement affichée(s) dans :</span>
                  <select
                    defaultValue=""
                    onChange={(e) => {
                      if (e.target.value === '') return;
                      rangerAffichees(e.target.value === 'SANS' ? null : Number(e.target.value));
                      e.target.value = '';
                    }}
                    className="px-2 py-1.5 border border-slate-200 rounded-lg"
                  >
                    <option value="">— choisir —</option>
                    {categoriesDb.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.libelle}
                      </option>
                    ))}
                    <option value="SANS">Sans catégorie</option>
                  </select>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
      <ConfirmDialog state={confirmCat} onClose={() => setConfirmCat(null)} />
    </div>
  );
};
