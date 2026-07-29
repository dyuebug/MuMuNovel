import type { NovelWorkflowPhase } from '../../../types';

export interface NovelWorkflowPhasePresentation {
  phase: NovelWorkflowPhase;
  label: string;
  color: string;
  description: string;
}

export const NOVEL_WORKFLOW_PHASE_PRESENTATIONS: readonly NovelWorkflowPhasePresentation[] = [
  {
    phase: 'inspiration',
    label: '灵感构思',
    color: 'magenta',
    description: '收集创意、主题与故事核心冲突。',
  },
  {
    phase: 'foundation',
    label: '项目奠基',
    color: 'volcano',
    description: '明确作品定位、创作目标与基础约束。',
  },
  {
    phase: 'world_building',
    label: '世界构建',
    color: 'geekblue',
    description: '完善时代、地点、规则与世界运行逻辑。',
  },
  {
    phase: 'character_design',
    label: '角色设计',
    color: 'purple',
    description: '设计角色动机、关系与成长弧线。',
  },
  {
    phase: 'outline',
    label: '大纲规划',
    color: 'cyan',
    description: '组织故事结构、情节节点与章节安排。',
  },
  {
    phase: 'writing',
    label: '正文创作',
    color: 'blue',
    description: '按照既定创作上下文持续生成和编辑正文。',
  },
  {
    phase: 'reviewing',
    label: '审校修订',
    color: 'gold',
    description: '检查结构、连续性、逻辑与文本问题。',
  },
  {
    phase: 'polishing',
    label: '润色定稿',
    color: 'orange',
    description: '统一文风、细化表达并完成定稿准备。',
  },
  {
    phase: 'completed',
    label: '已完结',
    color: 'green',
    description: '作品已完成当前版本的创作与审校。',
  },
] as const;

const phasePresentationMap = new Map(
  NOVEL_WORKFLOW_PHASE_PRESENTATIONS.map((item) => [item.phase, item]),
);

const phaseOrderMap = new Map(
  NOVEL_WORKFLOW_PHASE_PRESENTATIONS.map((item, index) => [item.phase, index]),
);

export const getNovelWorkflowPhasePresentation = (
  phase: NovelWorkflowPhase,
): NovelWorkflowPhasePresentation => {
  const presentation = phasePresentationMap.get(phase);
  if (!presentation) {
    throw new Error(`Unsupported novel workflow phase: ${phase}`);
  }
  return presentation;
};

export const isNovelWorkflowRollbackTransition = (
  currentPhase: NovelWorkflowPhase,
  targetPhase: NovelWorkflowPhase,
): boolean => {
  const currentOrder = phaseOrderMap.get(currentPhase);
  const targetOrder = phaseOrderMap.get(targetPhase);
  return currentOrder !== undefined && targetOrder !== undefined && targetOrder < currentOrder;
};

export const requiresNovelWorkflowTransitionConfirmation = (
  currentPhase: NovelWorkflowPhase,
  targetPhase: NovelWorkflowPhase,
): boolean =>
  isNovelWorkflowRollbackTransition(currentPhase, targetPhase) || targetPhase === 'completed';
