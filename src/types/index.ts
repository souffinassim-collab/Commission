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
  /** Catégorie d'affichage (aucune = « Sans catégorie ») */
  categorie_id?: number | null;
}

/** Catégorie de paramètres (regroupement d'affichage géré par l'utilisateur) */
export interface Categorie {
  id?: number;
  libelle: string;
  ordre: number;
  /** Nombre de lignes rattachées (lecture seule) */
  nb_branches?: number;
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
  client_code?: string;
  branche?: string;
  date_emission?: string;
  date_effet?: string;
  date_echeance?: string;
  cp?: number;
  prime_ttc?: number;
  /** Police déjà facturée dans un autre bordereau (origine) : verrouillée, non comptée */
  deja_facturee?: string;
}

/** Facture de l'intermédiaire saisie pour confirmation (commission HT déclarée par ligne du bordereau) */
export interface FactureIntermediaire {
  numero: string;
  date: string;
  commentaire: string;
  /** clé : code|libellé|branche de la ligne */
  montants_ht: Record<string, number>;
}

export interface PoliceDetail {
  exercice: number;
  police: string;
  branche: string;
  client?: string;
  date_emission?: string;
  date_effet?: string;
  date_echeance?: string;
  prime_nette: number;
  ventilation: (number | null)[];
  cp?: number;
  prime_ttc?: number;
}

export interface LigneRapprochement {
  cle: string;
  libelle: string;
  prime_nette: number;
  taux: number;
  com_ht_calculee: number;
  com_ht_declaree?: number | null;
  ecart?: number | null;
}

/** « Détail de production » à confirmer : détail par police, rapprochement calcul / facture, écarts */
export interface DetailProduction {
  /** « Détail de production courtier » (ou AGA) */
  titre: string;
  succursale: string;
  departement: string;
  type_intermediaire: string;
  intermediaire: string;
  clients: string[];
  exercice: number;
  mois?: number;
  numero_bordereau: string;
  colonnes_ventilation: string[];
  polices: PoliceDetail[];
  total_prime_nette: number;
  total_ventilation: number[];
  total_cp: number;
  total_prime_ttc: number;
  lignes: LigneRapprochement[];
  total_prime_lignes: number;
  controle: number;
  tva_taux: number;
  ht_calcule: number;
  tva_calculee: number;
  ttc_calcule: number;
  facture_saisie: boolean;
  facture_numero: string;
  facture_date: string;
  ht_declare: number;
  tva_declaree: number;
  ttc_declare: number;
  ecart_ht: number;
  ecart_tva: number;
  ecart_ttc: number;
  lignes_non_saisies: number;
  polices_exclues: [string, string, number][];
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
  selection?: string;
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
  /** Facture de l'intermédiaire saisie pour confirmation */
  facture?: FactureIntermediaire;
  /** Client(s) du bordereau au format « code nom » */
  client_libelle?: string;
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
  /** Registre Excel : point de vente retenu */
  point_vente?: string;
  /** Registre Excel : clients retenus (affaires d'un courtier) */
  clients_selectionnes?: string[];
  /** Description de la sélection faite dans le fichier */
  selection?: string;
}

export interface ExcelClientSummary {
  code_client: string;
  designation_client: string;
  nombre_polices: number;
  total_prime_nette: number;
}

/** Point de vente d'un registre de production (préfixe du N° de police = code de l'agence / AGA) */
export interface PointVenteSummary {
  code: string;
  nombre_polices: number;
  nombre_clients: number;
  total_prime_nette: number;
  branches: string[];
}

export interface ExcelScanResult {
  /** Identifiant du classeur conservé par l'application le temps de la sélection */
  staging_id: string;
  file_name: string;
  total_lignes: number;
  points_vente: PointVenteSummary[];
  /** Informations de l'export CIRIS (agence, période d'extraction) */
  info_export: string[];
  periode_emission?: string;
}

/** Sélection dans un registre : un point de vente, et éventuellement les clients apportés par un courtier */
export interface ExcelSelection {
  point_vente?: string;
  clients: string[];
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
  /** Bordereau en cours de modification (ses propres polices ne sont pas des doublons) */
  bordereau_id?: number;
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
