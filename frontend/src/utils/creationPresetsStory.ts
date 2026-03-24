import type { CreativeMode, StoryFocus } from '../types';
import {
  type CreationBlueprint,
  type CreationBlueprintScene,
  type CreationPlotStage,
  type StoryAcceptanceCard,
  type StoryCharacterArcCard,
  type StoryExecutionChecklist,
  type StoryObjectiveCard,
  type StoryRepetitionRiskCard,
  type StoryResultCard,
  type VolumePacingPlan,
  type VolumePacingSegment,
  CREATIVE_MODE_LABELS,
  STORY_FOCUS_LABELS,
  PLOT_STAGE_LABELS,
  PLOT_STAGE_MISSIONS,
  dedupeItems,
} from './creationPresetsCore';

export function buildVolumePacingPlan(
  chapterCount?: number | null,
  options?: {
    preferredStage?: CreationPlotStage | null;
    currentChapterNumber?: number | null;
  },
): VolumePacingPlan | undefined {
  const total = Math.max(0, Math.floor(Number(chapterCount ?? 0)));
  if (total <= 0) return undefined;

  let developmentCount = 1;
  let climaxCount = 0;
  let endingCount = 0;

  if (total === 1) {
    developmentCount = 1;
  } else if (total === 2) {
    developmentCount = 1;
    endingCount = 1;
  } else if (total === 3) {
    developmentCount = 1;
    climaxCount = 1;
    endingCount = 1;
  } else {
    developmentCount = Math.max(1, Math.round(total * 0.45));
    climaxCount = Math.max(1, Math.round(total * 0.35));
    endingCount = total - developmentCount - climaxCount;

    if (endingCount < 1) {
      endingCount = 1;
      if (developmentCount >= climaxCount && developmentCount > 1) {
        developmentCount -= 1;
      } else if (climaxCount > 1) {
        climaxCount -= 1;
      }
    }
  }

  const segments: VolumePacingSegment[] = [];
  let cursor = 1;
  const pushSegment = (stage: CreationPlotStage, count: number) => {
    if (count <= 0) return;
    const startChapter = cursor;
    const endChapter = cursor + count - 1;
    cursor = endChapter + 1;
    segments.push({
      stage,
      label: PLOT_STAGE_LABELS[stage],
      startChapter,
      endChapter,
      mission: PLOT_STAGE_MISSIONS[stage],
    });
  };

  pushSegment('development', developmentCount);
  pushSegment('climax', climaxCount);
  pushSegment('ending', endingCount);

  const currentChapter = Math.max(0, Math.floor(Number(options?.currentChapterNumber ?? 0)));
  const currentStage = currentChapter > 0
    ? segments.find((segment) => currentChapter >= segment.startChapter && currentChapter <= segment.endChapter)?.stage
    : undefined;

  const preferredStage = options?.preferredStage ?? undefined;
  const summaryParts = [`按 ${total} 章体量，建议整体分为 ${segments.length} 段节奏推进。`];
  if (preferredStage) {
    summaryParts.push(`当前重点阶段是「${PLOT_STAGE_LABELS[preferredStage]}」。`);
  }
  if (currentStage) {
    summaryParts.push(`当前章节位置落在「${PLOT_STAGE_LABELS[currentStage]}」。`);
  }

  return {
    title: '卷级节奏预览',
    summary: summaryParts.join(' '),
    segments,
    currentStage,
  };
}


export function buildStoryObjectiveCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryObjectiveCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let objective = scene === 'outline'
    ? '让本轮章节承担清晰主任务，不平均摊功能。'
    : '让本章推动一个看得见的目标，不写空转段落。';
  let obstacle = scene === 'outline'
    ? '让中段持续抬压，每一章都比上一章更难一点。'
    : '安排一次明确受阻、代价上升或信息错位。';
  let turn = scene === 'outline'
    ? '在后段安排一次会改写后续走向的结构转折。'
    : '在中后段安排一次认知或局面改写。';
  let hook = scene === 'outline'
    ? '尾段留下下一轮章节必须回应的问题或新任务。'
    : '章尾留下追读牵引，不平收。';

  switch (creativeMode) {
    case 'hook':
      hook = '把钩子放在异常、危险或未决选择上，尽量做到前段抓人、尾段牵引。';
      turn = '转折优先用信息缺口扩大、危险临门或局势突然偏转来触发。';
      break;
    case 'emotion':
      objective = scene === 'outline'
        ? '目标除了推进事件，还要逼出人物情绪波动和关系反馈。'
        : '让本章既推进事件，也逼出人物情绪与关系反应。';
      turn = '转折优先落在情绪反噬、误伤、和解受阻或认知偏移上。';
      hook = '钩子留在情绪未落地、关系未说破或选择仍有余震处。';
      break;
    case 'suspense':
      obstacle = '阻力优先来自信息差、误判、证据反噬或真相未全。';
      turn = '转折通过线索翻面、认知刷新、身份异动或危险升级完成。';
      hook = '钩子留在新疑点、半揭开的答案或更近一步的危险上。';
      break;
    case 'relationship':
      objective = scene === 'outline'
        ? '本轮重点推动人物关系位移，让站队和信任结构发生变化。'
        : '让本章推动一次明确的关系位移，而不只是情绪点缀。';
      obstacle = '阻力来自立场差、亏欠、信任裂缝或试探失手。';
      turn = '转折优先用关系破裂、突然靠近、站队变化或误会反转来完成。';
      hook = '钩子留在关系未定、话没说透、立场悬空的地方。';
      break;
    case 'payoff':
      objective = scene === 'outline'
        ? '本轮重点兑现前文铺垫、承诺或能力，并带出更大后果。'
        : '让本章承担一次明确兑现，让读者感到回报落地。';
      turn = '转折优先让兑现带出更大代价、更高目标或新的麻烦。';
      hook = '钩子放在回报之后的新失衡上，而不是只停在爽点本身。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      objective = '核心目标是把局势往前推一格，至少形成新的行动结果。';
      break;
    case 'deepen_character':
      objective = '核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。';
      break;
    case 'escalate_conflict':
      obstacle = '阻力必须逐层变强，让代价和对立面都更具体。';
      break;
    case 'reveal_mystery':
      turn = '转折优先通过线索出现、误导修正和认知刷新来完成。';
      break;
    case 'relationship_shift':
      turn = '转折必须带来关系位移、立场重排或信任结构变化。';
      break;
    case 'foreshadow_payoff':
      objective = '核心目标是兑现前文埋设，并顺手打开新的后续空间。';
      hook = '钩子留在兑现后的新承诺、新麻烦或更大代价上。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      objective = scene === 'outline'
        ? '当前阶段先立局、铺变量和主任务，把后续压力链搭起来。'
        : '当前阶段先把局势和眼前目标推到更难的位置。';
      break;
    case 'climax':
      obstacle = '阻力要逼近正面碰撞，选择代价必须明显抬高。';
      turn = '转折要接近核心碰撞点，不能只是小波动。';
      break;
    case 'ending':
      objective = scene === 'outline'
        ? '当前阶段优先回收主承诺、主悬念和关键关系线。'
        : '当前阶段让本章承担主承诺或关键关系线的回收职责。';
      hook = '钩子更适合留余味、次级悬念或收束后的新失衡，不能抢走主收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲目标卡' : '章节目标卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来组织本轮叙事任务。`
      : '当前会按默认叙事任务来组织本轮创作。',
    objective,
    obstacle,
    turn,
    hook,
  };
}


export function buildStoryResultCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryResultCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let progress = scene === 'outline'
    ? '这一轮结束后，主线应进入一个更具体、更难回头的新局面。'
    : '这一章结束后，局势应明确前移，人物不能还停在原地。';
  let reveal = scene === 'outline'
    ? '至少释放一轮信息、真相碎片或兑现回报，避免纯拖延。'
    : '至少交付一个新认知、新线索或一次有效兑现。';
  let relationship = scene === 'outline'
    ? '关键人物关系、站队或信任结构要出现可见位移。'
    : '至少有一条人物关系线出现可见变化，而不是只说情绪。';
  let fallout = scene === 'outline'
    ? '尾段要把下一轮章节必须回应的压力、问题或任务钉住。'
    : '章尾要留下一个会逼出下章动作的余波，而不是平稳收住。';

  switch (creativeMode) {
    case 'hook':
      progress = scene === 'outline'
        ? '本轮结束后，读者要感到故事被明显拽进下一段更危险的局面。'
        : '本章结束后，局势必须被推到一个不继续看就会难受的节点。';
      fallout = '余波优先落在未决选择、临门危险或刚被挑开的异常上。';
      break;
    case 'emotion':
      reveal = '结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。';
      relationship = '关系结果要落到互动后果上，让人物之后的做法因此改变。';
      break;
    case 'suspense':
      reveal = '至少留下一个更接近真相的新证据，同时制造新的误判空间。';
      fallout = '余波留在新疑点、身份异动或危险升级上，不能只剩空白遮掩。';
      break;
    case 'relationship':
      relationship = '结果里必须出现一次明确的关系位移、立场变化或信任重排。';
      fallout = '余波最好落在关系未定、话未说透或站队未稳上。';
      break;
    case 'payoff':
      reveal = '结果要让读者看到铺垫兑现、回报落地，并感到不是白等。';
      progress = scene === 'outline'
        ? '兑现之后，主线要进入一个新的阶段，而不是只做结算。'
        : '兑现之后，局势要被顺势推向更高目标或更大麻烦。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      progress = '推进结果必须清晰可见：行动产生了后果，局势换了位置。';
      break;
    case 'deepen_character':
      reveal = '结果要让人物的弱点、执念或价值判断真正显形，而非停在说明。';
      relationship = '人物变化要影响他与他人的互动方式或后续选择。';
      break;
    case 'escalate_conflict':
      progress = '推进结果不是前进一步，而是把人推入更高代价的冲突区。';
      fallout = '余波要把冲突继续抬高，让下一轮没有轻松退路。';
      break;
    case 'reveal_mystery':
      reveal = '揭示结果必须真实推进谜团，不只是制造更多模糊表述。';
      break;
    case 'relationship_shift':
      relationship = '关系结果必须足够明确，能改变两人之后的说话方式、站位或合作条件。';
      break;
    case 'foreshadow_payoff':
      reveal = '结果要让前文埋设获得兑现，同时打开新的后续空间。';
      fallout = '余波放在兑现后的新承诺、新代价或更大失衡上。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      progress = scene === 'outline'
        ? '这一轮结束后，故事应完成立局并把压力链真正搭起来。'
        : '这一章结束后，故事要进入一个更难但更清晰的推进区。';
      fallout = '余波要把后续任务钉住，让读者知道下一章不是重复上一章。';
      break;
    case 'climax':
      progress = '推进结果要逼近或触发正面碰撞，不能只是外围晃动。';
      reveal = '揭示结果要掀开关键底牌、核心真相或决定性误判。';
      break;
    case 'ending':
      reveal = '揭示结果优先服务主承诺、主悬念与关键伏笔的回收。';
      relationship = '关系结果要体现收束、定局或带余温的最终位移。';
      fallout = '余波更适合留余味、后效和新失衡，不能抢走主收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲结果卡' : '章节结果卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先要求本轮写完后留下「${comboLabels.join(' / ')}」对应的结果落点。`
      : '当前会按默认结果落点来组织本轮创作产出。',
    progress,
    reveal,
    relationship,
    fallout,
  };
}


export function buildStoryExecutionChecklist(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryExecutionChecklist | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let opening = scene === 'outline'
    ? '前段先用 1-2 章立主任务、人物站位和局势缺口，尽快进入事件。'
    : '开场 30% 内抛出目标、异常或受阻点，不平铺背景。';
  let pressure = scene === 'outline'
    ? '中段持续加压，每一章追加一个新阻力、代价或变量。'
    : '中段用动作、对话和反馈连续加压，避免解释停顿。';
  let pivot = scene === 'outline'
    ? '后段安排一次会改写路线的关键转折、揭示或站队变化。'
    : '中后段安排一次改写认知或局面的关键动作。';
  let closing = scene === 'outline'
    ? '尾段先给阶段性结果，再把下一轮问题抛实。'
    : '收尾先落结果，再留下逼出下章的余波。';

  switch (creativeMode) {
    case 'hook':
      opening = scene === 'outline'
        ? '前段优先让异常、危险或未决任务尽快冒头，不慢热铺垫。'
        : '开场尽快抛出异常、险情或未决选择，让读者立刻进入状态。';
      closing = '收尾把悬而未决的危险、选择或信息缺口钉牢，形成追读牵引。';
      break;
    case 'emotion':
      pressure = '中段用互动、误伤、退让受阻或情绪回弹来持续加压。';
      pivot = '关键转折优先落在情绪爆裂、和解失败或认知刺痛上。';
      closing = '收尾保留情绪余震，让人物无法当场彻底消化。';
      break;
    case 'suspense':
      opening = '开场先扔出异常线索、误判苗头或危险信号，再补背景。';
      pressure = '中段不断扩大信息差、证据变化和错误判断的代价。';
      pivot = '转折优先让线索翻面、身份异动或危险升级来改写局面。';
      closing = '收尾留下更尖锐的新疑点，而不是只把答案藏起来。';
      break;
    case 'relationship':
      opening = '开场先把关系张力、站位差或试探动作摆上台面。';
      pressure = '中段持续通过对话、行动和站队测试来挤压关系。';
      pivot = '转折优先用关系破裂、突然靠近或立场变化来触发。';
      closing = '收尾把关系悬在未定状态，逼出下一轮互动。';
      break;
    case 'payoff':
      opening = '开场尽快回扣前文埋设，提醒读者这轮会有兑现。';
      pressure = '中段不断把兑现条件推近，同时抬高兑现所需代价。';
      pivot = '转折优先让铺垫兑现落地，但必须伴随新后果。';
      closing = '收尾不要停在爽点，要顺手抛出兑现后的新失衡。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      opening = '开场先亮明本轮要推进的事，别让读者等太久才知道这章要干嘛。';
      pressure = '中段每次推进都要带来新结果，避免原地解释和空转。';
      break;
    case 'deepen_character':
      pressure = '中段把压力尽量变成选择题，让人物性格在决策里显形。';
      pivot = '关键转折最好来自人物自己的选择、软肋或价值判断。';
      closing = '收尾保留人物做完选择后的余震，而不是只交代事件结束。';
      break;
    case 'escalate_conflict':
      pressure = '中段每一轮加压都要比上一轮更狠，别重复同级冲突。';
      pivot = '转折要把冲突推向正面碰撞，而不是继续绕圈。';
      closing = '收尾把人物钉在更高代价区，确保下一轮没法轻退。';
      break;
    case 'reveal_mystery':
      opening = '开场尽快抛出线索、异常或疑点，别先讲设定。';
      pressure = '中段通过调查、误导修正和证据变化推进认知。';
      pivot = '转折要真正修正一次认知，而不是只多说一点背景。';
      break;
    case 'relationship_shift':
      pressure = '中段每次互动都要推动信任、亏欠或站队发生偏移。';
      pivot = '转折必须带来明确关系位移，而不是轻微情绪波动。';
      closing = '收尾把关系停在新的不稳定位置，逼出后续回应。';
      break;
    case 'foreshadow_payoff':
      opening = '开场尽快回扣旧埋设，提示这轮不是凭空发生。';
      pivot = '转折优先让伏笔兑现落地，同时带出新的空间。';
      closing = '收尾把兑现后的新承诺、新代价或新任务明确抛出。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      opening = scene === 'outline'
        ? '前段优先立局、铺变量和主任务，让整轮章节知道往哪推。'
        : '开场先把局势、变量和眼前目标摆上桌，不要空转导入。';
      pivot = '转折可以不必最大，但必须让方向感发生变化。';
      closing = '收尾把下一轮任务钉住，让压力链接得上。';
      break;
    case 'climax':
      opening = '开场就贴近核心矛盾，不要再绕外围做长引子。';
      pressure = '中段压缩时间、代价和回旋空间，让碰撞不可拖延。';
      pivot = '转折必须接近决定性碰撞、底牌掀开或局势断裂。';
      closing = '收尾把后果锁死，让人物进入无法回避的下一步。';
      break;
    case 'ending':
      opening = '开场快速回扣未收束的主承诺、主悬念或关键关系线。';
      pressure = '中段优先处理最重要的收束任务，不分散到支线琐事。';
      pivot = '转折优先落在主承诺兑现、主真相揭开或关键关系定局上。';
      closing = '收尾保留余味和后效，但不要再制造喧宾夺主的新主线。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲执行清单' : '章节执行清单',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来拆分本轮执行步骤。`
      : '当前会按默认执行步骤来组织本轮创作。',
    opening,
    pressure,
    pivot,
    closing,
  };
}


export function buildStoryRepetitionRiskCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryRepetitionRiskCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let openingRisk = scene === 'outline'
    ? '不要每轮前段都只做设定铺陈，读者会感觉整轮大纲在原地起步。'
    : '不要反复用回忆、说明或同一种异常开场，容易让章节起手发闷。';
  let pressureRisk = scene === 'outline'
    ? '不要每章都用同一级别阻力灌水，中段会失去递进感。'
    : '不要把受阻写成同一种争吵、误会或嘴上发狠，压力会显得空。';
  let pivotRisk = scene === 'outline'
    ? '不要把每次转折都写成临时加设定或生硬插入新人物。'
    : '不要把转折写成假反转、硬转念或只靠旁白解释。';
  let closingRisk = scene === 'outline'
    ? '不要每轮都只用“下回更精彩”式尾章，下一轮任务必须具体。'
    : '不要每章都用同一种问句、敲门声或电话铃收尾，钩子会疲劳。';

  switch (creativeMode) {
    case 'hook':
      openingRisk = '钩子模式下不要每次都靠突发危险硬拽开场，异常类型需要变化。';
      closingRisk = '不要连续多章都用悬空危险硬切章尾，读者会识别套路。';
      break;
    case 'emotion':
      pressureRisk = '不要反复靠争吵、沉默或内心独白制造情绪，否则张力会钝化。';
      pivotRisk = '不要把情绪转折写成突然想通，缺少事件触发会显得虚。';
      break;
    case 'suspense':
      openingRisk = '悬念模式下不要只会丢疑点不交代有效信息，否则会像故意遮掩。';
      pivotRisk = '不要连续用“其实另有隐情”做反转，真相推进需要层次。';
      closingRisk = '不要只留空白疑问而不给新证据，悬念会变成拖延。';
      break;
    case 'relationship':
      pressureRisk = '不要把关系推进写成重复拉扯却没有立场后果，读者会觉得没变化。';
      pivotRisk = '不要每次都靠误会触发关系变化，站队和选择也要轮换。';
      break;
    case 'payoff':
      openingRisk = '回收模式下不要一上来就罗列旧伏笔目录，读者需要事件化兑现。';
      closingRisk = '不要每次回收完都再塞一个更大的谜团，容易冲淡回报感。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      pressureRisk = '主线推进不要只做位移和赶路，缺少阻力变化会像流水账。';
      break;
    case 'deepen_character':
      openingRisk = '人物塑形不要总从心理描写起手，最好让性格先在动作里显形。';
      pressureRisk = '不要把成长写成同一种自责或回忆，人物弧线会发虚。';
      break;
    case 'escalate_conflict':
      pressureRisk = '冲突升级不要一直放大音量不抬高代价，否则只是吵得更大声。';
      pivotRisk = '不要把冲突转折只写成新敌人登场，最好让旧矛盾也发生质变。';
      break;
    case 'reveal_mystery':
      pivotRisk = '谜团揭示不要总靠旁人解释，证据和事件本身也要承担揭示功能。';
      closingRisk = '不要连续多次只留下谜面不回收谜底，读者会怀疑作者在拖。';
      break;
    case 'relationship_shift':
      pressureRisk = '关系转折不要只换台词腔调，最好同步改变合作方式和站位。';
      break;
    case 'foreshadow_payoff':
      closingRisk = '伏笔回收不要每次都变成新伏笔发射器，需保留真正落地的满足。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      openingRisk = '发展阶段不要长时间停在铺垫准备态，必须尽快把变量推上桌。';
      closingRisk = '发展阶段不要每章都只留一个模糊目标，任务应逐步具体化。';
      break;
    case 'climax':
      pressureRisk = '高潮阶段不要反复假装要碰撞却不断拖开，读者会明显感到泄劲。';
      pivotRisk = '高潮阶段不要只有大声量和快节奏，没有决定性变化就不算高潮。';
      break;
    case 'ending':
      openingRisk = '结局阶段不要又重新搭新盘子，优先收最重要的旧承诺。';
      closingRisk = '结局阶段不要为了续作感强行再开主线，否则会稀释收束力度。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲重复风险卡' : '章节重复风险卡',
    summary: comboLabels.length > 0
      ? `当前组合容易在「${comboLabels.join(' / ')}」上形成固定套路，建议提前避重。`
      : '当前会按默认重复风险维度提醒本轮创作避重。',
    openingRisk,
    pressureRisk,
    pivotRisk,
    closingRisk,
  };
}


export function buildStoryAcceptanceCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryAcceptanceCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let missionCheck = scene === 'outline'
    ? '验收时先看这轮章节是否承担了明确主任务，而不是平均摊功能。'
    : '验收时先看本章是否完成了一个清晰主任务，而不是热闹但空转。';
  let changeCheck = scene === 'outline'
    ? '至少要看到局势、关系或认知层面的阶段性变化，不能只搭台。'
    : '至少要看到局势、关系或认知有一项明确变化，不能原地踏步。';
  let freshnessCheck = scene === 'outline'
    ? '检查本轮关键章法是否和上一轮过度同构，避免整卷节拍重复。'
    : '检查开场、加压、转折、收尾是否又落回同一种旧套路。';
  let closingCheck = scene === 'outline'
    ? '尾段既要交代阶段结果，也要给下一轮留下具体任务。'
    : '章尾既要完成本章收束，也要留下合适的追读牵引或余味。';

  switch (creativeMode) {
    case 'hook':
      missionCheck = '验收时重点看开场和章尾是否真正形成牵引，而不只是制造噪音。';
      closingCheck = '结尾要让读者有继续读的冲动，但不能只有硬切和悬空。';
      break;
    case 'emotion':
      changeCheck = '验收时要看到情绪余震和关系后果，而不是只有一段抒情。';
      freshnessCheck = '检查情绪推进是否又只是争吵、沉默或内心独白轮换。';
      break;
    case 'suspense':
      changeCheck = '验收时至少要有一个有效线索、认知刷新或危险升级真正落地。';
      closingCheck = '结尾要留下更尖锐的问题，但不能完全不给有效信息。';
      break;
    case 'relationship':
      missionCheck = '验收时看人物关系是否真的发生位移，而不是只多说了几句狠话。';
      changeCheck = '关系变化最好能改动人物之后的站位、合作或信任条件。';
      break;
    case 'payoff':
      missionCheck = '验收时要确认前文铺垫是否真正兑现，而不是只口头提到。';
      closingCheck = '兑现之后要有后效和新失衡，不能只停在一次性爽点。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      missionCheck = '验收时先看主线是否实打实前进，而不是忙了很多事却没推局势。';
      break;
    case 'deepen_character':
      changeCheck = '验收时看人物是否在选择里显形，而不是只补充背景说明。';
      freshnessCheck = '检查人物塑形是否又回到同一种回忆、自责或旁白总结。';
      break;
    case 'escalate_conflict':
      changeCheck = '验收时要能看见代价升级、对立加深或冲突进入新层级。';
      closingCheck = '本轮结束后人物应被留在更难的位置，而不是轻松退回安全区。';
      break;
    case 'reveal_mystery':
      missionCheck = '验收时必须确认谜团有真实推进，而不是只多堆了一层雾。';
      break;
    case 'relationship_shift':
      changeCheck = '验收时看关系是否足以改变说话方式、行动选择或站队逻辑。';
      break;
    case 'foreshadow_payoff':
      missionCheck = '验收时确认伏笔是否兑现落地，同时打开了新的后续空间。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      missionCheck = '发展阶段验收重点是：有没有把局势、变量和主任务真正搭起来。';
      closingCheck = '收尾应让下一轮任务更具体，而不是继续停留在准备态。';
      break;
    case 'climax':
      changeCheck = '高潮阶段验收重点是：有没有形成决定性碰撞、底牌掀开或局势断裂。';
      freshnessCheck = '检查高潮是否只是声量更大，还是确实发生了不可逆变化。';
      break;
    case 'ending':
      missionCheck = '结局阶段验收重点是：主承诺、主悬念和关键关系线是否得到有效回收。';
      closingCheck = '收尾应保留余味，但不能为了留白再次打散已经完成的收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲验收卡' : '章节验收卡',
    summary: comboLabels.length > 0
      ? `当前组合建议按「${comboLabels.join(' / ')}」来验收本轮成稿质量。`
      : '当前会按默认验收标准来检查本轮创作是否达标。',
    missionCheck,
    changeCheck,
    freshnessCheck,
    closingCheck,
  };
}


export function buildStoryCharacterArcCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryCharacterArcCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let externalLine = scene === 'outline'
    ? '这一轮至少要让核心人物的外在线任务更明确，不只推动事件壳子。'
    : '本章要让人物在外在线上做出能看见后果的动作，而不是被剧情拖着走。';
  let internalLine = scene === 'outline'
    ? '安排一次会暴露人物执念、伤口或价值判断的压力测试。'
    : '本章要逼出一次能暴露人物软肋、执念或底线的反应。';
  let relationshipLine = scene === 'outline'
    ? '让关键关系在信任、站队或依赖上出现可见变化。'
    : '至少让一条关系线发生可见位移，而不只是多说几句情绪台词。';
  let arcLanding = scene === 'outline'
    ? '尾段给出人物阶段性变化，让下一轮成长方向更清晰。'
    : '章尾要留下人物状态的新落点，让后续成长有承接。';

  switch (creativeMode) {
    case 'hook':
      externalLine = '人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。';
      arcLanding = '弧光落点要落在人物被推入新处境上，而不只是事件悬空。';
      break;
    case 'emotion':
      internalLine = '内在线重点看人物如何被情绪反噬、误伤他人或压抑失败。';
      relationshipLine = '关系线最好呈现安慰失败、靠近受阻或误伤后的余震。';
      break;
    case 'suspense':
      externalLine = '人物外在线尽量和追查、判断、求生或拆解异常绑定。';
      internalLine = '通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。';
      break;
    case 'relationship':
      relationshipLine = '关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。';
      arcLanding = '落点应让人物在关系位置上进入一个再也回不到原点的新阶段。';
      break;
    case 'payoff':
      externalLine = '人物外在线要和旧承诺兑现、旧目标回收或能力回报直接挂钩。';
      arcLanding = '落点要让人物因为兑现获得成长回报，或承担兑现带来的新责任。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      externalLine = '人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。';
      break;
    case 'deepen_character':
      internalLine = '内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。';
      arcLanding = '落点最好形成一次人物自我认知偏移，而不只是事件结束。';
      break;
    case 'escalate_conflict':
      internalLine = '冲突升级时要逼出人物底线，看看他在更高代价下会怎么变。';
      relationshipLine = '更强冲突最好同步改写人物之间的站位与依赖结构。';
      break;
    case 'reveal_mystery':
      externalLine = '人物外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。';
      internalLine = '认知刷新应反照人物偏见、恐惧或执念，而不是只补世界观信息。';
      break;
    case 'relationship_shift':
      relationshipLine = '关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。';
      break;
    case 'foreshadow_payoff':
      arcLanding = '人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      externalLine = scene === 'outline'
        ? '发展阶段先让人物想要什么、怕什么、要付什么代价变得清楚。'
        : '发展阶段先把人物眼前要争什么、躲什么、赌什么摆清楚。';
      arcLanding = '落点应把人物推入更难但更清晰的成长压力链。';
      break;
    case 'climax':
      internalLine = '高潮阶段要逼出人物真正底线、真实选择或最不愿面对的自我。';
      relationshipLine = '高潮中的关系变化最好是定向性变化，而不是小幅试探。';
      break;
    case 'ending':
      relationshipLine = '结局阶段要让关键关系线出现收束、定局或带余温的最终位移。';
      arcLanding = '落点要给人物阶段性定局、余味或代价后的新平衡。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲角色弧光卡' : '章节角色弧光卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来推动人物弧光。`
      : '当前会按默认人物弧光标准来组织本轮创作。',
    externalLine,
    internalLine,
    relationshipLine,
    arcLanding,
  };
}

export function buildCreationBlueprint(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): CreationBlueprint | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  const priorityBeats: string[] = [];
  const priorityRisks: string[] = [];

  switch (creativeMode) {
    case 'hook':
      priorityBeats.push('开场更早抛出异常、危险或未完成目标，先抓住读者注意力。');
      priorityBeats.push('尾段优先保留信息缺口、危险临门或选择未决，不要平收。');
      priorityRisks.push('不要只堆钩子和异常，却缺少实质推进。');
      break;
    case 'emotion':
      priorityBeats.push('关键转折后要写出人物情绪余震和关系反应，不只交代结果。');
      priorityBeats.push('让动作、停顿和对白共同承载情绪，而不是全靠抒情说明。');
      priorityRisks.push('不要让情绪独自悬空，必须落回选择与后果。');
      break;
    case 'suspense':
      priorityBeats.push('中前段持续制造信息差、误判或证据变化，让压力逐步抬升。');
      priorityBeats.push('每个阶段都给出一点新认知，但不要一次讲透底牌。');
      priorityRisks.push('避免把悬念写成纯遮掩，读者需要看到有效推进。');
      break;
    case 'relationship':
      priorityBeats.push('把关键冲突尽量落在人与人之间的立场差、亏欠感或试探上。');
      priorityBeats.push('安排一次关系位移，让后续行动因为关系变化而改道。');
      priorityRisks.push('不要只有关系情绪，没有行动层面的后续影响。');
      break;
    case 'payoff':
      priorityBeats.push('优先安排前文铺垫兑现、收获反馈或阶段性反转，给读者明确回报。');
      priorityBeats.push('兑现后顺手打开下一轮更大的目标或麻烦，不把气口写死。');
      priorityRisks.push('不要只顾爽点回收，忽略代价与后续空间。');
      break;
    case 'balanced':
      priorityBeats.push('推进、情绪、信息释放和回报要彼此穿插，不让单一节拍统治全文。');
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      priorityBeats.push('每个关键段都要写出行动结果和局势变化，避免原地解释。');
      priorityRisks.push('避免设定说明和情绪回旋挤压主线推进。');
      break;
    case 'deepen_character':
      priorityBeats.push('至少安排一次能暴露人物弱点、执念或价值判断的选择。');
      priorityRisks.push('不要把人物塑形写成静态介绍，必须落到行为上。');
      break;
    case 'escalate_conflict':
      priorityBeats.push('让阻力、代价和对立面逐段变强，形成持续抬压链条。');
      priorityRisks.push('避免重复同级冲突，读者会觉得原地踏步。');
      break;
    case 'reveal_mystery':
      priorityBeats.push('优先安排线索出现、误导修正和认知刷新，至少推进一点真相。');
      priorityRisks.push('不要把揭示写成解释堆叠，尽量通过事件和证据推进。');
      break;
    case 'relationship_shift':
      priorityBeats.push('对话、动作和站队变化都要服务关系转折，而不只是口头表态。');
      priorityRisks.push('不要让关系变化只停留在情绪层，没有后续选择代价。');
      break;
    case 'foreshadow_payoff':
      priorityBeats.push('回收时既要兑现前文承诺，也要带出新的悬念或任务。');
      priorityRisks.push('避免只用说明句回收伏笔，最好落在事件结果上。');
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      priorityBeats.push('当前阶段优先扩张局势、铺开变量，并把选择成本逐章抬高。');
      priorityRisks.push('避免太早交底或提前透支高潮。');
      break;
    case 'climax':
      priorityBeats.push('当前阶段要让核心矛盾正面碰撞，把选择逼到无法拖延的节点。');
      priorityRisks.push('避免高潮只有声量，没有清晰结果与代价。');
      break;
    case 'ending':
      priorityBeats.push('当前阶段要优先收束主承诺、主悬念和关键关系线，再留余味。');
      priorityRisks.push('避免只顾收尾，忘了兑现前文最重要的铺垫。');
      break;
    default:
      break;
  }

  const baseBeats = scene === 'outline'
    ? [
      '前段先放出主目标、局势缺口或新任务，不要直接堆设定。',
      '中段持续抬高阻力、代价或信息差，让章节彼此形成递进关系。',
      '后段安排一次明显转折、揭示或关系位移，改变后续走向。',
      '收尾既给阶段性结果，也留下下一轮想追下去的问题。',
    ]
    : [
      '开场尽快抛出异常、目标或受阻点，不做平铺导入。',
      '中段用连续动作推进局势，并让阻力或代价升级。',
      '后段安排一次局势改写、信息刷新或关系位移。',
      '结尾保留明确追读牵引，不要平收。',
    ];

  const baseRisks = scene === 'outline'
    ? ['不要把整轮大纲写成同一种功能，节拍必须有起伏。']
    : ['不要把节拍写成说明书，关键节点都要有动作和即时结果。'];

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲结构蓝图' : '章节结构蓝图',
    summary: comboLabels.length > 0
      ? `当前组合会按「${comboLabels.join(' / ')}」组织节拍。`
      : '当前会按默认节拍组织结构。',
    beats: dedupeItems([...priorityBeats, ...baseBeats]).slice(0, 4),
    risks: dedupeItems([...priorityRisks, ...baseRisks]).slice(0, 2),
  };
}
