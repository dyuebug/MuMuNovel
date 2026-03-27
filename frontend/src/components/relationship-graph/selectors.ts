import type { Edge } from '@xyflow/react';

import type {
  CareerItem,
  CharacterCareerRef,
  CharacterDetail,
  EdgeCategoryMeta,
  EdgeCategoryOption,
  EdgeColorPreset,
  RelationshipGraphThemeToken,
  RelationshipType,
} from './types';

const EDGE_CATEGORY_META: Record<string, EdgeCategoryMeta> = {
  organization: { label: '组织成员', colorPreset: 'primary', order: 1 },
  career_main: { label: '主职业关联', colorPreset: 'warning', order: 2 },
  career_sub: { label: '副职业关联', colorPreset: 'info', order: 3 },
  career_group: { label: '职业分组', colorPreset: 'textTertiary', order: 4 },
  family: { label: '家族关系', colorPreset: 'warning', order: 5 },
  hostile: { label: '敌对关系', colorPreset: 'error', order: 6 },
  professional: { label: '职业关系', colorPreset: 'info', order: 7 },
  social: { label: '社交关系', colorPreset: 'success', order: 8 },
  default: { label: '其他关系', colorPreset: 'textTertiary', order: 99 },
};

export const safeParseSubCareers = (raw: CharacterDetail['sub_careers']): CharacterCareerRef[] => {
  if (!raw) return [];

  if (Array.isArray(raw)) {
    return raw.filter((item) => item?.career_id);
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as CharacterCareerRef[];
      return Array.isArray(parsed) ? parsed.filter((item) => item?.career_id) : [];
    } catch {
      return [];
    }
  }

  return [];
};

export const buildCareerNameMap = (mainCareers: CareerItem[], subCareers: CareerItem[]) => {
  const map: Record<string, CareerItem> = {};
  [...mainCareers, ...subCareers].forEach((career) => {
    map[career.id] = career;
  });
  return map;
};

export const getEdgeCategory = (edge: Edge) =>
  typeof edge.data?.category === 'string' ? edge.data.category : 'default';

export const getEdgeCategoryMeta = (category: string): EdgeCategoryMeta =>
  EDGE_CATEGORY_META[category] || {
    label: `${category}关系`,
    colorPreset: 'textTertiary',
    order: 999,
  };

export const resolveEdgePresetColor = (
  colorPreset: EdgeColorPreset,
  token: Pick<
    RelationshipGraphThemeToken,
    'colorPrimary' | 'colorWarning' | 'colorInfo' | 'colorTextTertiary' | 'colorError' | 'colorSuccess'
  >,
) => {
  const colorMap: Record<EdgeColorPreset, string> = {
    primary: token.colorPrimary,
    warning: token.colorWarning,
    info: token.colorInfo,
    textTertiary: token.colorTextTertiary,
    error: token.colorError,
    success: token.colorSuccess,
  };

  return colorMap[colorPreset];
};

export const getRelationshipEdgeColor = (
  relationshipName: string,
  isActive: boolean,
  relationshipTypes: RelationshipType[],
  token: Pick<
    RelationshipGraphThemeToken,
    'colorPrimary' | 'colorWarning' | 'colorInfo' | 'colorTextTertiary' | 'colorError' | 'colorSuccess' | 'colorBorder'
  >,
) => {
  const inactiveColor = token.colorBorder;

  if (relationshipName.startsWith('组织成员·')) {
    return isActive ? token.colorPrimary : inactiveColor;
  }

  if (relationshipName.startsWith('主职业·')) {
    return isActive ? token.colorWarning : inactiveColor;
  }

  if (relationshipName.startsWith('副职业·')) {
    return isActive ? token.colorInfo : inactiveColor;
  }

  if (relationshipName.startsWith('职业分组·')) {
    return isActive ? token.colorTextTertiary : inactiveColor;
  }

  const relType = relationshipTypes.find((item) => item.name === relationshipName);
  const category = relType?.category || 'default';
  const categoryColors: Record<string, string> = {
    family: token.colorWarning,
    hostile: token.colorError,
    professional: token.colorInfo,
    social: token.colorSuccess,
    default: token.colorTextTertiary,
  };

  return isActive ? (categoryColors[category] || categoryColors.default) : inactiveColor;
};

export const buildEdgeCategoryOptions = (
  edges: Edge[],
  token: Pick<
    RelationshipGraphThemeToken,
    'colorPrimary' | 'colorWarning' | 'colorInfo' | 'colorTextTertiary' | 'colorError' | 'colorSuccess'
  >,
): EdgeCategoryOption[] => {
  const counter = new Map<string, number>();

  edges.forEach((edge) => {
    const category = getEdgeCategory(edge);
    counter.set(category, (counter.get(category) || 0) + 1);
  });

  return Array.from(counter.entries())
    .map(([category, count]) => {
      const meta = getEdgeCategoryMeta(category);
      return {
        category,
        count,
        label: meta.label,
        color: resolveEdgePresetColor(meta.colorPreset, token),
        order: meta.order,
      };
    })
    .sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'zh-CN'));
};
