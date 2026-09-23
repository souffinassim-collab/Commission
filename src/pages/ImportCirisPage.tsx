import React, { useEffect, useRef, useState } from 'react';
import {
  UploadCloud,
  CheckCircle,
  AlertCircle,
  Save,
  FileSpreadsheet,
  Building2,
  FolderOpen,
  Trash2,
  Search,
  Layers,
  ChevronDown,
  ChevronRight,
  X,
  UserCheck,
  AlertTriangle,
  Settings,
  Lock,
  Eye,
  EyeOff,
  RotateCcw,
  FilePlus2,
  Info,
  UserPlus,
} from 'lucide-react';
import {
  Bordereau,
  BordereauLigne,
  BrancheParam,
  ExcelClientSummary,
  ExcelScanResult,
  ImportContext,
  Intermediaire,
  UploadedFileResult,
} from '../types';
import { api, errMsg, formatDZD, formatMontant, isTauri, MOIS_LIBELLES, readFileAsDataUrl } from '../lib/api';
import {
  AnomaliesBadges,
  AnomaliesPanel,
  anomaliesBloquantes,
  appliquerPolicesExclues,
  ConfirmDialog,
  ligneActive,
  PolicesPanel,
  ConfirmState,
  recomputeTotals,
  StatutBadge,
} from '../components/Shared';

interface ImportCirisPageProps {
  onBordereauSaved?: (id: number) => void;
  /** Signale à l'application qu'un import non enregistré est en cours (protection à la navigation) */
  onDirtyChange?: (dirty: boolean) => void;
}

type FileSource = { kind: 'path'; path: string; name: string } | { kind: 'blob'; base64: string; name: string };

interface Params {
  inter: Intermediaire | null;
  exercice: number;
  mois?: number;
  tva: number;
  cumul: boolean;
  clFilter: string;
}

interface RateModalState {
  ligne: BordereauLigne;
  mode: 'create' | 'edit' | 'alias';
  form: BrancheParam;
  pct: { courtier: number; apportAga: number; gestionAga: number };
  aliasTargetId?: number;
}

const FILE_TYPE_LABEL: Record<string, string> = {
  CIRIS_PDF: 'Rapport CIRIS',
  FLOTTE_PDF: 'PDF Flotte',
  EXCEL_PRODUCTION: 'Registre Excel',
};

const isExcel = (name: string) => /\.xlsx?$/i.test(name);
const pct = (t: number) => Math.round(t * 10000) / 100;
const fromPct = (p: number) => Math.round(p * 100) / 10000;
const periodeLabel = (exercice: number, mois?: number) =>
  mois ? `${MOIS_LIBELLES[mois - 1]} ${exercice}` : `exercice ${exercice} (annuel)`;
/** Anomalies propres au fichier + contrôles liés au bordereau (intermédiaire, période, doublon) */
const toutesAnomalies = (f: UploadedFileResult) => [...f.anomalies, ...(f.anomalies_contexte || [])];

interface InterCandidate {
  code: string;
  nom: string;
  type: Intermediaire['type_intermediaire'];
  source: string;
  existingId?: number;
}

interface InterModalState {
  mode: 'choisir' | 'creer';
  raison: string;
  selectedId?: number;
  form: Intermediaire;
  candidates?: InterCandidate[];
}

const ligneKey = (l: BordereauLigne) => `${l.code_garantie}|${l.libelle_garantie}|${l.branche_nom}`;

export const ImportCirisPage: React.FC<ImportCirisPageProps> = ({ onBordereauSaved, onDirtyChange }) => {
  const [intermediaires, setIntermediaires] = useState<Intermediaire[]>([]);
  const [branches, setBranches] = useState<BrancheParam[]>([]);
  const [interId, setInterId] = useState<number | null>(null);
  const [exercice, setExercice] = useState<number>(new Date().getFullYear());
  const [mois, setMois] = useState<number | undefined>(undefined);
  const [tvaTaux, setTvaTaux] = useState<number>(0.19);
  const [cumul, setCumul] = useState<boolean>(true);
  const [clientFilter, setClientFilter] = useState<string>('');

  const [files, setFilesState] = useState<UploadedFileResult[]>([]);
  const filesRef = useRef<UploadedFileResult[]>([]);
  const setFiles = (f: UploadedFileResult[]) => {
    filesRef.current = f;
    setFilesState(f);
  };
  const [bordereau, setBordereau] = useState<Bordereau | null>(null);
  const [modifie, setModifie] = useState<boolean>(false);
  // Polices exclues par le commercial (primes non encaissées), conservées lors des recalculs
  const policesExcluesRef = useRef<Set<string>>(new Set());

  const [loading, setLoading] = useState<boolean>(false);
  const [saving, setSaving] = useState<boolean>(false);
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [openFile, setOpenFile] = useState<string | null>(null);
  const [excelModal, setExcelModal] = useState<{ scan: ExcelScanResult; source: FileSource; search: string } | null>(null);
  const [rateModal, setRateModal] = useState<RateModalState | null>(null);
  const [interModal, setInterModal] = useState<InterModalState | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const inter = intermediaires.find((i) => i.id === interId) || null;
  const locked = files.length > 0;
  const isCourtier = inter?.type_intermediaire === 'COURTIER';

  // Paramètres courants lus par les traitements asynchrones (évite les valeurs périmées)
  const paramsRef = useRef<Params>({ inter, exercice, mois, tva: tvaTaux, cumul, clFilter: clientFilter });
  paramsRef.current = { inter, exercice, mois, tva: tvaTaux, cumul, clFilter: clientFilter };

  const loadRefs = async () => {
    try {
      const [inters, br] = await Promise.all([api.getIntermediaires(), api.getBranches()]);
      setIntermediaires(inters);
      setBranches(br);
    } catch (e) {
      setMessage({ type: 'error', text: errMsg(e) });
    }
  };

  useEffect(() => {
    loadRefs();
  }, []);

  // --- Protection contre la perte d'un import non enregistré ---
  const dirty = files.length > 0 && (!bordereau?.id || modifie);
  useEffect(() => {
    onDirtyChange?.(dirty);
    if (!dirty) return;
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = '';
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [dirty]);

  useEffect(
    () => () => {
      const ids = filesRef.current.map((f) => f.id);
      if (ids.length) api.clearPendingFiles(ids).catch(() => {});
      onDirtyChange?.(false);
    },
    []
  );

  // --- Calcul ---
  const recalculate = async (list: UploadedFileResult[], overrides: Partial<Params> = {}) => {
    const p = { ...paramsRef.current, ...overrides };
    if (list.length === 0 || !p.inter) {
      setBordereau(null);
      return;
    }
    try {
      const computed = await api.calculateMultiBordereau({
        files: list,
        intermediaire_id: p.inter.id,
        type_intermediaire: p.inter.type_intermediaire,
        intermediaire_code: p.inter.code,
        intermediaire_nom: p.inter.nom,
        code_client: p.clFilter || list.find((f) => f.file_type !== 'CIRIS_PDF' && f.code)?.code,
        succursale: p.inter.succursale,
        departement: p.inter.departement,
        tva_taux: p.tva,
        exercice: p.exercice,
        mois: p.mois,
        cumuler_par_branche: p.cumul,
      });
      setBordereau((prev) => {
        const base: Bordereau = prev?.id
          ? { ...computed, id: prev.id, numero_bordereau: prev.numero_bordereau, statut: prev.statut, fichiers: prev.fichiers }
          : computed;
        const exclues = new Set((prev?.lignes || []).filter((l) => l.desactive).map(ligneKey));
        const lignes = appliquerPolicesExclues(
          base.lignes.map((l) => ({ ...l, desactive: exclues.has(ligneKey(l)) })),
          policesExcluesRef.current
        );
        return recomputeTotals(base, lignes);
      });
      setModifie(true);
    } catch (e) {
      setMessage({ type: 'error', text: `Erreur lors du calcul : ${errMsg(e)}` });
    }
  };

  const resetImport = async () => {
    const ids = filesRef.current.map((f) => f.id);
    if (ids.length) await api.clearPendingFiles(ids).catch(() => {});
    setFiles([]);
    setBordereau(null);
    setModifie(false);
    policesExcluesRef.current = new Set();
    setClientFilter('');
    setExpanded(new Set());
    setOpenFile(null);
  };

  // --- Import de fichiers ---
  const currentCtx = (overrides: Partial<ImportContext> = {}): ImportContext => ({
    intermediaire_id: paramsRef.current.inter?.id,
    exercice: paramsRef.current.exercice,
    mois: paramsRef.current.mois,
    bordereau_id: bordereau?.id,
    ...overrides,
  });

  const processSource = async (source: FileSource, clientOverride?: string, ctxOverride: Partial<ImportContext> = {}) => {
    setLoading(true);
    setMessage(null);
    try {
      const filter = clientOverride || paramsRef.current.clFilter || undefined;
      if (isExcel(source.name) && !filter) {
        const scan =
          source.kind === 'path'
            ? await api.scanExcelClients(source.path)
            : await api.scanExcelClientsBase64(source.base64, source.name);
        if (scan.multiple_clients) {
          setExcelModal({ scan, source, search: '' });
          return;
        }
      }

      const ctx = currentCtx(ctxOverride);
      const res =
        source.kind === 'path'
          ? await api.parseFileAuto(source.path, ctx, filter)
          : await api.parseFileAutoBase64(source.base64, source.name, ctx, filter);

      const doublon = filesRef.current.find((f) => f.hash === res.hash);
      if (doublon) {
        await api.clearPendingFiles([res.id]);
        setMessage({ type: 'error', text: `Ce fichier est déjà inclus dans l'import en cours (« ${doublon.file_name} »).` });
        return;
      }

      // Premier fichier = rapport CIRIS : la période du bordereau est alignée automatiquement sur celle du rapport
      let fichier = res;
      let infoPeriode = '';
      if (
        res.file_type === 'CIRIS_PDF' &&
        filesRef.current.length === 0 &&
        res.exercice &&
        (res.exercice !== ctx.exercice || (res.mois && res.mois !== ctx.mois))
      ) {
        const ex = res.exercice;
        const m = res.mois ?? undefined;
        setExercice(ex);
        setMois(m);
        paramsRef.current = { ...paramsRef.current, exercice: ex, mois: m };
        [fichier] = await api.revaliderFichiers([res], { ...ctx, exercice: ex, mois: m });
        infoPeriode = ` Période du bordereau alignée sur le rapport : ${periodeLabel(ex, m)}.`;
      }

      const memeNom = filesRef.current.find((f) => f.file_name === fichier.file_name);
      const commit = async () => {
        if (memeNom) await api.clearPendingFiles([memeNom.id]).catch(() => {});
        const updated = [...filesRef.current.filter((f) => f.file_name !== fichier.file_name), fichier];
        const overrides: Partial<Params> = {};
        if (clientOverride) {
          setClientFilter(clientOverride);
          overrides.clFilter = clientOverride;
        } else if (!paramsRef.current.clFilter && fichier.code && fichier.file_type !== 'CIRIS_PDF') {
          setClientFilter(fichier.code);
          overrides.clFilter = fichier.code;
        }
        setFiles(updated);
        await recalculate(updated, overrides);
        const anomalies = toutesAnomalies(fichier);
        const bloquantes = anomalies.filter((a) => a.niveau === 'ERREUR').length;
        const avert = anomalies.filter((a) => a.niveau === 'AVERTISSEMENT').length;
        setMessage({
          type: bloquantes ? 'error' : 'success',
          text:
            `« ${fichier.file_name} » (${FILE_TYPE_LABEL[fichier.file_type] || fichier.file_type}) : ${fichier.items.length} lignes, ${formatDZD(fichier.total_prime_nette)}.` +
            infoPeriode +
            (bloquantes ? ` ${bloquantes} anomalie(s) bloquante(s) — voir le détail du fichier.` : '') +
            (avert ? ` ${avert} point(s) à vérifier.` : ''),
        });
        if (bloquantes || avert) setOpenFile(fichier.id);
        // Intermédiaire pas encore connu : identification automatique ou demande à l'utilisateur
        if (!paramsRef.current.inter) await identifierIntermediaire(fichier);
      };

      if (memeNom) {
        setConfirm({
          title: 'Remplacer un fichier déjà importé ?',
          message: (
            <p>
              Un fichier nommé <strong>{fichier.file_name}</strong> fait déjà partie de cet import. Le remplacer recalculera le bordereau
              en cours.
            </p>
          ),
          confirmLabel: 'Remplacer et recalculer',
          cancelLabel: 'Conserver l’ancien',
          onConfirm: commit,
          onCancel: () => {
            api.clearPendingFiles([res.id]).catch(() => {});
          },
        });
      } else {
        await commit();
      }
    } catch (e) {
      setMessage({ type: 'error', text: `Import refusé : ${errMsg(e)}` });
    } finally {
      setLoading(false);
    }
  };

  const processSourceRef = useRef(processSource);
  processSourceRef.current = processSource;

  // Glisser-déposer natif Tauri (enregistré une seule fois, lit l'état courant via les refs)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    (async () => {
      if (!isTauri()) return;
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const un = await getCurrentWebview().onDragDropEvent(async (event) => {
          if (event.payload.type === 'over') setIsDragging(true);
          else if (event.payload.type === 'drop') {
            setIsDragging(false);
            for (const p of event.payload.paths || []) {
              const name = p.split(/[\\/]/).pop() || p;
              if (/\.(pdf|xlsx?)$/i.test(name)) await processSourceRef.current({ kind: 'path', path: p, name });
              else setMessage({ type: 'error', text: `Format ignoré : ${name}. Formats acceptés : PDF (CIRIS / flotte) et Excel.` });
            }
          } else setIsDragging(false);
        });
        if (disposed) un();
        else unlisten = un;
      } catch (err) {
        console.warn('Glisser-déposer natif indisponible :', err);
      }
    })();
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);

  const handleBrowserFiles = async (list: FileList | null) => {
    if (!list) return;
    for (const f of Array.from(list)) {
      const base64 = await readFileAsDataUrl(f);
      await processSource({ kind: 'blob', base64, name: f.name });
    }
  };

  const handleSelectClient = async (client: ExcelClientSummary) => {
    if (!excelModal) return;
    const source = excelModal.source;
    setExcelModal(null);
    const code = client.code_client?.trim();
    const nom = client.designation_client?.trim() || code;
    let interId: number | undefined = undefined;
    if (code || nom) {
      let liste = intermediaires;
      try {
        liste = await api.getIntermediaires();
        setIntermediaires(liste);
      } catch {}
      let trouve = code ? liste.find((i) => i.code.trim().toUpperCase() === code.toUpperCase()) : undefined;
      if (!trouve && nom) {
        trouve = liste.find((i) => i.nom.trim().toUpperCase() === nom.toUpperCase());
      }
      if (trouve && trouve.actif) {
        await choisirIntermediaire(trouve);
        interId = trouve.id;
      } else if (!trouve && code) {
        try {
          const id = await api.saveIntermediaire(nouvelIntermediaire('COURTIER', code, nom || code));
          const maj = await api.getIntermediaires();
          setIntermediaires(maj);
          const cree = maj.find((i) => i.id === id);
          if (cree) {
            await choisirIntermediaire(cree);
            interId = cree.id;
          }
        } catch (e) {
          console.warn('Création auto intermédiaire client impossible :', e);
        }
      }
    }
    await processSource(source, client.code_client, interId ? { intermediaire_id: interId } : {});
  };

  const handleRemoveFile = (f: UploadedFileResult) => {
    setConfirm({
      title: 'Retirer ce fichier ?',
      message: (
        <p>
          Retirer <strong>{f.file_name}</strong> ({formatDZD(f.total_prime_nette)}) recalculera le bordereau en cours
          {bordereau?.id ? ' ; le bordereau enregistré ne sera modifié qu’au prochain enregistrement' : ''}.
        </p>
      ),
      confirmLabel: 'Retirer le fichier',
      danger: true,
      onConfirm: async () => {
        await api.clearPendingFiles([f.id]).catch(() => {});
        const updated = filesRef.current.filter((x) => x.id !== f.id);
        setFiles(updated);
        if (updated.length === 0) {
          setBordereau(null);
          setModifie(false);
        } else await recalculate(updated);
      },
    });
  };

  // --- Paramètres verrouillés : toute modification annule l'import en cours ---
  const demanderModificationParametres = () => {
    const enregistreSansModif = !!bordereau?.id && !modifie;
    setConfirm({
      title: enregistreSansModif ? 'Fermer le bordereau enregistré ?' : "Annuler l'import en cours ?",
      danger: !enregistreSansModif,
      message: enregistreSansModif ? (
        <p>
          Le bordereau <strong>{bordereau?.numero_bordereau}</strong> est enregistré. Pour modifier les paramètres, cet écran sera
          réinitialisé ; le bordereau reste consultable dans l'historique.
        </p>
      ) : (
        <>
          <p>
            L'import en cours comprend <strong>{files.length} fichier(s)</strong> rattaché(s) à <strong>{inter?.nom}</strong> pour{' '}
            <strong>{periodeLabel(exercice, mois)}</strong>, et un calcul {bordereau?.id ? 'modifié depuis le dernier enregistrement' : 'non enregistré'}.
          </p>
          <p>
            Modifier l'intermédiaire, la période, la TVA, le filtre client ou le mode de cumul <strong>annulera cet import</strong> :
            les fichiers et le calcul seront abandonnés et devront être réimportés avec les nouveaux paramètres.
          </p>
          <p className="font-bold">Voulez-vous annuler l'import en cours ?</p>
        </>
      ),
      confirmLabel: enregistreSansModif ? 'Fermer et modifier' : "Oui, annuler l'import",
      cancelLabel: "Non, conserver l'import",
      onConfirm: resetImport,
    });
  };

  // --- Intermédiaire : choisi avant l'import, ou identifié / demandé après l'import des fichiers ---
  const choisirIntermediaire = async (i: Intermediaire) => {
    setInterId(i.id ?? null);
    setTvaTaux(i.tva_applicable);
    setCumul(i.type_intermediaire === 'COURTIER');
    const p: Params = { ...paramsRef.current, inter: i, tva: i.tva_applicable, cumul: i.type_intermediaire === 'COURTIER' };
    paramsRef.current = p;
    if (filesRef.current.length === 0) return;
    try {
      // Les contrôles liés à l'intermédiaire (agence, période) sont relancés sur les fichiers déjà importés
      const revalides = await api.revaliderFichiers(filesRef.current, {
        intermediaire_id: i.id,
        exercice: p.exercice,
        mois: p.mois,
        bordereau_id: bordereau?.id,
      });
      setFiles(revalides);
      await recalculate(revalides, p);
      const bloquantes = revalides.flatMap((f) => (f.anomalies_contexte || []).filter((a) => a.niveau === 'ERREUR'));
      setMessage(
        bloquantes.length
          ? { type: 'error', text: `Intermédiaire ${i.code} · ${i.nom} rattaché, mais : ${bloquantes[0].message}` }
          : { type: 'success', text: `Import rattaché à ${i.type_intermediaire} ${i.code} · ${i.nom}.` }
      );
    } catch (e) {
      setMessage({ type: 'error', text: errMsg(e) });
    }
  };

  const nouvelIntermediaire = (type: Intermediaire['type_intermediaire'], code = '', nom = ''): Intermediaire => {
    const modele = intermediaires.find((x) => x.type_intermediaire === type) || intermediaires[0];
    return {
      type_intermediaire: type,
      code,
      nom,
      succursale: (modele?.succursale || '').trim() || 'SUCCURSALE SUD',
      departement: (modele?.departement || '').trim() || 'Département Commercial',
      tva_applicable: modele?.tva_applicable ?? 0.19,
      actif: true,
    };
  };

  const handleSelectCandidate = async (c: InterCandidate) => {
    try {
      let choisi: Intermediaire | undefined;
      let liste = intermediaires;
      try {
        liste = await api.getIntermediaires();
        setIntermediaires(liste);
      } catch {}

      if (c.existingId) {
        choisi = liste.find((i) => i.id === c.existingId);
      }
      if (!choisi && c.code) {
        choisi = liste.find((i) => i.code.trim().toUpperCase() === c.code.trim().toUpperCase());
      }
      if (!choisi && c.nom) {
        choisi = liste.find((i) => i.nom.trim().toUpperCase() === c.nom.trim().toUpperCase());
      }

      if (!choisi && c.code) {
        const id = await api.saveIntermediaire(nouvelIntermediaire(c.type, c.code, c.nom || c.code));
        const maj = await api.getIntermediaires();
        setIntermediaires(maj);
        choisi = maj.find((i) => i.id === id);
      }

      if (choisi) {
        setInterModal(null);
        await choisirIntermediaire(choisi);
        setMessage({
          type: 'success',
          text: `Intermédiaire rattaché : ${choisi.type_intermediaire} ${choisi.code} · ${choisi.nom}.`,
        });
      }
    } catch (e) {
      setMessage({ type: 'error', text: `Rattachement impossible : ${errMsg(e)}` });
    }
  };

  const identifierIntermediaire = async (f: UploadedFileResult) => {
    let liste = intermediaires;
    try {
      liste = await api.getIntermediaires();
      setIntermediaires(liste);
    } catch {
      /* liste courante */
    }

    const type: Intermediaire['type_intermediaire'] = f.file_type === 'CIRIS_PDF' ? 'AGA' : 'COURTIER';
    const prefixe = f.items.find((it) => it.police)?.police?.split('/')[0]?.replace(/\D/g, '') || '';

    // Collecter tous les candidats détectés dans le fichier
    const rawCandidates: { code: string; nom: string }[] = [];
    if (f.code || f.courtier_ou_client) {
      const c = (f.code || '').trim();
      const n = (f.courtier_ou_client || '').trim();
      if (c || n) rawCandidates.push({ code: c, nom: n });
    }
    for (const it of f.items) {
      const c = (it.client_code || '').trim();
      const n = (it.client_nom || '').trim();
      if (c || n) {
        if (!rawCandidates.some((x) => x.code === c && x.nom === n)) {
          rawCandidates.push({ code: c, nom: n });
        }
      }
    }

    // Associer les candidats aux intermédiaires existants en base si trouvés
    const candidates: InterCandidate[] = rawCandidates.map((c) => {
      const match =
        (c.code ? liste.find((i) => i.code.trim().toUpperCase() === c.code.toUpperCase()) : undefined) ||
        (c.nom ? liste.find((i) => i.nom.trim().toUpperCase() === c.nom.toUpperCase()) : undefined) ||
        (prefixe ? liste.find((i) => i.type_intermediaire === type && i.code.replace(/\D/g, '') === prefixe) : undefined);
      return {
        code: c.code || (match ? match.code : prefixe || ''),
        nom: c.nom || (match ? match.nom : c.code || (type === 'AGA' ? 'Agence' : 'Courtier')),
        type,
        source: f.file_name,
        existingId: match?.id,
      };
    });

    // S'il y a un candidat clair
    const primary = candidates[0];
    if (primary) {
      if (primary.existingId) {
        const trouve = liste.find((i) => i.id === primary.existingId);
        if (trouve && trouve.actif) {
          await choisirIntermediaire(trouve);
          setMessage({
            type: 'success',
            text: `Intermédiaire identifié automatiquement : ${trouve.type_intermediaire} ${trouve.code} · ${trouve.nom}.`,
          });
          return;
        }
      } else if (primary.code) {
        try {
          const id = await api.saveIntermediaire(nouvelIntermediaire(type, primary.code, primary.nom || primary.code));
          const maj = await api.getIntermediaires();
          setIntermediaires(maj);
          const cree = maj.find((i) => i.id === id);
          if (cree) {
            await choisirIntermediaire(cree);
            setMessage({
              type: 'success',
              text: `Nouvel intermédiaire créé automatiquement : ${cree.type_intermediaire} ${cree.code} · ${cree.nom} (fiche modifiable dans « Intermédiaires »).`,
            });
            return;
          }
        } catch (e) {
          console.warn('Création auto de l’intermédiaire impossible :', e);
        }
      }
    }

    // Si sélection manuelle / validation requise : modal avec sélection en 1 clic
    const suggestion = prefixe
      ? liste.find((i) => i.actif && i.type_intermediaire === 'COURTIER' && i.code.replace(/\D/g, '') === prefixe)
      : undefined;
    const defaultCode = primary?.code || prefixe || '';
    const defaultNom = primary?.nom || f.courtier_ou_client?.trim() || '';

    setInterModal(
      (prev) =>
        prev ?? {
          mode: candidates.some((c) => c.existingId) || liste.some((i) => i.actif) ? 'choisir' : 'creer',
          raison: candidates.length > 0
            ? `Intermédiaire(s) détecté(s) dans « ${f.file_name} ». Choisissez l'intermédiaire ou sélectionnez-le directement ci-dessous.`
            : `Le fichier « ${f.file_name} » (${FILE_TYPE_LABEL[f.file_type] || f.file_type}) n'indique pas clairement le courtier${
                f.courtier_ou_client ? ` (client : ${f.courtier_ou_client})` : ''
              }. Indiquez le code du courtier concerné.`,
          selectedId: primary?.existingId || suggestion?.id,
          form: nouvelIntermediaire(type, defaultCode, defaultNom),
          candidates,
        }
    );
  };

  const validerInterModal = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!interModal) return;
    try {
      let choisi: Intermediaire | undefined;
      if (interModal.mode === 'choisir') {
        choisi = intermediaires.find((i) => i.id === interModal.selectedId);
        if (!choisi) {
          setMessage({ type: 'error', text: 'Choisissez un intermédiaire dans la liste ou créez-le.' });
          return;
        }
      } else {
        const id = await api.saveIntermediaire(interModal.form);
        const liste = await api.getIntermediaires();
        setIntermediaires(liste);
        choisi = liste.find((i) => i.id === id);
      }
      setInterModal(null);
      if (choisi) await choisirIntermediaire(choisi);
    } catch (err) {
      setMessage({ type: 'error', text: `Intermédiaire non enregistré : ${errMsg(err)}` });
    }
  };

  const handleSelectIntermediaire = (id: number | null) => {
    const i = intermediaires.find((x) => x.id === id);
    if (i && filesRef.current.length > 0) {
      choisirIntermediaire(i);
      return;
    }
    setInterId(id);
    if (i) {
      setTvaTaux(i.tva_applicable);
      setCumul(i.type_intermediaire === 'COURTIER');
    }
  };

  // --- Lignes ---
  const toggleLigne = (index: number) => {
    setBordereau((prev) =>
      prev ? recomputeTotals(prev, prev.lignes.map((l, i) => (i === index ? { ...l, desactive: !l.desactive } : l))) : prev
    );
    setModifie(true);
  };

  // Exclusion / réintégration de polices (encaissement vérifié par le commercial)
  const togglePolices = (keys: string[], exclue: boolean) => {
    const set = new Set(policesExcluesRef.current);
    keys.forEach((k) => (exclue ? set.add(k) : set.delete(k)));
    policesExcluesRef.current = set;
    setBordereau((prev) => (prev ? recomputeTotals(prev, appliquerPolicesExclues(prev.lignes, set)) : prev));
    setModifie(true);
  };

  // --- Paramétrage d'un taux manquant (enregistré en BDD) ---
  const openRateModal = (l: BordereauLigne) => {
    const existing = l.branche_id ? branches.find((b) => b.id === l.branche_id) : undefined;
    if (existing) {
      setRateModal({
        ligne: l,
        mode: 'edit',
        form: { ...existing },
        pct: { courtier: pct(existing.taux_apport_courtier), apportAga: pct(existing.taux_apport_aga), gestionAga: pct(existing.taux_gestion_aga) },
      });
      return;
    }
    const code = l.source_code || l.code_garantie;
    const isAlias = l.source_type === 'BRANCHE_IMPORT';
    setRateModal({
      ligne: l,
      mode: isAlias ? 'alias' : 'create',
      form: {
        code: l.source_code_ministeriel || '',
        branche: l.branche_nom.replace(/\s*\(\s*[\d.]+\)\s*$/, ''),
        sous_code: isAlias ? '' : code,
        sous_libelle: '',
        taux_apport_courtier: 0,
        taux_apport_aga: 0,
        taux_gestion_aga: 0,
        alias_import: isAlias ? code : '',
        groupe_bordereau: '',
        ordre_bordereau: 999,
      },
      pct: { courtier: 0, apportAga: 0, gestionAga: 0 },
    });
  };

  const saveRate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!rateModal) return;
    const code = rateModal.ligne.source_code || rateModal.ligne.code_garantie;
    try {
      let payload: BrancheParam;
      if (rateModal.mode === 'alias' && rateModal.aliasTargetId) {
        const target = branches.find((b) => b.id === rateModal.aliasTargetId)!;
        payload = { ...target, alias_import: [target.alias_import, code].filter((s) => s.trim()).join(',') };
      } else {
        payload = {
          ...rateModal.form,
          taux_apport_courtier: fromPct(rateModal.pct.courtier),
          taux_apport_aga: fromPct(rateModal.pct.apportAga),
          taux_gestion_aga: fromPct(rateModal.pct.gestionAga),
        };
      }
      const tauxUtile = isCourtier ? payload.taux_apport_courtier : payload.taux_apport_aga + payload.taux_gestion_aga;
      if (tauxUtile <= 0) {
        setMessage({
          type: 'error',
          text: `Le taux ${isCourtier ? 'courtier' : 'AGA'} doit être supérieur à 0 %. Si cette garantie ne doit pas être commissionnée, excluez la ligne du bordereau.`,
        });
        return;
      }
      await api.saveBranche(payload);
      const br = await api.getBranches();
      setBranches(br);
      setRateModal(null);
      await recalculate(filesRef.current);
      setMessage({ type: 'success', text: `Paramètre « ${payload.sous_code || payload.alias_import || payload.code} » enregistré en base ; bordereau recalculé.` });
    } catch (err) {
      setMessage({ type: 'error', text: `Paramètre non enregistré : ${errMsg(err)}` });
    }
  };

  // --- Enregistrement ---
  const fichiersBloquants = files.filter((f) => toutesAnomalies(f).some((a) => a.niveau === 'ERREUR'));
  const calculBloquant = bordereau ? anomaliesBloquantes(bordereau.anomalies, bordereau.lignes) : [];
  const lignesNP = bordereau?.lignes.filter((l) => l.non_parametre && ligneActive(l)) || [];
  const peutEnregistrer = !!bordereau && fichiersBloquants.length === 0 && calculBloquant.length === 0 && lignesNP.length === 0;

  const handleSave = async () => {
    if (!bordereau || !peutEnregistrer) return;
    setSaving(true);
    try {
      const wasSaved = !!bordereau.id;
      const id = await api.saveBordereau(bordereau, filesRef.current);
      const saved = await api.getBordereau(id);
      if (saved) setBordereau(saved);
      setModifie(false);
      setMessage({
        type: 'success',
        text: `Bordereau N° ${saved?.numero_bordereau} ${wasSaved ? 'mis à jour' : 'enregistré'} (ID #${id}) avec ${files.length} fichier(s) source rattaché(s).`,
      });
      onBordereauSaved?.(id);
    } catch (err) {
      setMessage({ type: 'error', text: `Enregistrement refusé : ${errMsg(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleExport = async () => {
    if (!bordereau) return;
    try {
      const path = await api.exportExcel(bordereau);
      setMessage({ type: 'success', text: `Bordereau exporté : ${path}` });
    } catch (err) {
      setMessage({ type: 'error', text: `Erreur export Excel : ${errMsg(err)}` });
    }
  };

  const nbExclues = bordereau?.lignes.filter((l) => !ligneActive(l)).length || 0;
  const annees = Array.from({ length: 8 }, (_, i) => new Date().getFullYear() + 1 - i);
  const inputCls =
    'w-full bg-slate-50 border border-slate-200 rounded-lg p-2 font-medium text-slate-800 focus:ring-2 focus:ring-indigo-500 focus:outline-hidden disabled:opacity-60 disabled:cursor-not-allowed';

  const filteredClients =
    excelModal?.scan.clients.filter((c) => {
      const q = excelModal.search.toLowerCase();
      return c.code_client.toLowerCase().includes(q) || c.designation_client.toLowerCase().includes(q);
    }) || [];

  return (
    <div className="space-y-6 pb-12">
      <div className="bg-gradient-to-r from-indigo-900 via-indigo-800 to-slate-900 rounded-2xl p-6 text-white shadow-xl">
        <h1 className="text-2xl font-black tracking-tight">Import & calcul d'un bordereau de commissions</h1>
        <p className="text-sm text-indigo-200 mt-1 max-w-3xl">
          1. Choisissez l'intermédiaire et la période — 2. importez ses fichiers (rapport CIRIS, PDF de flotte, registre Excel) —
          3. contrôlez les anomalies et paramétrez les taux manquants — 4. enregistrez : le bordereau, ses fichiers sources et
          l'intermédiaire restent liés en base.
        </p>
      </div>

      {message && (
        <div
          className={`p-4 rounded-xl text-sm flex items-start gap-3 border ${
            message.type === 'success'
              ? 'bg-emerald-50 border-emerald-200 text-emerald-900'
              : message.type === 'info'
              ? 'bg-sky-50 border-sky-200 text-sky-900'
              : 'bg-rose-50 border-rose-200 text-rose-900'
          }`}
        >
          {message.type === 'success' ? <CheckCircle className="w-5 h-5 shrink-0" /> : <AlertCircle className="w-5 h-5 shrink-0" />}
          <span className="font-medium flex-1">{message.text}</span>
          <button type="button" onClick={() => setMessage(null)} className="cursor-pointer opacity-60 hover:opacity-100">
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Paramètres du bordereau */}
        <div className="bg-white rounded-2xl p-5 border border-slate-200 shadow-xs space-y-3 text-xs">
          <h3 className="text-sm font-bold text-slate-800 flex items-center gap-2">
            <Building2 className="w-4 h-4 text-indigo-600" /> Intermédiaire & période
          </h3>

          {locked && (
            <div className="p-3 rounded-xl bg-amber-50 border border-amber-200 text-amber-900 space-y-2">
              <div className="flex items-center gap-2 font-bold">
                <Lock className="w-3.5 h-3.5" /> Paramètres verrouillés
              </div>
              <p className="text-[11px]">
                Les {files.length} fichier(s) importé(s) et le calcul sont rattachés à ces paramètres. Les modifier annule l'import en cours.
              </p>
              <button
                type="button"
                onClick={demanderModificationParametres}
                className="w-full py-1.5 rounded-lg bg-white border border-amber-300 hover:bg-amber-100 font-bold cursor-pointer"
              >
                Modifier les paramètres…
              </button>
            </div>
          )}

          <div>
            <label className="block font-semibold text-slate-600 mb-1">Intermédiaire *</label>
            <select
              value={interId ?? ''}
              disabled={locked && !!inter}
              onChange={(e) => handleSelectIntermediaire(e.target.value ? Number(e.target.value) : null)}
              className={inputCls}
            >
              <option value="">— Sélectionner un AGA ou un courtier —</option>
              {intermediaires
                .filter((i) => i.actif || i.id === interId)
                .map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.type_intermediaire} · {i.code} · {i.nom}
                  </option>
                ))}
            </select>
            {intermediaires.length === 0 && (
              <p className="text-[11px] text-slate-500 mt-1">Aucun intermédiaire enregistré : il sera créé à partir du premier fichier importé.</p>
            )}
            {inter && (
              <p className="text-[11px] text-slate-500 mt-1">
                {inter.succursale} • {inter.departement} • TVA {(inter.tva_applicable * 100).toFixed(0)} %
              </p>
            )}
          </div>

          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block font-semibold text-slate-600 mb-1">Exercice</label>
              <select value={exercice} disabled={locked} onChange={(e) => setExercice(Number(e.target.value))} className={inputCls}>
                {!annees.includes(exercice) && <option value={exercice}>{exercice}</option>}
                {annees.map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block font-semibold text-slate-600 mb-1">Mois</label>
              <select
                value={mois ?? ''}
                disabled={locked}
                onChange={(e) => setMois(e.target.value ? Number(e.target.value) : undefined)}
                className={inputCls}
              >
                <option value="">Annuel</option>
                {MOIS_LIBELLES.map((m, i) => (
                  <option key={m} value={i + 1}>
                    {String(i + 1).padStart(2, '0')} - {m}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block font-semibold text-slate-600 mb-1">TVA</label>
              <select value={tvaTaux} disabled={locked} onChange={(e) => setTvaTaux(parseFloat(e.target.value))} className={inputCls}>
                {[0.19, 0.09, 0.0, ...(inter && ![0.19, 0.09, 0].includes(inter.tva_applicable) ? [inter.tva_applicable] : [])].map((t) => (
                  <option key={t} value={t}>
                    {t === 0 ? 'Exonéré (0 %)' : `${(t * 100).toFixed(0)} %`}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block font-semibold text-slate-600 mb-1">Filtre client (Excel)</label>
              <input
                type="text"
                value={clientFilter}
                disabled={locked}
                onChange={(e) => setClientFilter(e.target.value)}
                placeholder="Code ou nom"
                className={inputCls}
              />
            </div>
          </div>

          {isCourtier && (
            <label className="flex items-start gap-2 pt-2 border-t border-slate-100 cursor-pointer">
              <input type="checkbox" checked={cumul} disabled={locked} onChange={(e) => setCumul(e.target.checked)} className="mt-0.5" />
              <span>
                <span className="font-bold text-slate-800">Cumuler par groupe du bordereau courtier</span>
                <span className="block text-[10px] text-slate-400">Regroupement défini dans les paramètres (colonne « Groupe bordereau »)</span>
              </span>
            </label>
          )}
        </div>

        {/* Dépôt & fichiers */}
        <div className="lg:col-span-2 space-y-4">
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept=".pdf,.xlsx,.xls"
            className="hidden"
            onChange={async (e) => {
              const list = e.target.files;
              await handleBrowserFiles(list);
              e.target.value = '';
            }}
          />
          <div
            onClick={() => !loading && fileInputRef.current?.click()}
            onDragOver={(e) => {
              e.preventDefault();
              setIsDragging(true);
            }}
            onDragLeave={() => setIsDragging(false)}
            onDrop={async (e) => {
              e.preventDefault();
              setIsDragging(false);
              if (!isTauri()) await handleBrowserFiles(e.dataTransfer.files);
            }}
            className={`border-2 border-dashed rounded-2xl p-7 flex flex-col items-center justify-center text-center min-h-[170px] select-none transition-all ${
              isDragging
                ? 'border-amber-500 bg-amber-500/10 cursor-copy'
                : 'border-indigo-200 hover:border-indigo-400 bg-indigo-50/40 cursor-pointer'
            }`}
          >
            <UploadCloud className="w-8 h-8 text-indigo-600 mb-2" />
            <h3 className="text-sm font-bold text-slate-800">
              {loading
                ? 'Analyse en cours…'
                : inter
                ? `Déposez les fichiers de ${inter.nom} (${periodeLabel(exercice, mois)})`
                : 'Déposez les fichiers (rapport CIRIS, PDF de flotte, registre Excel)'}
            </h3>
            <p className="text-xs text-slate-500 mt-1">
              {inter
                ? 'Chaque fichier est contrôlé puis rattaché à ce bordereau.'
                : "L'intermédiaire est identifié automatiquement (code agence du rapport CIRIS) ou vous sera demandé après l'import."}
            </p>
            {(
              <button
                type="button"
                disabled={loading}
                onClick={(e) => {
                  e.stopPropagation();
                  fileInputRef.current?.click();
                }}
                className="mt-3 text-xs font-bold px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white cursor-pointer flex items-center gap-2 disabled:opacity-50"
              >
                <FolderOpen className="w-4 h-4" /> Parcourir
              </button>
            )}
          </div>

          {files.length > 0 && (
            <div className="bg-white rounded-2xl border border-slate-200 shadow-xs p-4 space-y-2">
              <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-slate-700">
                <Layers className="w-4 h-4 text-indigo-600" /> Fichiers rattachés à ce bordereau ({files.length})
              </div>
              {files.map((f) => (
                <div key={f.id} className="rounded-xl border border-slate-200 bg-slate-50 text-xs">
                  <div className="flex items-center justify-between gap-3 p-3">
                    <div className="min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-bold text-slate-900 truncate">{f.file_name}</span>
                        <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-indigo-100 text-indigo-800 border border-indigo-200">
                          {FILE_TYPE_LABEL[f.file_type] || f.file_type}
                        </span>
                        <AnomaliesBadges anomalies={toutesAnomalies(f)} />
                      </div>
                      <div className="text-[11px] text-slate-500 mt-0.5">
                        {f.courtier_ou_client && <>Client : <strong>{f.courtier_ou_client}</strong> </>}
                        {f.code && <>({f.code}) </>}
                        {f.periode && <>• Période {f.periode} </>}• {f.items.length} lignes
                      </div>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <span className="font-mono font-bold text-slate-900">{formatDZD(f.total_prime_nette)}</span>
                      {toutesAnomalies(f).length > 0 && (
                        <button
                          type="button"
                          onClick={() => setOpenFile(openFile === f.id ? null : f.id)}
                          className="p-1.5 rounded-lg text-slate-500 hover:bg-slate-200 cursor-pointer"
                          title="Détail des contrôles"
                        >
                          {openFile === f.id ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => handleRemoveFile(f)}
                        className="p-1.5 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 cursor-pointer"
                        title="Retirer ce fichier"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                  {openFile === f.id && (
                    <div className="px-3 pb-3">
                      <AnomaliesPanel anomalies={toutesAnomalies(f)} />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Fichiers importés, intermédiaire encore inconnu */}
      {files.length > 0 && !inter && (
        <div className="bg-amber-50 border-2 border-amber-300 p-4 rounded-2xl flex flex-wrap items-center justify-between gap-3">
          <div className="text-xs text-amber-950">
            <div className="font-black text-sm flex items-center gap-2">
              <AlertTriangle className="w-5 h-5 text-amber-600" /> Intermédiaire à identifier
            </div>
            <p className="mt-0.5">
              {files.length} fichier(s) importé(s). Indiquez l'AGA ou le courtier concerné pour calculer le bordereau.
            </p>
          </div>
          <button
            type="button"
            onClick={() => identifierIntermediaire(filesRef.current[0])}
            className="px-4 py-2.5 rounded-xl bg-amber-500 hover:bg-amber-400 text-slate-950 text-xs font-bold flex items-center gap-2 cursor-pointer"
          >
            <UserPlus className="w-4 h-4" /> Indiquer l'intermédiaire
          </button>
        </div>
      )}

      {/* Résultat du calcul */}
      {bordereau && (
        <div className="space-y-5">
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
            {[
              ['Primes nettes', bordereau.total_prime_nette, 'text-slate-900'],
              ['Commission apport', bordereau.total_commission_apport, 'text-indigo-600'],
              ['Commission gestion', bordereau.total_commission_gestion, 'text-sky-600'],
              ['Commission H.T', bordereau.total_commission_ht, 'text-indigo-950'],
              [`TVA (${(bordereau.tva_taux * 100).toFixed(0)} %)`, bordereau.total_tva, 'text-slate-800'],
              ['Commission T.T.C', bordereau.total_commission_ttc, 'text-emerald-700'],
            ].map(([label, val, cls]) => (
              <div key={label as string} className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs">
                <span className="text-[11px] font-semibold text-slate-500 uppercase tracking-wider">{label}</span>
                <p className={`text-base font-extrabold mt-1 ${cls}`}>{formatDZD(val as number)}</p>
              </div>
            ))}
          </div>

          <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-xs flex flex-wrap items-center justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <h4 className="text-sm font-bold text-slate-900">
                  {bordereau.numero_bordereau} — {bordereau.intermediaire_nom}
                </h4>
                {bordereau.id ? <StatutBadge statut={bordereau.statut} /> : <span className="text-[10px] font-bold text-amber-700">NON ENREGISTRÉ</span>}
                {bordereau.id && modifie && <span className="text-[10px] font-bold text-amber-700">modifié depuis l'enregistrement</span>}
              </div>
              <p className="text-xs text-slate-500">
                {periodeLabel(bordereau.exercice, bordereau.mois)} • {bordereau.lignes.length} lignes ({nbExclues} exclue(s)) • {files.length} fichier(s)
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={handleSave}
                disabled={saving || !peutEnregistrer || (!!bordereau.id && !modifie)}
                title={peutEnregistrer ? '' : 'Corrigez les points bloquants listés ci-dessous'}
                className="px-4 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-700 disabled:opacity-40 disabled:cursor-not-allowed text-white text-xs font-bold flex items-center gap-2 cursor-pointer"
              >
                <Save className="w-4 h-4" />
                {saving ? 'Enregistrement…' : bordereau.id ? 'Enregistrer les modifications' : 'Enregistrer le bordereau'}
              </button>
              <button
                type="button"
                onClick={handleExport}
                className="px-4 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold flex items-center gap-2 cursor-pointer"
              >
                <FileSpreadsheet className="w-4 h-4" /> Exporter Excel
              </button>
              <button
                type="button"
                onClick={demanderModificationParametres}
                className="px-4 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-2 cursor-pointer"
              >
                <FilePlus2 className="w-4 h-4" /> Nouvel import
              </button>
            </div>
          </div>

          {!peutEnregistrer && (
            <div className="bg-rose-50 border-2 border-rose-300 p-4 rounded-2xl space-y-3">
              <div className="flex items-center gap-2 font-black text-sm text-rose-950">
                <AlertTriangle className="w-5 h-5 text-rose-600" /> Enregistrement impossible tant que ces points ne sont pas réglés
              </div>
              {fichiersBloquants.map((f) => (
                <AnomaliesPanel key={f.id} title={`Fichier ${f.file_name}`} anomalies={toutesAnomalies(f).filter((a) => a.niveau === 'ERREUR')} />
              ))}
              {lignesNP.length > 0 && (
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span className="font-bold text-rose-900">{lignesNP.length} ligne(s) sans taux paramétré :</span>
                  {lignesNP.map((l) => (
                    <button
                      key={ligneKey(l)}
                      type="button"
                      onClick={() => openRateModal(l)}
                      className="px-2.5 py-1 rounded-lg bg-rose-600 hover:bg-rose-700 text-white font-bold flex items-center gap-1 cursor-pointer"
                    >
                      <Settings className="w-3 h-3" /> Paramétrer {l.source_code || l.code_garantie}
                    </button>
                  ))}
                  <span className="text-rose-800">ou excluez la ligne du bordereau.</span>
                </div>
              )}
              {calculBloquant.filter((a) => !a.ligne).length > 0 && (
                <AnomaliesPanel title="Calcul" anomalies={calculBloquant.filter((a) => !a.ligne)} />
              )}
            </div>
          )}

          {(bordereau.anomalies || []).length > 0 && (
            <details className="bg-white rounded-xl border border-slate-200 p-4">
              <summary className="text-xs font-bold text-slate-700 cursor-pointer flex items-center gap-2">
                <Info className="w-4 h-4 text-sky-600" /> Contrôles du calcul ({(bordereau.anomalies || []).length})
              </summary>
              <div className="mt-3">
                <AnomaliesPanel anomalies={bordereau.anomalies || []} lignes={bordereau.lignes} />
              </div>
            </details>
          )}

          <PolicesPanel lignes={bordereau.lignes} onToggle={togglePolices} />

          <div className="bg-white rounded-2xl border border-slate-200 shadow-xs overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead>
                  <tr className="bg-slate-50 border-b border-slate-200 text-slate-700 font-bold uppercase tracking-wider">
                    <th className="px-4 py-3">Branche / garantie</th>
                    <th className="px-4 py-3 text-right">Prime nette</th>
                    <th className="px-4 py-3 text-center">Taux apport</th>
                    {!isCourtier && <th className="px-4 py-3 text-center">Taux gestion</th>}
                    <th className="px-4 py-3 text-right">Commission H.T</th>
                    <th className="px-4 py-3 text-right">TVA</th>
                    <th className="px-4 py-3 text-right">Commission T.T.C</th>
                    <th className="px-4 py-3 text-center">Actions</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {bordereau.lignes.map((l, index) => {
                    const exclue = !ligneActive(l);
                    const np = !exclue && !!l.non_parametre;
                    const nbPolicesExclues = (l.contributions || []).filter((c) => c.exclue).length;
                    const open = expanded.has(index);
                    const strike = exclue ? 'line-through text-slate-400' : '';
                    return (
                      <React.Fragment key={index}>
                        <tr className={exclue ? 'bg-slate-100/80' : np ? 'bg-rose-50/80 border-l-4 border-rose-500' : 'hover:bg-slate-50'}>
                          <td className="px-4 py-2.5">
                            <div className={`font-bold ${strike || 'text-slate-900'}`}>{l.libelle_garantie}</div>
                            <div className="text-[10px] text-slate-400">
                              {l.branche_nom} • {l.contributions?.length || 0} contribution(s)
                              {l.desactive && ' • exclue du calcul'}
                              {!l.desactive && exclue && ' • toutes les polices exclues (non encaissées)'}
                              {!exclue && nbPolicesExclues > 0 && (
                                <span className="text-rose-700 font-bold"> • {nbPolicesExclues} contribution(s) non encaissée(s) exclue(s)</span>
                              )}
                              {np && <span className="text-rose-700 font-bold"> • aucun taux paramétré</span>}
                            </div>
                          </td>
                          <td className={`px-4 py-2.5 text-right font-mono ${strike}`}>{formatMontant(l.prime_nette)}</td>
                          <td className={`px-4 py-2.5 text-center font-bold ${strike || (np ? 'text-rose-700' : 'text-indigo-700')}`}>
                            {(l.taux_apport * 100).toFixed(2)} %
                          </td>
                          {!isCourtier && (
                            <td className={`px-4 py-2.5 text-center font-bold ${strike || 'text-sky-700'}`}>{(l.taux_gestion * 100).toFixed(2)} %</td>
                          )}
                          <td className={`px-4 py-2.5 text-right font-mono font-bold ${strike}`}>{formatMontant(l.montant_ht)}</td>
                          <td className={`px-4 py-2.5 text-right font-mono ${strike}`}>{formatMontant(l.montant_tva)}</td>
                          <td className={`px-4 py-2.5 text-right font-mono font-extrabold ${strike || 'text-emerald-800'}`}>{formatMontant(l.montant_ttc)}</td>
                          <td className="px-4 py-2.5">
                            <div className="flex items-center justify-center gap-1.5">
                              {np && (
                                <button
                                  type="button"
                                  onClick={() => openRateModal(l)}
                                  className="px-2 py-1 rounded-lg bg-rose-600 hover:bg-rose-700 text-white font-bold text-[11px] flex items-center gap-1 cursor-pointer"
                                >
                                  <Settings className="w-3 h-3" /> Paramétrer
                                </button>
                              )}
                              <button
                                type="button"
                                onClick={() => toggleLigne(index)}
                                className={`px-2 py-1 rounded-lg text-[11px] font-bold flex items-center gap-1 cursor-pointer ${
                                  l.desactive ? 'bg-emerald-600 text-white' : 'bg-slate-100 text-slate-600 hover:text-rose-700 border border-slate-200'
                                }`}
                              >
                                {l.desactive ? <Eye className="w-3.5 h-3.5" /> : <EyeOff className="w-3.5 h-3.5" />}
                                {l.desactive ? 'Réintégrer' : 'Exclure la ligne'}
                              </button>
                              <button
                                type="button"
                                onClick={() =>
                                  setExpanded((prev) => {
                                    const n = new Set(prev);
                                    if (n.has(index)) n.delete(index);
                                    else n.add(index);
                                    return n;
                                  })
                                }
                                className="p-1 rounded-lg text-slate-500 hover:bg-slate-100 cursor-pointer"
                                title="Détail des contributions"
                              >
                                {open ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                              </button>
                            </div>
                          </td>
                        </tr>
                        {open && (
                          <tr className="bg-indigo-50/20">
                            <td colSpan={isCourtier ? 7 : 8} className="px-6 py-3">
                              <div className="grid grid-cols-1 md:grid-cols-2 gap-2 max-h-56 overflow-y-auto text-[11px]">
                                {(l.contributions || []).map((c, i) => (
                                  <div
                                    key={i}
                                    className={`flex items-center justify-between p-2 rounded-lg border ${
                                      c.exclue ? 'bg-rose-50 border-rose-200 line-through text-slate-400' : 'bg-white border-slate-200'
                                    }`}
                                  >
                                    <span className="truncate">
                                      <strong>{c.fichier_source}</strong>
                                      {c.police && <span className="text-indigo-700"> • N° {c.police}</span>}
                                      {c.client && <span className="text-slate-500"> • {c.client}</span>}
                                    </span>
                                    <span className="font-mono font-bold whitespace-nowrap">{formatMontant(c.montant)}</span>
                                  </div>
                                ))}
                              </div>
                            </td>
                          </tr>
                        )}
                      </React.Fragment>
                    );
                  })}
                </tbody>
                <tfoot>
                  <tr className="bg-slate-100 border-t-2 border-slate-300 font-black text-slate-900">
                    <td className="px-4 py-3 uppercase">Total ({nbExclues ? 'hors lignes exclues' : 'général'})</td>
                    <td className="px-4 py-3 text-right font-mono">{formatMontant(bordereau.total_prime_nette)}</td>
                    <td />
                    {!isCourtier && <td />}
                    <td className="px-4 py-3 text-right font-mono">{formatMontant(bordereau.total_commission_ht)}</td>
                    <td className="px-4 py-3 text-right font-mono">{formatMontant(bordereau.total_tva)}</td>
                    <td className="px-4 py-3 text-right font-mono text-emerald-900">{formatDZD(bordereau.total_commission_ttc)}</td>
                    <td />
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Choix du client dans un registre Excel multi-clients */}
      {excelModal && (
        <div className="fixed inset-0 z-50 bg-slate-900/60 flex items-center justify-center p-4">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-2xl max-h-[85vh] flex flex-col overflow-hidden">
            <div className="px-6 py-4 flex items-center justify-between bg-gradient-to-r from-indigo-900 to-slate-900 text-white">
              <div className="flex items-center gap-2 font-bold">
                <UserCheck className="w-5 h-5 text-amber-400" /> {excelModal.scan.clients.length} clients dans « {excelModal.scan.file_name} »
              </div>
              <button type="button" onClick={() => setExcelModal(null)} className="cursor-pointer">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-4 border-b border-slate-100 relative">
              <Search className="w-4 h-4 text-slate-400 absolute left-7 top-1/2 -translate-y-1/2" />
              <input
                autoFocus
                value={excelModal.search}
                onChange={(e) => setExcelModal({ ...excelModal, search: e.target.value })}
                placeholder="Rechercher un client (nom ou code)…"
                className="w-full border border-slate-200 rounded-xl pl-9 pr-4 py-2.5 text-xs"
              />
              <p className="text-[11px] text-slate-500 mt-2">
                Un client par code (polices regroupées). Double-cliquez sur un client : il devient l'intermédiaire du bordereau.
              </p>
            </div>
            <div className="overflow-y-auto flex-1 p-4 divide-y divide-slate-100">
              {filteredClients.slice(0, 100).map((c) => (
                <div
                  key={c.code_client}
                  onDoubleClick={() => handleSelectClient(c)}
                  title="Double-cliquez pour sélectionner"
                  className="py-2.5 px-2 rounded-lg flex items-center justify-between text-xs cursor-pointer hover:bg-indigo-50 select-none"
                >
                  <div>
                    <div className="font-bold text-slate-900">{c.designation_client || 'Sans désignation'}</div>
                    <div className="text-slate-500 font-mono">
                      {c.code_client} • {c.nombre_polices} police(s)
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <span className="font-mono font-bold">{formatDZD(c.total_prime_nette)}</span>
                    <button
                      type="button"
                      onClick={() => handleSelectClient(c)}
                      className="px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white font-bold cursor-pointer"
                    >
                      Sélectionner
                    </button>
                  </div>
                </div>
              ))}
              {filteredClients.length === 0 && <div className="py-10 text-center text-slate-400 text-xs">Aucun client trouvé.</div>}
            </div>
          </div>
        </div>
      )}

      {/* Paramétrage d'un taux manquant */}
      {rateModal && (
        <div className="fixed inset-0 z-50 bg-slate-900/60 flex items-center justify-center p-4">
          <form onSubmit={saveRate} className="bg-white rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden text-xs">
            <div className="px-6 py-4 flex items-center justify-between bg-gradient-to-r from-rose-900 to-slate-900 text-white">
              <div className="font-extrabold text-sm flex items-center gap-2">
                <AlertTriangle className="w-4 h-4" /> Paramétrer « {rateModal.ligne.source_code || rateModal.ligne.code_garantie} »
              </div>
              <button type="button" onClick={() => setRateModal(null)} className="cursor-pointer">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-6 space-y-3">
              <div className="p-3 bg-rose-50 rounded-xl border border-rose-200 space-y-1">
                <div>
                  Origine : <strong>{rateModal.ligne.source_type === 'BRANCHE_IMPORT' ? 'code branche du registre Excel' : 'code garantie'}</strong>{' '}
                  {rateModal.ligne.source_code_ministeriel && <>• code ministériel {rateModal.ligne.source_code_ministeriel}</>}
                </div>
                <div>
                  Branche déclarée : <strong>{rateModal.ligne.branche_nom}</strong>
                </div>
                <div>
                  Prime concernée : <strong>{formatDZD(rateModal.ligne.prime_nette)}</strong>
                </div>
                <div className="text-[11px] text-rose-800">
                  Le taux sera enregistré dans les paramètres (BDD) et servira à tous les prochains calculs.
                </div>
              </div>

              {rateModal.mode === 'alias' && (
                <div>
                  <label className="block font-bold text-slate-700 mb-1">Rattacher le code « {rateModal.ligne.source_code} » à un paramètre existant</label>
                  <select
                    value={rateModal.aliasTargetId ?? ''}
                    onChange={(e) => setRateModal({ ...rateModal, aliasTargetId: e.target.value ? Number(e.target.value) : undefined })}
                    className={inputCls}
                  >
                    <option value="">— Non : créer un nouveau paramètre ci-dessous —</option>
                    {branches.map((b) => (
                      <option key={b.id} value={b.id}>
                        {b.code} {b.sous_code && `· ${b.sous_code}`} · {b.sous_libelle || b.branche} ({(b.taux_apport_courtier * 100).toFixed(1)} % / {(b.taux_apport_aga * 100).toFixed(1)} % + {(b.taux_gestion_aga * 100).toFixed(1)} %)
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {!(rateModal.mode === 'alias' && rateModal.aliasTargetId) && (
                <>
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Code branche (ministériel) *</label>
                      <input required value={rateModal.form.code} onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, code: e.target.value } })} className={inputCls} />
                    </div>
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Nom de la branche *</label>
                      <input required value={rateModal.form.branche} onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, branche: e.target.value } })} className={inputCls} />
                    </div>
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Sous-code garantie</label>
                      <input
                        value={rateModal.form.sous_code}
                        readOnly={rateModal.mode === 'create'}
                        onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, sous_code: e.target.value.toUpperCase() } })}
                        className={`${inputCls} font-mono`}
                      />
                    </div>
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Libellé</label>
                      <input value={rateModal.form.sous_libelle} onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, sous_libelle: e.target.value } })} className={inputCls} />
                    </div>
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Alias d'import</label>
                      <input value={rateModal.form.alias_import} onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, alias_import: e.target.value.toUpperCase() } })} className={`${inputCls} font-mono`} />
                    </div>
                    <div>
                      <label className="block font-bold text-slate-700 mb-1">Groupe bordereau courtier</label>
                      <input value={rateModal.form.groupe_bordereau} onChange={(e) => setRateModal({ ...rateModal, form: { ...rateModal.form, groupe_bordereau: e.target.value } })} className={inputCls} placeholder="ex: RC AUTOMOBILE" />
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-2 border-t border-slate-100 pt-3">
                    {(
                      [
                        ['courtier', 'Apport courtier %'],
                        ['apportAga', 'Apport AGA %'],
                        ['gestionAga', 'Gestion AGA %'],
                      ] as const
                    ).map(([k, label]) => (
                      <div key={k}>
                        <label className="block font-bold text-slate-700 mb-1">{label}</label>
                        <input
                          type="number"
                          step="0.01"
                          min="0"
                          max="100"
                          required
                          value={rateModal.pct[k]}
                          onChange={(e) => setRateModal({ ...rateModal, pct: { ...rateModal.pct, [k]: parseFloat(e.target.value) || 0 } })}
                          className={`${inputCls} text-center font-bold`}
                        />
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
            <div className="px-6 py-3 bg-slate-50 border-t border-slate-100 flex justify-end gap-2">
              <button type="button" onClick={() => setRateModal(null)} className="px-4 py-2 rounded-xl bg-white border border-slate-300 font-bold cursor-pointer">
                Annuler
              </button>
              <button type="submit" className="px-4 py-2 rounded-xl bg-rose-600 hover:bg-rose-700 text-white font-bold cursor-pointer flex items-center gap-1">
                <RotateCcw className="w-3.5 h-3.5" /> Enregistrer en base & recalculer
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Identification de l'intermédiaire après import */}
      {interModal && (
        <div className="fixed inset-0 z-50 bg-slate-900/60 flex items-center justify-center p-4">
          <form onSubmit={validerInterModal} className="bg-white rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden text-xs">
            <div className="px-6 py-4 flex items-center justify-between bg-gradient-to-r from-indigo-900 to-slate-900 text-white">
              <div className="font-extrabold text-sm flex items-center gap-2">
                <UserPlus className="w-4 h-4" /> Intermédiaire de cet import
              </div>
              <button type="button" onClick={() => setInterModal(null)} className="cursor-pointer">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-6 space-y-3">
              <p className="p-3 rounded-xl bg-amber-50 border border-amber-200 text-amber-950">{interModal.raison}</p>

              {/* Intermédiaires détectés dans le fichier : sélection directe en 1 clic */}
              {interModal.candidates && interModal.candidates.length > 0 && (
                <div className="space-y-2 p-3 bg-indigo-50/60 border border-indigo-200 rounded-xl">
                  <div className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                    <CheckCircle className="w-4 h-4 text-emerald-600" />
                    Intermédiaire(s) détecté(s) dans le fichier :
                  </div>
                  <div className="space-y-2 max-h-48 overflow-y-auto pr-1">
                    {interModal.candidates.map((c, idx) => (
                      <div
                        key={idx}
                        className="p-2.5 bg-white border border-indigo-100 rounded-lg flex items-center justify-between shadow-sm hover:border-indigo-300 transition-colors"
                      >
                        <div>
                          <div className="font-bold text-slate-900">{c.nom || 'Sans nom'}</div>
                          <div className="text-[11px] text-slate-500 font-mono flex items-center gap-2">
                            <span>Code : {c.code || 'Non spécifié'}</span>
                            <span>•</span>
                            <span>{c.type}</span>
                            {c.existingId ? (
                              <span className="px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-800 font-sans text-[10px] font-bold">
                                Existant en base
                              </span>
                            ) : (
                              <span className="px-1.5 py-0.5 rounded bg-amber-100 text-amber-800 font-sans text-[10px] font-bold">
                                Nouveau
                              </span>
                            )}
                          </div>
                        </div>
                        <button
                          type="button"
                          onClick={() => handleSelectCandidate(c)}
                          className="px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white font-bold cursor-pointer text-xs flex items-center gap-1"
                        >
                          Sélectionner
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="grid grid-cols-2 gap-2">
                {(
                  [
                    ['choisir', 'Intermédiaire existant'],
                    ['creer', 'Nouvel intermédiaire'],
                  ] as const
                ).map(([m, label]) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => setInterModal({ ...interModal, mode: m })}
                    className={`py-2 rounded-lg font-bold border cursor-pointer ${
                      interModal.mode === m ? 'bg-indigo-600 border-indigo-600 text-white' : 'bg-slate-50 border-slate-200 text-slate-700'
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>

              {interModal.mode === 'choisir' ? (
                <div>
                  <label className="block font-bold text-slate-700 mb-1">Code de l'intermédiaire *</label>
                  <select
                    value={interModal.selectedId ?? ''}
                    onChange={(e) => setInterModal({ ...interModal, selectedId: e.target.value ? Number(e.target.value) : undefined })}
                    className={inputCls}
                  >
                    <option value="">— Choisir —</option>
                    {intermediaires
                      .filter((i) => i.actif)
                      .map((i) => (
                        <option key={i.id} value={i.id}>
                          {i.code} · {i.nom} ({i.type_intermediaire})
                        </option>
                      ))}
                  </select>
                </div>
              ) : (
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="block font-bold text-slate-700 mb-1">Type *</label>
                    <select
                      value={interModal.form.type_intermediaire}
                      onChange={(e) =>
                        setInterModal({ ...interModal, form: { ...interModal.form, type_intermediaire: e.target.value as Intermediaire['type_intermediaire'] } })
                      }
                      className={inputCls}
                    >
                      <option value="AGA">AGA</option>
                      <option value="COURTIER">Courtier</option>
                    </select>
                  </div>
                  <div>
                    <label className="block font-bold text-slate-700 mb-1">Code *</label>
                    <input
                      required
                      value={interModal.form.code}
                      onChange={(e) => setInterModal({ ...interModal, form: { ...interModal.form, code: e.target.value } })}
                      className={`${inputCls} font-mono`}
                    />
                  </div>
                  <div className="col-span-2">
                    <label className="block font-bold text-slate-700 mb-1">Nom *</label>
                    <input
                      required
                      value={interModal.form.nom}
                      onChange={(e) => setInterModal({ ...interModal, form: { ...interModal.form, nom: e.target.value } })}
                      className={inputCls}
                    />
                  </div>
                  <div>
                    <label className="block font-bold text-slate-700 mb-1">Succursale *</label>
                    <input
                      required
                      value={interModal.form.succursale}
                      onChange={(e) => setInterModal({ ...interModal, form: { ...interModal.form, succursale: e.target.value } })}
                      className={inputCls}
                    />
                  </div>
                  <div>
                    <label className="block font-bold text-slate-700 mb-1">Département *</label>
                    <input
                      required
                      value={interModal.form.departement}
                      onChange={(e) => setInterModal({ ...interModal, form: { ...interModal.form, departement: e.target.value } })}
                      className={inputCls}
                    />
                  </div>
                  <div>
                    <label className="block font-bold text-slate-700 mb-1">TVA</label>
                    <select
                      value={interModal.form.tva_applicable}
                      onChange={(e) => setInterModal({ ...interModal, form: { ...interModal.form, tva_applicable: parseFloat(e.target.value) } })}
                      className={inputCls}
                    >
                      <option value={0.19}>19 %</option>
                      <option value={0.09}>9 %</option>
                      <option value={0}>Exonéré (0 %)</option>
                    </select>
                  </div>
                </div>
              )}
            </div>
            <div className="px-6 py-3 bg-slate-50 border-t border-slate-100 flex justify-end gap-2">
              <button type="button" onClick={() => setInterModal(null)} className="px-4 py-2 rounded-xl bg-white border border-slate-300 font-bold cursor-pointer">
                Plus tard
              </button>
              <button type="submit" className="px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white font-bold cursor-pointer">
                {interModal.mode === 'creer' ? 'Enregistrer et rattacher' : 'Rattacher cet intermédiaire'}
              </button>
            </div>
          </form>
        </div>
      )}

      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
    </div>
  );
};
