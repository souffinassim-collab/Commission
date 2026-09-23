import {
  BrancheParam,
  Intermediaire,
  Bordereau,
  DbInfo,
  ExcelScanResult,
  UploadedFileResult,
  ImportContext,
  CalculateMultiBordereauRequest,
  StatutBordereau,
} from '../types';

// Vérification de l'environnement Tauri
export const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

// Toutes les données (paramètres, intermédiaires, bordereaux, fichiers) proviennent exclusivement
// de la base SQLite via le backend Tauri : aucune donnée simulée.
async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error("Cette fonction nécessite l'application de bureau (base de données locale).");
  }
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

export const api = {
  // Base
  getDbInfo: () => invokeTauri<DbInfo>('get_db_info'),
  backupDatabase: () => invokeTauri<string>('backup_database'),

  // Paramètres
  getBranches: () => invokeTauri<BrancheParam[]>('get_branches'),
  updateBranche: (branche: BrancheParam) => invokeTauri<void>('update_branche', { branche }),
  saveBranche: (branche: BrancheParam) => invokeTauri<number>('save_branche', { branche }),
  deleteBranche: (id: number) => invokeTauri<void>('delete_branche', { id }),
  resetBranchesToDefault: () => invokeTauri<BrancheParam[]>('reset_branches_to_default'),

  // Intermédiaires
  getIntermediaires: () => invokeTauri<Intermediaire[]>('get_intermediaires'),
  saveIntermediaire: (item: Intermediaire) => invokeTauri<number>('save_intermediaire', { item }),
  deleteIntermediaire: (id: number) => invokeTauri<void>('delete_intermediaire', { id }),

  // Import
  scanExcelClients: (filePath: string) => invokeTauri<ExcelScanResult>('scan_excel_clients', { filePath }),
  scanExcelClientsBase64: (base64Data: string, fileName: string) =>
    invokeTauri<ExcelScanResult>('scan_excel_clients_base64', { base64Data, fileName }),
  parseFileAuto: (filePath: string, ctx: ImportContext, clientFilter?: string) =>
    invokeTauri<UploadedFileResult>('parse_file_auto', { filePath, clientFilter, ctx }),
  parseFileAutoBase64: (base64Data: string, fileName: string, ctx: ImportContext, clientFilter?: string) =>
    invokeTauri<UploadedFileResult>('parse_file_auto_base64', { base64Data, fileName, clientFilter, ctx }),
  clearPendingFiles: (ids: string[]) => invokeTauri<void>('clear_pending_files', { ids }),
  revaliderFichiers: (fichiers: UploadedFileResult[], ctx: ImportContext) =>
    invokeTauri<UploadedFileResult[]>('revalider_fichiers', { fichiers, ctx }),
  calculateMultiBordereau: (req: CalculateMultiBordereauRequest) =>
    invokeTauri<Bordereau>('calculate_multi_bordereau', { req }),

  // Bordereaux
  saveBordereau: (bordereau: Bordereau, fichiers?: UploadedFileResult[]) =>
    invokeTauri<number>('save_bordereau', { bordereau, fichiers: fichiers ?? null }),
  getBordereaux: () => invokeTauri<Bordereau[]>('get_bordereaux'),
  getBordereau: (id: number) => invokeTauri<Bordereau | null>('get_bordereau', { id }),
  updateBordereauStatut: (id: number, statut: StatutBordereau) =>
    invokeTauri<void>('update_bordereau_statut', { id, statut }),
  deleteBordereau: (id: number) => invokeTauri<void>('delete_bordereau', { id }),
  exportExcel: (bordereau: Bordereau, exportPath?: string) =>
    invokeTauri<string>('export_excel', { bordereau, exportPath }),

  // Fichiers rattachés / pièces jointes
  addPieceJointe: (bordereauId: number, filePath: string) =>
    invokeTauri<number>('add_piece_jointe', { bordereauId, filePath }),
  addPieceJointeBase64: (bordereauId: number, fileName: string, base64Data: string) =>
    invokeTauri<number>('add_piece_jointe_base64', { bordereauId, fileName, base64Data }),
  deletePieceJointe: (fichierId: number) => invokeTauri<void>('delete_piece_jointe', { fichierId }),
  ouvrirFichier: (fichierId: number) => invokeTauri<string>('ouvrir_fichier', { fichierId }),
};

/** Lit un fichier local (navigateur) en data-URL base64 */
export const readFileAsDataUrl = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error || new Error('Lecture du fichier impossible'));
    reader.readAsDataURL(file);
  });

export const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

export const formatDZD = (v: number) =>
  new Intl.NumberFormat('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(v) + ' DZD';

export const formatMontant = (v: number) => v.toLocaleString('fr-DZ', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

export const MOIS_LIBELLES = [
  'Janvier', 'Février', 'Mars', 'Avril', 'Mai', 'Juin',
  'Juillet', 'Août', 'Septembre', 'Octobre', 'Novembre', 'Décembre',
];
