export interface GraphNode {
  id: string;
  name: string;
  type: string;
  role_type: string;
  avatar: string | null;
}

export interface GraphLink {
  source: string;
  target: string;
  relationship: string;
  intimacy: number;
  status: string;
}

export interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
}

export interface RelationshipType {
  id: number;
  name: string;
  category: string;
  reverse_name: string;
  intimacy_range: string;
  icon: string;
  description: string;
  created_at: string;
}

export interface CharacterCareerRef {
  career_id: string;
  stage?: number;
}

export interface CharacterDetail {
  id: string;
  project_id: string;
  name: string;
  age: string;
  gender: string;
  is_organization: boolean;
  role_type: string;
  personality: string;
  background: string;
  appearance: string;
  organization_type: string;
  organization_purpose: string;
  organization_members: string;
  traits: string;
  avatar_url: string;
  power_level: number;
  location: string;
  motto: string;
  color: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_careers?: CharacterCareerRef[] | string | null;
}

export interface CareerItem {
  id: string;
  name: string;
  type: 'main' | 'sub';
  max_stage: number;
}

export interface CareerListResponse {
  main_careers?: CareerItem[];
  sub_careers?: CareerItem[];
}

export interface CharacterListResponse {
  items?: CharacterDetail[];
}

export interface RelationshipGraphThemeToken {
  colorBgContainer: string;
  colorBorder: string;
  colorError: string;
  colorFillSecondary: string;
  colorInfo: string;
  colorPrimary: string;
  colorSuccess: string;
  colorText: string;
  colorTextBase: string;
  colorTextSecondary: string;
  colorTextTertiary: string;
  colorWarning: string;
}

export type EdgeColorPreset = 'primary' | 'warning' | 'info' | 'textTertiary' | 'error' | 'success';

export interface EdgeCategoryMeta {
  label: string;
  colorPreset: EdgeColorPreset;
  order: number;
}

export interface EdgeCategoryOption {
  category: string;
  count: number;
  label: string;
  color: string;
  order: number;
}

export const GROUP_MAIN_CAREER_NODE_ID = '__career_group_main__';
export const GROUP_SUB_CAREER_NODE_ID = '__career_group_sub__';
