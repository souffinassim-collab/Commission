export type IntermediaireType = 'AGA' | 'COURTIER';
export type StatutBordereau = 'BROUILLON' | 'VALIDE' | 'REGLE';

export interface BrancheParam {
  id?: number;
  code: string;
  branche: string;
  sous_code: string;
  sous_libelle: string;
  taux_apport_courtier: number;
  taux_apport_aga: number;
  taux_gestion_aga: number;
  /** Codes utilisés dans les fichiers importés (ex: colonne Branche Excel "VP,VO,VR") */
  alias_import: string;
  /** Libellé de la ligne cumulée du bordereau courtier (ex: "RC AUTOMOBILE") */
  groupe_bordereau: string;
  ordre_bordereau: number;
}

export interface Intermediaire {
  id?: number;
  type_intermediaire: IntermediaireType;
  code: string;
  nom: string;
  succursale: string;
  departement: string;
  contact?: string;
  adresse?: string;
  tva_applicable: number;
  actif: boolean;
}

/** Anomalie d'import ou de calcul — ERREUR = bloquante pour l'enregistrement */
export interface Anomalie {
  niveau: 'ERREUR' | 'AVERTISSEMENT' | 'INFO';
  message: string;
  /** Ligne de bordereau concernée : l'anomalie ne bloque plus si cette ligne est exclue */
  ligne?: string;
}

export interface LigneContribution {
  fichier_source: string;
  police?: string;
  client?: string;
  montant: number;
  /** Police exclue par le commercial (ex: prime non encaissée) : non commissionnée */
  exclue?: boolean;
}

export interface BordereauLigne {
  id?: number;
  bordereau_id?: number;
  branche_id?: number;
  code_garantie: string;
  libelle_garantie: string;
  branche_nom: string;
  prime_nette: number;
  taux_apport: number;
  taux_gestion: number;
  montant_apport: number;
  montant_gestion: number;
  montant_ht: number;
  montant_tva: number;
  montant_ttc: number;
  non_parametre?: boolean;
  desactive?: boolean;
  contributions?: LigneContribution[];
  source_code?: string;
  source_code_ministeriel?: string;
  source_type?: 'GARANTIE' | 'BRANCHE_IMPORT';
}

export interface FichierBordereau {
  id?: number;
  bordereau_id: number;
  type_piece: 'IMPORT' | 'PIECE_JOINTE';
  nom_fichier: string;
  type_fichier: string;
  hash: string;
  taille: number;
  date_ajout: string;
  client_code?: string;
  client_nom?: string;
  periode?: string;
  total_prime_nette: number;
  nb_lignes: number;
  anomalies: Anomalie[];
}

export interface Bordereau {
  id?: number;
  intermediaire_id?: number;
  intermediaire_nom: string;
  intermediaire_code: string;
  type_intermediaire: IntermediaireType;
  numero_bordereau: string;
  exercice: number;
  mois?: number;
  code_client?: string;
  succursale: string;
  departement: string;
  tva_taux: number;
  date_generation: string;
  total_prime_nette: number;
  total_commission_apport: number;
  total_commission_gestion: number;
  total_commission_ht: number;
  total_tva: number;
  total_commission_ttc: number;
  statut: StatutBordereau;
  lignes: BordereauLigne[];
  anomalies?: Anomalie[];
  fichiers?: FichierBordereau[];
}

export interface ExtractedItem {
  fichier_source: string;
  police?: string;
  client_code?: string;
  client_nom?: string;
  code_garantie: string;
  libelle_garantie: string;
  branche_code: string;
  branche_libelle: string;
  prime_nette: number;
  prime_ttc?: number;
  code_ministeriel?: string;
  source_type: 'GARANTIE' | 'BRANCHE_IMPORT';
}

export interface UploadedFileResult {
  id: string;
  file_name: string;
  file_type: 'CIRIS_PDF' | 'FLOTTE_PDF' | 'EXCEL_PRODUCTION' | string;
  courtier_ou_client?: string;
  code?: string;
  periode?: string;
  total_prime_nette: number;
  items: ExtractedItem[];
  anomalies: Anomalie[];
  /** Contrôles liés au bordereau (intermédiaire, période, fichier déjà commissionné) */
  anomalies_contexte?: Anomalie[];
  hash: string;
  taille: number;
  exercice?: number;
  mois?: number;
}

export interface ExcelClientSummary {
  code_client: string;
  designation_client: string;
  nombre_polices: number;
  total_prime_nette: number;
}

export interface ExcelScanResult {
  file_name: string;
  clients: ExcelClientSummary[];
  total_lignes: number;
  multiple_clients: boolean;
}

/** Contexte d'import : chaque fichier est rattaché à l'intermédiaire et à la période du bordereau */
export interface ImportContext {
  /** Absent si les fichiers sont importés avant d'identifier l'intermédiaire */
  intermediaire_id?: number;
  exercice: number;
  mois?: number;
  bordereau_id?: number;
}

export interface CalculateMultiBordereauRequest {
  files: UploadedFileResult[];
  intermediaire_id?: number;
  type_intermediaire: IntermediaireType;
  intermediaire_code: string;
  intermediaire_nom: string;
  code_client?: string;
  succursale: string;
  departement: string;
  tva_taux: number;
  exercice: number;
  mois?: number;
  cumuler_par_branche: boolean;
}

export interface DbInfo {
  db_path: string;
  version: number;
  total_branches: number;
  total_intermediaires: number;
  total_bordereaux: number;
  last_backup?: string;
}
