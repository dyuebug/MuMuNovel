import type { CreativeMode, PlotStage, QualityPreset, StoryFocus } from '../types';

export type PreferenceOption<T extends string> = {
  value: T;
  label: string;
  description: string;
};

export type QualityPresetOption = PreferenceOption<QualityPreset> & {
  bestFor: string;
  caution: string;
};

export const CREATIVE_MODE_OPTIONS: PreferenceOption<CreativeMode>[] = [
  { value: 'balanced', label: '均衡推进', description: '兼顾钩子、推进、情绪与信息释放。' },
  { value: 'hook', label: '钩子优先', description: '更强调章尾牵引与追读冲动。' },
  { value: 'emotion', label: '情绪沉浸', description: '更强调人物情绪波峰与余震。' },
  { value: 'suspense', label: '悬念加压', description: '更强调危险逼近、信息缺口与不确定。' },
  { value: 'relationship', label: '关系推进', description: '更强调人物拉扯、羁绊和关系变化。' },
  { value: 'payoff', label: '爽点回收', description: '更强调铺垫兑现、爆点与回报闭环。' },
];

export const STORY_FOCUS_OPTIONS: PreferenceOption<StoryFocus>[] = [
  { value: 'advance_plot', label: '主线推进', description: '优先让章节承担推进局势和任务的职责。' },
  { value: 'deepen_character', label: '人物塑形', description: '优先让章节暴露人物选择、弱点与成长痕迹。' },
  { value: 'escalate_conflict', label: '冲突升级', description: '优先让矛盾、代价和阻力逐层升高。' },
  { value: 'reveal_mystery', label: '谜团揭示', description: '优先让章节承担揭线索、修认知、推真相。' },
  { value: 'relationship_shift', label: '关系转折', description: '优先推动人物关系发生可见变化。' },
  { value: 'foreshadow_payoff', label: '伏笔回收', description: '优先处理前文埋设并形成结构回报。' },
];

export const PLOT_STAGE_OPTIONS: PreferenceOption<PlotStage>[] = [
  { value: 'development', label: '发展段', description: '适合铺设矛盾、推进主线、抬升压力。' },
  { value: 'climax', label: '高潮段', description: '适合正面对撞、揭牌、爆点释放。' },
  { value: 'ending', label: '收束段', description: '适合回收伏笔、结算代价、收束情绪。' },
];

export const QUALITY_PRESET_OPTIONS: QualityPresetOption[] = [
  {
    value: 'balanced',
    label: '均衡成稿',
    description: '兼顾可读性、推进效率、情绪落点与信息清晰度。',
    bestFor: '适合大多数题材的通用初稿与稳定连载。',
    caution: '如果要极致钩子或极致文风，仍建议补充额外要求。',
  },
  {
    value: 'plot_drive',
    label: '推进优先',
    description: '更强调冲突推进、目标升级与情节转折。',
    bestFor: '开篇起量、强剧情题材、需要高节奏推进的文本。',
    caution: '可能压缩留白与抒情，人物细腻度需要额外关注。',
  },
  {
    value: 'immersive',
    label: '沉浸氛围',
    description: '更强调场景质感、感官细节与世界沉浸感。',
    bestFor: '玄幻、奇幻、悬疑等依赖氛围压迫感的内容。',
    caution: '若控制不好，可能拖慢推进速度。',
  },
  {
    value: 'emotion_drama',
    label: '情绪拉扯',
    description: '更强调人物情绪波动、关系拉扯与戏剧张力。',
    bestFor: '言情、家庭、都市关系戏等强人物反应题材。',
    caution: '需要防止情绪堆叠过多而主线前进不足。',
  },
  {
    value: 'clean_prose',
    label: '干净利落',
    description: '更强调句式利落、信息清晰与重复压缩。',
    bestFor: '需要高可读性、快节奏、网文连载的稳定成稿。',
    caution: '可能削弱语言纹理，需要额外补充风格细节。',
  },
];

export const resolveOptionLabel = <T extends string>(
  options: Array<{ value: T; label: string }>,
  value?: string | null,
  fallback = '未设定',
) => options.find((item) => item.value === value)?.label || value || fallback;

export const resolveOptionDescription = <T extends string>(
  options: Array<{ value: T; description: string }>,
  value?: string | null,
) => options.find((item) => item.value === value)?.description || '';
