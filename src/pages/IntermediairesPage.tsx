import React, { useState, useEffect } from 'react';
import { 
  Users, 
  Plus, 
  Edit2, 
  Building2, 
  Check, 
  X, 
  Phone, 
  MapPin, 
  Percent,
  Search,
  CheckCircle2
} from 'lucide-react';
import { Intermediaire, IntermediaireType } from '../types';
import { api } from '../lib/api';

export const IntermediairesPage: React.FC = () => {
  const [intermediaires, setIntermediaires] = useState<Intermediaire[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [showModal, setShowModal] = useState<boolean>(false);
  const [editingItem, setEditingItem] = useState<Intermediaire | null>(null);
  const [searchTerm, setSearchTerm] = useState<string>('');

  const [form, setForm] = useState<Intermediaire>({
    type_intermediaire: 'AGA',
    code: '',
    nom: '',
    succursale: 'SUCCURSALE SUD',
    departement: 'Département Commercial',
    contact: '',
    adresse: '',
    tva_applicable: 0.19,
    actif: true,
  });

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const list = await api.getIntermediaires();
      setIntermediaires(list);
    } catch (err) {
      console.error('Erreur:', err);
    } finally {
      setLoading(false);
    }
  };

  const openCreate = () => {
    setEditingItem(null);
    setForm({
      type_intermediaire: 'AGA',
      code: '',
      nom: '',
      succursale: 'SUCCURSALE SUD',
      departement: 'Département Commercial',
      contact: '',
      adresse: '',
      tva_applicable: 0.19,
      actif: true,
    });
    setShowModal(true);
  };

  const openEdit = (item: Intermediaire) => {
    setEditingItem(item);
    setForm({ ...item });
    setShowModal(true);
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await api.saveIntermediaire(form);
      setShowModal(false);
      await loadData();
    } catch (err) {
      alert('Erreur enregistrement: ' + err);
    }
  };

  const filtered = intermediaires.filter(
    (i) =>
      i.nom.toLowerCase().includes(searchTerm.toLowerCase()) ||
      i.code.toLowerCase().includes(searchTerm.toLowerCase())
  );

  return (
    <div className="space-y-6 pb-12">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-black text-slate-900 tracking-tight flex items-center gap-2">
            <Users className="w-6 h-6 text-indigo-600" />
            Répertoire des Intermédiaires (AGA & Courtiers)
          </h1>
          <p className="text-xs text-slate-500 mt-0.5">
            Gérez les fiches des Agents Généraux Agréés et Courtiers pour les calculs et bordereaux.
          </p>
        </div>

        <button
          onClick={openCreate}
          className="px-4 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-xs flex items-center gap-2 cursor-pointer transition-all shrink-0"
        >
          <Plus className="w-4 h-4" />
          <span>Ajouter un Intermédiaire</span>
        </button>
      </div>

      {/* Search */}
      <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs flex items-center justify-between">
        <div className="relative w-full max-w-sm">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
          <input
            type="text"
            placeholder="Rechercher nom, code..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9 pr-4 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-medium focus:outline-hidden focus:ring-2 focus:ring-indigo-500 w-full"
          />
        </div>
      </div>

      {/* Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
        {filtered.map((item) => (
          <div
            key={item.id}
            className="bg-white rounded-2xl border border-slate-200 p-6 shadow-xs hover:shadow-md transition-shadow relative group flex flex-col justify-between"
          >
            <div>
              <div className="flex items-start justify-between gap-2 mb-3">
                <span
                  className={`px-2.5 py-1 rounded-md text-[10px] font-black uppercase tracking-wider ${
                    item.type_intermediaire === 'AGA'
                      ? 'bg-indigo-50 text-indigo-700 border border-indigo-200'
                      : 'bg-emerald-50 text-emerald-700 border border-emerald-200'
                  }`}
                >
                  {item.type_intermediaire === 'AGA' ? 'Agent Général (AGA)' : 'Courtier'}
                </span>
                <span className="font-mono text-xs font-bold text-slate-500 bg-slate-100 px-2 py-0.5 rounded">
                  Code : {item.code}
                </span>
              </div>

              <h3 className="text-base font-extrabold text-slate-900 mb-1">{item.nom}</h3>
              <p className="text-xs text-slate-500 mb-4">{item.succursale} • {item.departement}</p>

              <div className="space-y-1.5 text-xs text-slate-600 border-t border-slate-100 pt-3">
                {item.contact && (
                  <div className="flex items-center gap-2">
                    <Phone className="w-3.5 h-3.5 text-slate-400" />
                    <span>{item.contact}</span>
                  </div>
                )}
                {item.adresse && (
                  <div className="flex items-center gap-2">
                    <MapPin className="w-3.5 h-3.5 text-slate-400" />
                    <span>{item.adresse}</span>
                  </div>
                )}
                <div className="flex items-center gap-2">
                  <Percent className="w-3.5 h-3.5 text-slate-400" />
                  <span>TVA applicable : {(item.tva_applicable * 100).toFixed(0)}%</span>
                </div>
              </div>
            </div>

            <div className="mt-5 pt-3 border-t border-slate-100 flex items-center justify-between">
              <span className="flex items-center gap-1.5 text-[11px] font-semibold text-emerald-600">
                <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
                Actif
              </span>

              <button
                onClick={() => openEdit(item)}
                className="px-3 py-1.5 rounded-lg bg-slate-100 hover:bg-indigo-50 hover:text-indigo-600 text-slate-700 text-xs font-bold transition-all flex items-center gap-1.5 cursor-pointer"
              >
                <Edit2 className="w-3.5 h-3.5" />
                <span>Modifier</span>
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* Create / Edit Modal */}
      {showModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center z-50 p-4">
          <div className="bg-white rounded-2xl shadow-xl max-w-md w-full p-6 border border-slate-200 animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100 mb-4">
              <h3 className="text-base font-bold text-slate-900">
                {editingItem ? "Modifier l'intermédiaire" : 'Nouvel Intermédiaire'}
              </h3>
              <button
                onClick={() => setShowModal(false)}
                className="text-slate-400 hover:text-slate-600 p-1 rounded-lg"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <form onSubmit={handleSave} className="space-y-4 text-xs">
              <div>
                <label className="block font-semibold text-slate-700 mb-1">Type d'intermédiaire</label>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => setForm({ ...form, type_intermediaire: 'AGA' })}
                    className={`py-2 px-3 rounded-lg font-bold border text-xs ${
                      form.type_intermediaire === 'AGA'
                        ? 'bg-indigo-600 text-white border-indigo-600'
                        : 'bg-slate-50 text-slate-700 border-slate-200'
                    }`}
                  >
                    AGA
                  </button>
                  <button
                    type="button"
                    onClick={() => setForm({ ...form, type_intermediaire: 'COURTIER' })}
                    className={`py-2 px-3 rounded-lg font-bold border text-xs ${
                      form.type_intermediaire === 'COURTIER'
                        ? 'bg-indigo-600 text-white border-indigo-600'
                        : 'bg-slate-50 text-slate-700 border-slate-200'
                    }`}
                  >
                    Courtier
                  </button>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-1">
                  <label className="block font-semibold text-slate-700 mb-1">Code</label>
                  <input
                    type="text"
                    required
                    value={form.code}
                    onChange={(e) => setForm({ ...form, code: e.target.value })}
                    className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-mono font-bold text-slate-900 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                    placeholder="ex: 492"
                  />
                </div>
                <div className="col-span-2">
                  <label className="block font-semibold text-slate-700 mb-1">Nom Complet / Agence</label>
                  <input
                    type="text"
                    required
                    value={form.nom}
                    onChange={(e) => setForm({ ...form, nom: e.target.value })}
                    className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-bold text-slate-900 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                    placeholder="ex: AGA ABDELLAH"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block font-semibold text-slate-700 mb-1">Succursale</label>
                  <input
                    type="text"
                    value={form.succursale}
                    onChange={(e) => setForm({ ...form, succursale: e.target.value })}
                    className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  />
                </div>
                <div>
                  <label className="block font-semibold text-slate-700 mb-1">Département</label>
                  <input
                    type="text"
                    value={form.departement}
                    onChange={(e) => setForm({ ...form, departement: e.target.value })}
                    className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  />
                </div>
              </div>

              <div>
                <label className="block font-semibold text-slate-700 mb-1">Contact (Téléphone / Email)</label>
                <input
                  type="text"
                  value={form.contact || ''}
                  onChange={(e) => setForm({ ...form, contact: e.target.value })}
                  className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                  placeholder="ex: 0550 12 34 56"
                />
              </div>

              <div>
                <label className="block font-semibold text-slate-700 mb-1">TVA Applicable</label>
                <select
                  value={form.tva_applicable}
                  onChange={(e) => setForm({ ...form, tva_applicable: parseFloat(e.target.value) })}
                  className="w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-medium text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden"
                >
                  <option value={0.19}>19% (Standard)</option>
                  <option value={0.09}>9% (Réduit)</option>
                  <option value={0.10}>10% (TAV)</option>
                  <option value={0.00}>0% (Exonéré)</option>
                </select>
              </div>

              <div className="flex justify-end gap-2 pt-4 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setShowModal(false)}
                  className="px-4 py-2 rounded-lg bg-slate-100 text-slate-700 font-bold hover:bg-slate-200 cursor-pointer"
                >
                  Annuler
                </button>
                <button
                  type="submit"
                  className="px-4 py-2 rounded-lg bg-indigo-600 text-white font-bold hover:bg-indigo-700 shadow-xs cursor-pointer"
                >
                  Enregistrer
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
