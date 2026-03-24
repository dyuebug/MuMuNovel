import type { ActiveStoryRepairPayload } from '../types';

const ACTIVE_STORY_REPAIR_SOURCE_LABELS: Record<string, string> = {
  manual_request: '手动要求',
  current_chapter_quality: '当前章节质量',
  recent_history_summary: '近期质量趋势',
  manual_plus_current_chapter_quality: '手动要求 + 当前章节质量',
  manual_plus_recent_history_summary: '手动要求 + 近期质量趋势',
};

const ACTIVE_STORY_REPAIR_SCOPE_LABELS: Record<string, string> = {
  chapter: '单章',
  batch: '批量',
};

export const getActiveStoryRepairSourceLabel = (
  payload?: ActiveStoryRepairPayload | null,
): string => {
  if (!payload) {
    return '';
  }
  const source = typeof payload.source === 'string' ? payload.source.trim() : '';
  if (source && ACTIVE_STORY_REPAIR_SOURCE_LABELS[source]) {
    return ACTIVE_STORY_REPAIR_SOURCE_LABELS[source];
  }
  return payload.source_label?.trim() || '';
};

export const getActiveStoryRepairScopeLabel = (
  payload?: ActiveStoryRepairPayload | null,
): string => {
  if (!payload?.scope) {
    return '';
  }
  const scope = String(payload.scope).trim();
  return ACTIVE_STORY_REPAIR_SCOPE_LABELS[scope] || scope;
};

export const getActiveStoryRepairDisplayText = (
  payload?: ActiveStoryRepairPayload | null,
): string => {
  if (!payload) {
    return '';
  }
  return getActiveStoryRepairSourceLabel(payload) || payload.summary?.trim() || '';
};

export const formatActiveStoryRepairLabel = (
  payload?: ActiveStoryRepairPayload | null,
  prefix = '当前策略',
): string => {
  const text = getActiveStoryRepairDisplayText(payload);
  return text ? `${prefix}：${text}` : '';
};

export const formatActiveStoryRepairUpdatedAt = (
  payload?: ActiveStoryRepairPayload | null,
): string => {
  if (!payload?.updated_at) {
    return '';
  }
  const updatedAt = new Date(payload.updated_at);
  if (Number.isNaN(updatedAt.getTime())) {
    return payload.updated_at;
  }
  return updatedAt.toLocaleString('zh-CN');
};
