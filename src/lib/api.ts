import { BrancheParam, Intermediaire, Bordereau, CirisPdfExtractedData } from '../types';

// Vérification de l'environnement Tauri
export const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

// Wrapper sécurisé invoke
async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  console.warn(`[Web Fallback] Mocking Tauri IPC call for command: ${cmd}`);
  return mockTauriCalls<T>(cmd, args);
}

// Mocks complets et interactifs si exécuté dans un navigateur Web standard
let mockBranches: BrancheParam[] = [];
let mockIntermediaires: Intermediaire[] = [
  {
    id: 1,
    type_intermediaire: 'AGA',
    code: '492',
    nom: 'AGA ABDELLAH',
    succursale: 'SUCCURSALE SUD',
    departement: 'Département Commercial',
    contact: '0550 00 00 00',
    tva_applicable: 0.19,
    actif: true,
  },
  {
    id: 2,
    type_intermediaire: 'COURTIER',
    code: 'CRT-01',
    nom: 'Cabinet Assurances Sud Courtage',
    succursale: 'SUCCURSALE SUD',
    departement: 'Département Commercial',
    contact: '0555 12 34 56',
    tva_applicable: 0.19,
    actif: true,
  },
];

let mockBordereaux: Bordereau[] = [];

async function mockTauriCalls<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Simuler 100ms de latence pour réalisme
  await new Promise((r) => setTimeout(r, 80));

  switch (cmd) {
    case 'get_db_info':
      return {
        db_path: './BDD/commissions.db',
        version: 1,
        total_branches: 70,
        total_intermediaires: mockIntermediaires.length,
        total_bordereaux: mockBordereaux.length,
        last_backup: 'backup_commissions_20260920.db',
      } as T;

    case 'backup_database':
      return './BDD/backups/backup_commissions_manual.db' as T;

    case 'get_branches': {
      if (mockBranches.length === 0) {
        // Charger initial_rates.json
        try {
          const res = await fetch('/initial_rates.json');
          if (res.ok) {
            mockBranches = await res.json();
          }
        } catch {
          // fallback statique
        }
      }
      return mockBranches as T;
    }

    case 'update_branche': {
      const b = args?.branche as BrancheParam;
      const idx = mockBranches.findIndex((item) => item.id === b.id || (item.code === b.code && item.sous_code === b.sous_code));
      if (idx !== -1) {
        mockBranches[idx] = b;
      }
      return true as T;
    }

    case 'get_intermediaires':
      return mockIntermediaires as T;

    case 'save_intermediaire': {
      const item = args?.item as Intermediaire;
      if (item.id) {
        const idx = mockIntermediaires.findIndex((i) => i.id === item.id);
        if (idx !== -1) mockIntermediaires[idx] = item;
        return item.id as T;
      } else {
        const newId = mockIntermediaires.length + 1;
        mockIntermediaires.push({ ...item, id: newId });
        return newId as T;
      }
    }

    case 'parse_ciris_pdf':
    case 'parse_ciris_pdf_base64': {
      // Retourner les données certifiées du rapport 05-2025.PDF
      return getMock05_2025Data() as T;
    }

    case 'calculate_bordereau_from_pdf': {
      const extracted = args?.extracted_data as CirisPdfExtractedData;
      const typeInter = (args?.type_intermediaire as string) || 'AGA';
      const tvaTaux = (args?.tva_taux as number) ?? 0.19;
      const succursale = (args?.succursale as string) || 'SUCCURSALE SUD';
      const departement = (args?.departement as string) || 'Département Commercial';

      return calculateMockBordereau(extracted, typeInter, tvaTaux, succursale, departement) as T;
    }

    case 'save_bordereau': {
      const b = args?.bordereau as Bordereau;
      const newId = b.id || mockBordereaux.length + 1;
      const saved = { ...b, id: newId };
      const idx = mockBordereaux.findIndex((x) => x.id === newId);
      if (idx !== -1) mockBordereaux[idx] = saved;
      else mockBordereaux.unshift(saved);
      return newId as T;
    }

    case 'get_bordereaux':
      return mockBordereaux as T;

    case 'get_bordereau': {
      const id = args?.id as number;
      const found = mockBordereaux.find((b) => b.id === id) || null;
      return found as T;
    }

    case 'delete_bordereau': {
      const id = args?.id as number;
      mockBordereaux = mockBordereaux.filter((b) => b.id !== id);
      return true as T;
    }

    case 'export_excel':
      return 'Bordereau_Export.xlsx' as T;

    default:
      throw new Error(`Commande inconnue: ${cmd}`);
  }
}

export function getMock05_2025Data(): CirisPdfExtractedData {
  return {
    agence_code: '492',
    agence_nom: 'AGA ABDELLAH',
    periode_debut: '01/05/2025',
    periode_fin: '31/05/2025',
    mois: 5,
    exercice: 2025,
    devise: 'DZD Dinars Algérien',
    total_pdf_primes_nettes: 2272025.22,
    items: [
      { garantie: 'CANAT', description: 'Catastrophe Naturelle', branche_code: 'CN', branche_libelle: 'CATASTROPHES NATURELLES STANDARD ( 8.4)', prime_nette: 29500.00 },
      { garantie: 'RVAC', description: 'Accidents Caractérisés', branche_code: 'RV', branche_libelle: 'RC VOITURIER (RC DU TRANSPORTEUR) ( 10.2.0.0)', prime_nette: 22220.00 },
      { garantie: 'VDR', description: 'Défense Et Recours (Entreprise)', branche_code: 'VF', branche_libelle: 'AUTOMOBILE ENTREPRISE ( 3.1.0.2)', prime_nette: 10800.00 },
      { garantie: 'VPT', description: 'Personnes Transportées (Entreprise)', branche_code: 'VF', branche_libelle: 'AUTOMOBILE ENTREPRISE ( 3.1.0.2)', prime_nette: 1752.00 },
      { garantie: 'VRC', description: 'Responsabilité Civile (Entreprise)', branche_code: 'VF', branche_libelle: 'AUTOMOBILE ENTREPRISE ( 3.1.0.2)', prime_nette: 86274.90 },
      { garantie: 'VRCE', description: 'Responsabilité Civile à L\'étranger', branche_code: 'VO', branche_libelle: 'Assurance Extension De Territoire ( 3.1.0.3)', prime_nette: 9975.75 },
      { garantie: 'VASS', description: 'Assistance', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 74837.66 },
      { garantie: 'VBDG', description: 'Bris De Glaces', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 96235.00 },
      { garantie: 'VDC', description: 'Dommages Collisions', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 998264.99 },
      { garantie: 'VDR', description: 'Défense Et Recours', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 101055.00 },
      { garantie: 'VINV', description: 'Incendie de Véhicule', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 2000.00 },
      { garantie: 'VOLV', description: 'Vol de Véhicule', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 3000.00 },
      { garantie: 'VPT', description: 'Personnes Transportées', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 69267.02 },
      { garantie: 'VRC', description: 'Responsabilité Civile', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 738286.10 },
      { garantie: 'VRCR', description: 'Responsabilité Civile Remorque', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 3733.59 },
      { garantie: 'VTR', description: 'Tous Risques', branche_code: 'VP', branche_libelle: 'AUTOMOBILE PARTICULIER ( 3.1.0.1)', prime_nette: 12301.21 },
      { garantie: 'VRC', description: 'Responsabilité Civile (Frontière)', branche_code: 'VR', branche_libelle: 'ASSURANCE FRONTIERE ( 3.1.0.1)', prime_nette: 12522.00 },
    ],
  };
}

function calculateMockBordereau(
  extracted: CirisPdfExtractedData,
  typeInter: string,
  tvaTaux: number,
  succursale: string,
  departement: string
): Bordereau {
  const ratesMap: Record<string, { app: number; gest: number }> = {
    CANAT: { app: 0.05, gest: 0.05 },
    RVAC: { app: 0.09, gest: 0.04 },
    VASS: { app: 0.075, gest: 0.0 },
    VBDG: { app: 0.1, gest: 0.05 },
    VDC: { app: 0.1, gest: 0.05 },
    VDR: { app: 0.1, gest: 0.05 },
    VINV: { app: 0.1, gest: 0.05 },
    VOLV: { app: 0.1, gest: 0.05 },
    VPT: { app: 0.1, gest: 0.05 },
    VRC: { app: 0.05, gest: 0.05 },
    VRCR: { app: 0.05, gest: 0.05 },
    VRCE: { app: 0.05, gest: 0.05 },
    VTR: { app: 0.1, gest: 0.05 },
  };

  let totPrime = 0;
  let totApp = 0;
  let totGest = 0;

  const lignes = extracted.items.map((item, idx) => {
    const r = ratesMap[item.garantie] || { app: 0.05, gest: 0.05 };
    const t_app = r.app;
    const t_gest = typeInter === 'COURTIER' ? 0.0 : r.gest;

    const m_app = Math.round(item.prime_nette * t_app * 100) / 100;
    const m_gest = Math.round(item.prime_nette * t_gest * 100) / 100;
    const m_ht = Math.round((m_app + m_gest) * 100) / 100;
    const m_tva = Math.round(m_ht * tvaTaux * 100) / 100;
    const m_ttc = Math.round((m_ht + m_tva) * 100) / 100;

    totPrime += item.prime_nette;
    totApp += m_app;
    totGest += m_gest;

    return {
      id: idx + 1,
      code_garantie: item.garantie,
      libelle_garantie: item.description,
      branche_nom: item.branche_libelle,
      prime_nette: item.prime_nette,
      taux_apport: t_app,
      taux_gestion: t_gest,
      montant_apport: m_app,
      montant_gestion: m_gest,
      montant_ht: m_ht,
      montant_tva: m_tva,
      montant_ttc: m_ttc,
    };
  });

  totPrime = Math.round(totPrime * 100) / 100;
  totApp = Math.round(totApp * 100) / 100;
  totGest = Math.round(totGest * 100) / 100;
  const totHt = Math.round((totApp + totGest) * 100) / 100;
  const totTva = Math.round(totHt * tvaTaux * 100) / 100;
  const totTtc = Math.round((totHt + totTva) * 100) / 100;

  return {
    intermediaire_nom: extracted.agence_nom,
    intermediaire_code: extracted.agence_code,
    type_intermediaire: typeInter as any,
    numero_bordereau: `BORD-${typeInter}-${extracted.exercice}-${String(extracted.mois).padStart(2, '0')}`,
    exercice: extracted.exercice,
    mois: extracted.mois,
    succursale,
    departement,
    tva_taux: tvaTaux,
    date_generation: new Date().toISOString().split('T')[0],
    total_prime_nette: totPrime,
    total_commission_apport: totApp,
    total_commission_gestion: totGest,
    total_commission_ht: totHt,
    total_tva: totTva,
    total_commission_ttc: totTtc,
    statut: 'BROUILLON',
    lignes,
  };
}

// Export API publique
export const api = {
  getDbInfo: () => invokeTauri<any>('get_db_info'),
  backupDatabase: () => invokeTauri<string>('backup_database'),
  getBranches: () => invokeTauri<BrancheParam[]>('get_branches'),
  updateBranche: (branche: BrancheParam) => invokeTauri<void>('update_branche', { branche }),
  getIntermediaires: () => invokeTauri<Intermediaire[]>('get_intermediaires'),
  saveIntermediaire: (item: Intermediaire) => invokeTauri<number>('save_intermediaire', { item }),
  parseCirisPdf: (filePath: string) => invokeTauri<CirisPdfExtractedData>('parse_ciris_pdf', { filePath }),
  parseCirisPdfBase64: (base64Data: string) => invokeTauri<CirisPdfExtractedData>('parse_ciris_pdf_base64', { base64Data }),
  calculateBordereauFromPdf: (
    extractedData: CirisPdfExtractedData,
    typeIntermediaire: string,
    tvaTaux: number,
    succursale: string,
    departement: string
  ) =>
    invokeTauri<Bordereau>('calculate_bordereau_from_pdf', {
      extractedData,
      typeIntermediaire,
      tvaTaux,
      succursale,
      departement,
    }),
  saveBordereau: (bordereau: Bordereau) => invokeTauri<number>('save_bordereau', { bordereau }),
  getBordereaux: () => invokeTauri<Bordereau[]>('get_bordereaux'),
  getBordereau: (id: number) => invokeTauri<Bordereau | null>('get_bordereau', { id }),
  deleteBordereau: (id: number) => invokeTauri<void>('delete_bordereau', { id }),
  exportExcel: (bordereau: Bordereau, exportPath?: string) =>
    invokeTauri<string>('export_excel', { bordereau, exportPath }),
};
