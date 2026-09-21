export type IntermediaireType = 'AGA' | 'COURTIER';

export interface BrancheParam {
  id?: number;
  code: string;
  branche: string;
  sous_code: string;
  sous_libelle: string;
  taux_apport_courtier: number;
  taux_apport_aga: number;
  taux_gestion_aga: number;
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
  tva_applicable: number; // default 0.19
  actif: boolean;
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
  statut: 'BROUILLON' | 'VALIDE' | 'REGLE';
  lignes: BordereauLigne[];
}

export interface CirisPdfExtractedData {
  agence_code: string;
  agence_nom: string;
  periode_debut: string;
  periode_fin: string;
  mois: number;
  exercice: number;
  devise: string;
  total_pdf_primes_nettes: number;
  items: {
    garantie: string;
    description: string;
    branche_code: string;
    branche_libelle: string;
    prime_nette: number;
    primes_ttc?: number;
  }[];
}
