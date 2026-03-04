"""提示词管理服务"""
from typing import Dict, Any, Optional
import json


class WritingStyleManager:
    """写作风格管理器"""
    
    @staticmethod
    def apply_style_to_prompt(base_prompt: str, style_content: str) -> str:
        """
        将写作风格应用到基础提示词中
        
        Args:
            base_prompt: 基础提示词
            style_content: 风格要求内容
            
        Returns:
            组合后的提示词
        """
        style_profile = "default"
        normalized = (style_content or "").lower()
        if "连载感" in normalized:
            style_profile = "low_ai_serial"
        elif "生活化" in normalized:
            style_profile = "low_ai_life"
        elif "都市金融" in normalized or ("金融" in normalized and "商战" in normalized):
            style_profile = "urban_finance"
        elif "技术流修仙" in normalized or ("技术流" in normalized and "修仙" in normalized):
            style_profile = "tech_xianxia"
        elif "轻松幽默" in normalized or "幽默" in normalized:
            style_profile = "light_humor"
        elif "朴实年代" in normalized or "年代风" in normalized:
            style_profile = "era_plain"

        common_guard = (
            "写作执行要点："
            "你正在写长篇小说中段，不是开书导语，也不是全书终章。"
            "用中文母语者的自然表达写作，长短句穿插，读起来顺口。"
            "对话要像真人交流，少讲道理，多给反应和潜台词。"
            "出现设定术语时，尽量在场景中补一句通俗解释。"
            "结尾禁止总结型/预告型/感悟型收束，优先停在动作、对话或突发事件上。"
            "直接输出章节正文，不要加章节标题和额外说明。"
        )

        serial_guard = (
            "连载强化要点："
            "保持追更节奏，中段给小波折，章末留自然未完感。"
            "人物情绪要有层次，不要开口就结论化表态。"
            "让配角有主动选择，避免只当信息传声筒。"
        )

        life_guard = (
            "生活化强化要点："
            "优先用动作、表情和场景噪声传递情绪，别把解释写满。"
            "允许少量口语毛边，避免句句工整。"
        )

        urban_finance_guard = (
            "都市金融强化要点："
            "专业术语要落地到利益得失，避免术语堆砌。"
            "谈判和博弈要体现信息差与筹码变化，突出人物选择代价。"
        )

        tech_xianxia_guard = (
            "技术流修仙强化要点："
            "规则推演要清楚，但每段都要有行动反馈，避免连续讲义化解释。"
            "术法/阵法/功法术语出现后，尽量用角色互动补一句人话解释。"
        )

        light_humor_guard = (
            "轻松幽默强化要点："
            "笑点要服务剧情推进，不做连续段子堆叠。"
            "人物互怼要有立场差异，避免全员同口吻抖机灵。"
        )

        era_plain_guard = (
            "朴实年代强化要点："
            "时代细节要自然入戏，优先写可见的生活动作与人际压力。"
            "语言克制朴素，避免现代网络梗和悬浮金句。"
        )

        profile_guard = ""
        if style_profile == "low_ai_serial":
            profile_guard = serial_guard
        elif style_profile == "low_ai_life":
            profile_guard = life_guard
        elif style_profile == "urban_finance":
            profile_guard = urban_finance_guard
        elif style_profile == "tech_xianxia":
            profile_guard = tech_xianxia_guard
        elif style_profile == "light_humor":
            profile_guard = light_humor_guard
        elif style_profile == "era_plain":
            profile_guard = era_plain_guard

        # 在基础提示词末尾追加风格要求与执行护栏
        return f"{base_prompt}\n\n{style_content}\n\n{common_guard}\n{profile_guard}".strip()


class PromptService:
    """提示词模板管理"""
    
    # ========== V2版本提示词模板（RTCO框架）==========
    
    # 世界构建提示词 V2（RTCO框架）
    WORLD_BUILDING = """<system>
你是小说世界观策划搭档，擅长为{genre}类型作品搭出真实、可写、可推进剧情的世界底盘。
</system>

<task>
【设计任务】
为小说《{title}》构建完整的世界观设定。

【核心要求】
- 主题契合：世界观必须支撑主题"{theme}"
- 简介匹配：为简介中的情节提供合理背景
- 类型适配：符合{genre}类型的特征
- 规模适当：根据题材选择合适的设定尺度
- 表达自然：避免百科腔和教科书腔，尽量写成创作可直接使用的描述
</task>

<input priority="P0">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}
简介：{description}
</input>

<guidelines priority="P1">
【类型指导原则】

**现代都市/言情/青春**：
- 时间：当代社会（2020年代）或近未来（2030-2050年）
- 避免：大崩解、纪元、末日等宏大概念
- 重点：具体城市环境、职场文化、社会现状

**历史/古代**：
- 时间：明确的历史朝代或虚构古代
- 重点：时代特征、礼教制度、阶级分化

**玄幻/仙侠/修真**：
- 时间：修炼文明的特定时期
- 重点：修炼规则、灵气环境、门派势力

**科幻**：
- 时间：未来明确时期（如2150年、星际时代初期）
- 重点：科技水平、社会形态、文明转折

**奇幻/魔法**：
- 时间：魔法文明的特定阶段
- 重点：魔法体系、种族关系、大陆格局

**设定尺度控制**：
- 现代都市：聚焦某个城市、行业、阶层
- 校园青春：学校环境、学生生活、成长困境
- 职场言情：公司文化、行业特点、职业压力
- 史诗题材：才需要宏大的世界观架构
</guidelines>

<output priority="P0">
【输出格式】
生成包含以下四个字段的JSON对象，每个字段300-500字：

1. **time_period**（时间背景与社会状态）
   - 根据类型设定合适规模的时间背景
   - 现代题材：具体社会特征（如：2024年北京，互联网行业高速发展）
   - 历史题材：明确朝代和阶段（如：明朝嘉靖年间，海禁政策下的沿海）
   - 幻想题材：文明发展阶段，具体而非空泛
   - 阐明时代核心矛盾和社会焦虑

2. **location**（空间环境与地理特征）
   - 故事主要发生的空间环境
   - 现代题材：具体城市名或类型
   - 环境如何影响居民生存方式
   - 标志性场景描述

3. **atmosphere**（感官体验与情感基调）
   - 身临其境的感官细节（视觉、听觉、嗅觉）
   - 美学风格和色彩基调
   - 居民心理状态和情绪氛围
   - 与主题情感呼应

4. **rules**（世界规则与社会结构）
   - 世界运行的核心法则
   - 现代题材：社会规则、行业潜规则、人际法则
   - 幻想题材:力量体系、社会等级、资源分配
   - 权力结构和利益格局
   - 社会禁忌及后果

【格式规范】
- 纯JSON输出，以{{开始、}}结束
- 无markdown标记、代码块符号
- 字段值为完整段落文本
- 不使用特殊符号包裹内容
- 提供充实原创内容

【JSON示例】
{{
  "time_period": "时间背景与社会状态的详细描述（300-500字）",
  "location": "空间环境与地理特征的详细描述（300-500字）",
  "atmosphere": "感官体验与情感基调的详细描述（300-500字）",
  "rules": "世界规则与社会结构的详细描述（300-500字）"
}}
</output>

<constraints>
【必须遵守】
✅ 简介契合：为简介情节提供合理背景
✅ 类型适配：符合{genre}的特征
✅ 主题贴合：支撑主题"{theme}"
✅ 具象化：用具体细节而非空洞概念
✅ 逻辑自洽：所有设定相互支撑
✅ 叙述可写：给出的设定能直接转成场景、冲突和角色行动

【禁止事项】
❌ 生成与类型不匹配的设定
❌ 为小规模题材使用宏大世界观
❌ 使用模板化、空泛的表达
❌ 输出markdown或代码块标记
❌ 大量使用“首先/其次/最后”式说明文结构
</constraints>"""

    # 批量角色生成提示词 V2（RTCO框架）
    CHARACTERS_BATCH_GENERATION = """<system>
你是小说角色与势力设定搭档，擅长按{genre}题材做出能推动剧情的人物和组织。
</system>

<task>
【生成任务】
生成{count}个角色和组织实体。

【数量要求 - 严格遵守】
数组中必须精确包含{count}个对象，不多不少。

【实体类型分配】
- 至少1个主角（protagonist）
- 多个配角（supporting）
- 可包含反派（antagonist）
- 可包含1-2个高影响力组织（power_level: 70-95）

【写法要求】
- 设定信息要具体可用，避免空泛评价词
- 名字、称谓和关系表达贴近中文网文阅读习惯
- 角色与组织要为后续冲突和推进服务，不做摆设
</task>

<worldview priority="P0">
【世界观信息】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}

主题：{theme}
类型：{genre}
</worldview>

<requirements priority="P1">
【特殊要求】
{requirements}
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON数组，每个对象包含：

**角色对象**：
{{
  "name": "角色姓名",
  "age": 25,
  "gender": "男/女/其他",
  "is_organization": false,
  "role_type": "protagonist/supporting/antagonist",
  "personality": "性格特点（100-200字）：核心性格、优缺点、特殊习惯",
  "background": "背景故事（100-200字）：家庭背景、成长经历、重要转折",
  "appearance": "外貌描述（50-100字）：身高、体型、面容、着装风格",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_array": [
    {{
      "target_character_name": "已生成的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系描述"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已生成的组织名称",
      "position": "职位",
      "rank": 5,
      "loyalty": 80
    }}
  ]
}}

**组织对象**：
{{
  "name": "组织名称",
  "is_organization": true,
  "role_type": "supporting",
  "personality": "组织特性（100-200字）：运作方式、核心理念、行事风格",
  "background": "组织背景（100-200字）：建立历史、发展历程、重要事件",
  "appearance": "外在表现（50-100字）：总部位置、标志性建筑",
  "organization_type": "组织类型",
  "organization_purpose": "组织目的",
  "organization_members": ["成员1", "成员2"],
  "power_level": 85,
  "location": "所在地或主要活动区域",
  "motto": "组织格言、口号或宗旨",
  "color": "代表颜色",
  "traits": []
}}

【关系类型参考】
- 家族：父亲、母亲、兄弟、姐妹、子女、配偶、恋人
- 社交：师父、徒弟、朋友、同学、同事、邻居、知己
- 职业：上司、下属、合作伙伴
- 敌对：敌人、仇人、竞争对手、宿敌

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10（职位等级）
- power_level：70到95（组织影响力）
</output>

<constraints>
【必须遵守】
✅ 数量精确：数组必须包含{count}个对象
✅ 符合世界观：角色设定与世界观一致
✅ 有深度：性格和背景要立体
✅ 关系网络：角色间形成合理关系
✅ 组织合理：组织是推动剧情的关键力量

【关系约束】
✅ relationships_array只能引用本批次已出现的角色
✅ organization_memberships只能引用本批次的组织
✅ 第一个角色的relationships_array必须为空[]
✅ 禁止幻觉：不引用不存在的角色或组织

【格式约束】
✅ 纯JSON数组输出，无markdown标记
✅ 内容描述中严禁使用特殊符号（引号、方括号、书名号等）
✅ 专有名词直接书写，不使用符号包裹

【禁止事项】
❌ 生成数量不符（多于或少于{count}个）
❌ 引用不存在的角色或组织
❌ 生成低影响力的无关紧要组织
❌ 使用markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 用“总之/综上/值得注意的是”这类模板化总结句灌水
</constraints>"""

    # 大纲生成提示词 V2（RTCO框架）
    OUTLINE_CREATE = """<system>
你是小说大纲搭档，擅长为{genre}类型作品设计有钩子的开篇。
</system>

<task>
【创作任务】
为小说《{title}》生成开篇{chapter_count}章的大纲。

【重要说明】
这是项目初始化的开头部分，不是完整大纲：
- 完成开局设定和世界观展示
- 引入主要角色，建立初始关系
- 埋下核心矛盾和悬念钩子
- 为后续剧情发展打下基础
- 不需要完整闭环，为续写留空间

【风格要求】
- 用中文小说读者熟悉的叙事表达，不写公文式说明
- 每章summary尽量“可视化”：有场景、有动作、有冲突触发点
- 避免空泛评价词，优先给可落地的情节信息
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 世界规则要落地到事件层：至少1个关键事件必须体现规则如何限制或放大角色选择
- 本模板只负责产出章节规划内容，不输出调度执行说明或方案对比结论
</task>

<project priority="P0">
【项目信息】
书名：{title}
主题：{theme}
类型：{genre}
开篇章节数：{chapter_count}
叙事视角：{narrative_perspective}
</project>

<worldview priority="P1">
【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<mcp_context priority="P2">
{mcp_references}
</mcp_context>

<requirements priority="P1">
【其他要求】
{requirements}
</requirements>

<fusion_contract priority="P0">
【第三版融合约束（调度边界）】
- 你是内容生成者，不是调度器、评审员或流程记录员
- 禁止输出“执行X.X”“调用Agent”“方案对比/总结”这类调度话术
- 禁止输出流程日志、步骤说明、元叙事解释
- 当信息不足时，先给最小可执行方案：角色目标→阻力→选择→即时后果
</fusion_contract>

<output priority="P0">
【输出格式】
返回包含{chapter_count}个章节对象的JSON数组：

[
  {{
   "chapter_number": 1,
   "title": "章节标题",
   "summary": "章节概要（500-1000字）：主要情节、角色互动、关键事件、冲突与转折",
   "scenes": ["场景1描述", "场景2描述", "场景3描述"],
   "characters": [
     {{"name": "角色名1", "type": "character"}},
     {{"name": "组织/势力名1", "type": "organization"}}
   ],
   "key_points": ["情节要点1", "情节要点2"],
   "emotion": "本章情感基调",
   "goal": "本章叙事目标"
 }},
 {{
   "chapter_number": 2,
   "title": "章节标题",
   "summary": "章节概要...",
   "scenes": ["场景1", "场景2"],
   "characters": [
     {{"name": "角色名2", "type": "character"}},
     {{"name": "组织名2", "type": "organization"}}
   ],
   "key_points": ["要点1", "要点2"],
   "emotion": "情感基调",
   "goal": "叙事目标"
 }}
]

【characters字段说明】
- type为"character"表示个人角色，type为"organization"表示组织/势力/门派/帮派等
- 必须区分角色和组织，不要把组织当作角色

【格式规范】
- 纯JSON数组输出，无markdown标记
- 内容描述中严禁使用特殊符号
- 专有名词直接书写
- 字段结构与已有章节完全一致
</output>

<constraints>
【开篇大纲要求】
✅ 开局设定：前几章完成世界观呈现、主角登场、初始状态
✅ 矛盾引入：引出核心冲突，但不急于展开
✅ 角色亮相：主要角色依次登场，展示性格和关系
✅ 节奏控制：开篇不宜过快，给读者适应时间
✅ 悬念设置：埋下伏笔和钩子，为续写预留空间
✅ 视角统一：采用{narrative_perspective}视角
✅ 留白艺术：结尾不收束过紧，留发展空间
✅ 冲突有压强：每章都有清晰阻力，不让情节平推
✅ 规则有作用：关键事件中体现世界规则对行动成本/风险的影响

【必须遵守】
✅ 数量精确：数组包含{chapter_count}个章节对象
✅ 符合类型：情节符合{genre}类型特征
✅ 主题贴合：体现主题"{theme}"
✅ 开篇定位：是开局而非完整故事
✅ 描述详细：每个summary 500-1000字
✅ summary可落地：读完能直接用于后续写章
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 试图在开篇完结故事
❌ 节奏过快，信息过载
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 章节只有信息介绍，没有目标冲突和决策后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""
    
    # 大纲续写提示词 V2（RTCO框架 - 简化版）
    OUTLINE_CONTINUE = """<system>
你是小说续写大纲搭档，擅长在不跑偏的前提下推进{genre}类型剧情。
</system>

<task>
【续写任务】
基于已有{current_chapter_count}章内容，续写第{start_chapter}章到第{end_chapter}章的大纲（共{chapter_count}章）。

【当前情节阶段】
{plot_stage_instruction}

【故事发展方向】
{story_direction}

【风格要求】
- 续写语言保持中文小说叙事习惯，不写说明书式大纲
- 每章summary优先写“发生了什么”，而不是“要表达什么”
- 同时给出冲突推进与情绪变化，避免流水账
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 世界规则要落地到事件层：至少1个关键事件必须体现规则如何限制或放大角色选择
- 本模板只负责产出章节规划内容，不输出调度执行说明或方案对比结论
</task>

<project priority="P0">
【项目信息】
书名：{title}
主题：{theme}
类型：{genre}
叙事视角：{narrative_perspective}
</project>

<worldview priority="P1">
【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<previous_context priority="P0">
{recent_outlines}
</previous_context>

<characters priority="P0">
【所有角色信息】
{characters_info}
</characters>

<user_input priority="P0">
【用户输入】
续写章节数：{chapter_count}章
情节阶段：{plot_stage_instruction}
故事方向：{story_direction}
其他要求：{requirements}
</user_input>

<fusion_contract priority="P0">
【第三版融合约束（大纲续写）】
- 你是内容生成者，不是调度器、评审员或流程记录员
- 禁止输出“执行X.X”“调用Agent”“方案对比/总结”这类调度话术
- 禁止输出流程日志、步骤说明、元叙事解释
- 当信息不足时，先给最小可执行方案：角色目标→阻力→选择→即时后果
</fusion_contract>

<mcp_context priority="P2">
{mcp_references}
</mcp_context>

<output priority="P0">
【输出格式】
返回第{start_chapter}到第{end_chapter}章的JSON数组（共{chapter_count}个对象）：

[
  {{
   "chapter_number": {start_chapter},
   "title": "章节标题",
   "summary": "章节概要（500-1000字）：主要情节、角色互动、关键事件、冲突与转折",
   "scenes": ["场景1描述", "场景2描述", "场景3描述"],
   "characters": [
     {{"name": "角色名1", "type": "character"}},
     {{"name": "组织/势力名1", "type": "organization"}}
   ],
   "key_points": ["情节要点1", "情节要点2"],
   "emotion": "本章情感基调",
   "goal": "本章叙事目标"
 }},
 {{
   "chapter_number": {start_chapter} + 1,
   "title": "章节标题",
   "summary": "章节概要...",
   "scenes": ["场景1", "场景2"],
   "characters": [
     {{"name": "角色名2", "type": "character"}},
     {{"name": "组织名2", "type": "organization"}}
   ],
   "key_points": ["要点1", "要点2"],
   "emotion": "情感基调",
   "goal": "叙事目标"
 }}
]

【characters字段说明】
- type为"character"表示个人角色，type为"organization"表示组织/势力/门派/帮派等
- 必须区分角色和组织，不要把组织当作角色

【格式规范】
- 纯JSON数组输出，无markdown标记
- 内容描述中严禁使用特殊符号
- 专有名词直接书写
- 字段结构与已有章节完全一致
</output>

<constraints>
【续写要求】
✅ 剧情连贯：与前文自然衔接，保持连贯性
✅ 角色发展：遵循角色成长轨迹，充分利用角色信息
✅ 情节阶段：遵循{plot_stage_instruction}的要求
✅ 风格一致：保持与已有章节相同风格和详细程度
✅ 大纲详细：充分解析最近10章大纲的structure字段信息
✅ 冲突升级：每章有新的阻力或风险上升，不做平铺推进
✅ 规则落地：关键事件体现世界规则的约束、代价或红利

【必须遵守】
✅ 数量精确：数组包含{chapter_count}个章节
✅ 编号正确：从第{start_chapter}章开始
✅ 描述详细：每个summary 500-1000字
✅ 承上启下：自然衔接前文
✅ 情节可执行：summary能直接转成章节正文
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 与前文矛盾或脱节
❌ 忽略已有角色发展
❌ 忽略最近大纲中的情节线索
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 把世界规则当背景板，不作用于角色选择与冲突结果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""
    
    # 章节生成 - 1-N模式（第1章）
    CHAPTER_GENERATION_ONE_TO_MANY = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task>
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：至少给1名核心配角安排一次反预期行为，并在当场补一句动机解释
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲 - 作为主线】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<characters priority="P1">
【本章角色 - 保持人设稳定】
{characters_info}

⚠️ 角色互动须知：
- 角色之间的对话和行为必须符合其关系设定（如师徒、敌对等）
- 涉及组织的情节须体现角色在组织中的身份和职位
- 角色的能力表现须符合其职业和阶段设定
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文生成）】
- 信息冲突时按优先级处理：本章大纲 > 上下文锚点 > 相关记忆 > 风格补充
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 保持角色性格、说话方式一致
✅ 角色互动须符合关系设定（师徒、朋友、敌对等）
✅ 组织相关情节须体现成员身份和职位层级
✅ 字数控制在目标范围内
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 配角有转折：至少1名核心配角在本章出现一次反预期选择，并紧跟一句动机说明
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 添加作者注释或创作说明
❌ 角色行为超出其职业阶段的能力范围
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 把角色写成只负责报信息的工具人
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-1模式（第1章）
    CHAPTER_GENERATION_ONE_TO_ONE = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task priority="P0">
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：至少给1名核心配角安排一次反预期行为，并在当场补一句动机解释
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<characters priority="P1">
【本章角色】
{characters_info}
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文生成）】
- 信息冲突时按优先级处理：本章大纲 > 上下文锚点 > 相关记忆 > 风格补充
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 保持角色性格、说话方式一致
✅ 字数尽量贴近目标字数（允许小幅波动）
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 配角有转折：至少1名核心配角在本章出现一次反预期选择，并紧跟一句动机说明
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 添加作者注释或创作说明
❌ 不要明显超出目标字数
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 把角色写成只负责报信息的工具人
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-1模式（第2章及以后）
    CHAPTER_GENERATION_ONE_TO_ONE_NEXT = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task priority="P0">
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：至少给1名核心配角安排一次反预期行为，并在当场补一句动机解释
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<previous_chapter_summary priority="P1">
【上一章剧情概要】
{previous_chapter_summary}
</previous_chapter_summary>

<previous_chapter priority="P1">
【上一章末尾500字内容】
{previous_chapter_content}
</previous_chapter>

<characters priority="P1">
【本章角色】
{characters_info}
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文续写）】
- 信息冲突时按优先级处理：本章大纲 > 上章末尾内容 > 上章剧情概要 > 相关记忆
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 自然承接上一章末尾内容，保持连贯性
✅ 保持角色性格、说话方式一致
✅ 字数尽量贴近目标字数（允许小幅波动）
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 配角有转折：至少1名核心配角在本章出现一次反预期选择，并紧跟一句动机说明
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 添加作者注释或创作说明
❌ 重复上一章已发生的事件
❌ 不要明显超出目标字数
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 把角色写成只负责报信息的工具人
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-N模式（第2章及以后）
    CHAPTER_GENERATION_ONE_TO_MANY_NEXT = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task>
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：至少给1名核心配角安排一次反预期行为，并在当场补一句动机解释
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲 - 作为主线】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<recent_context priority="P1">
【最近章节规划 - 故事脉络参考】
{recent_chapters_context}
</recent_context>

<continuation priority="P0">
【衔接锚点 - 用来接续】
上一章结尾：
「{continuation_point}」

【上一章已完成剧情（避免重复）】
{previous_chapter_summary}

注意：
1. 上述"已完成剧情"和"衔接锚点"是**已经写过的**内容
2. 本章必须推进到**新的情节点**，不要重复叙述已经发生的事件
3. 如果锚点是对话结束，请描写对话后的动作或场景转换，不要重复对话
4. 如果锚点是场景描写，请直接开始人物行动，不要重复描写环境
</continuation>

<characters priority="P1">
【本章角色 - 保持人设稳定】
{characters_info}

⚠️ 角色互动须知：
- 角色之间的对话和行为必须符合其关系设定（如师徒、敌对等）
- 涉及组织的情节须体现角色在组织中的身份和职位
- 角色的能力表现须符合其职业和阶段设定
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P1">
【🎯 伏笔提醒 - 需关注】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆 - 参考】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文续写）】
- 信息冲突时按优先级处理：本章大纲 > 上章已完成剧情 > 衔接锚点 > 最近章节规划 > 相关记忆
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 自然承接上一章结尾，不重复已发生事件
✅ 保持角色性格、说话方式一致
✅ 角色互动须符合关系设定（师徒、朋友、敌对等）
✅ 组织相关情节须体现成员身份和职位层级
✅ 字数控制在目标范围内
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 配角有转折：至少1名核心配角在本章出现一次反预期选择，并紧跟一句动机说明

【🔴 反重复特别指令】
✅ 检查本章开篇是否与"衔接锚点"内容重复
✅ 检查本章情节是否与"上一章已完成剧情"重复
✅ 确保本章推进到了大纲中规划的新事件
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 添加作者注释或创作说明
❌ 重复叙述上一章已发生的事件（包括环境描写、心理活动）
❌ 在开篇使用"接上回"、"书接上文"等套话
❌ 角色行为超出其职业阶段的能力范围
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 把角色写成只负责报信息的工具人
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 单个角色生成提示词 V2（RTCO框架）
    SINGLE_CHARACTER_GENERATION = """<system>
你是小说角色设定搭档，擅长做出真实、有戏、能推动剧情的人物。
</system>

<task>
【设计任务】
根据用户需求和项目上下文，创建一个完整的角色设定。

【写法要求】
- 先解决“这个人能在故事里干什么”，再补性格与背景
- 避免模板化夸赞，优先给可落地的行为特征
- 描述贴近中文小说阅读习惯，不写教科书腔
</task>

<context priority="P0">
【项目上下文】
{project_context}

【用户需求】
{user_input}
</context>

<output priority="P0">
【输出格式】
生成完整的角色卡片JSON对象：

{{
  "name": "角色姓名（如用户未提供则生成符合世界观的名字）",
  "age": "年龄（具体数字或年龄段）",
  "gender": "男/女/其他",
  "appearance": "外貌描述（100-150字）：身高体型、面容特征、着装风格",
  "personality": "性格特点（150-200字）：核心性格特质、优缺点、特殊习惯",
  "background": "背景故事（200-300字）：家庭背景、成长经历、重要转折、与主题关联",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_text": "人际关系的自然语言描述",
  "relationships": [
    {{
      "target_character_name": "已存在的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系的详细描述",
      "started_at": "关系开始的故事时间点（可选）"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已存在的组织名称",
      "position": "职位名称",
      "rank": 8,
      "loyalty": 80,
      "joined_at": "加入时间（可选）",
      "status": "active"
    }}
  ],
  "career_info": {{
    "main_career_name": "从可用主职业列表中选择的职业名称",
    "main_career_stage": 5,
    "sub_careers": [
      {{
        "career_name": "从可用副职业列表中选择的职业名称",
        "stage": 3
      }}
    ]
  }}
}}

【职业信息说明】
如果项目上下文包含职业列表：
- 主职业：从"可用主职业"列表中选择最符合角色的职业
- 主职业阶段：根据角色实力设定合理阶段（1到max_stage）
- 副职业：可选择0-2个副职业
- ⚠️ 填写职业名称而非ID，系统会自动匹配
- 职业选择必须与角色背景、能力和定位高度契合

【关系类型参考】
- 家族：父亲、母亲、兄弟、姐妹、子女、配偶、恋人
- 社交：师父、徒弟、朋友、同学、同事、邻居、知己
- 职业：上司、下属、合作伙伴
- 敌对：敌人、仇人、竞争对手、宿敌

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10
</output>

<constraints>
【必须遵守】
✅ 符合世界观：角色设定与项目世界观一致
✅ 主题关联：背景故事与项目主题关联
✅ 立体饱满：性格复杂有矛盾性，不脸谱化
✅ 为故事服务：设定要推动剧情发展
✅ 职业匹配：职业选择与角色高度契合

【角色定位要求】
✅ 主角：有成长空间和目标动机
✅ 反派：有合理动机，不脸谱化
✅ 配角：有独特性，不是工具人

【关系约束】
✅ relationships只引用已存在的角色
✅ organization_memberships只引用已存在的组织
✅ 无关系或组织时对应数组为空[]

【格式约束】
✅ 纯JSON对象输出，无markdown标记
✅ 内容描述中严禁使用特殊符号
✅ 专有名词直接书写

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号（引号、方括号等）
❌ 引用不存在的角色或组织
❌ 脸谱化的角色设定
❌ 使用“总之/综上/值得注意的是”这类模板连接句
</constraints>"""

    # 单个组织生成提示词 V2（RTCO框架）
    SINGLE_ORGANIZATION_GENERATION = """<system>
你是小说组织设定搭档，擅长创建有目标、有规则、有戏剧价值的势力。
</system>

<task>
【设计任务】
根据用户需求和项目上下文，创建一个完整的组织/势力设定。

【写法要求】
- 组织设定要能直接用于剧情，不做纯背景板
- 描述优先具体行动逻辑，少抽象口号
- 语言自然，避免模板腔
</task>

<context priority="P0">
【项目上下文】
{project_context}

【用户需求】
{user_input}
</context>

<output priority="P0">
【输出格式】
生成完整的组织设定JSON对象：

{{
  "name": "组织名称（如用户未提供则生成符合世界观的名称）",
  "is_organization": true,
  "organization_type": "组织类型（帮派/公司/门派/学院/政府机构/宗教组织等）",
  "personality": "组织特性（150-200字）：核心理念、行事风格、文化价值观、运作方式",
  "background": "组织背景（200-300字）：建立历史、发展历程、重要事件、当前地位",
  "appearance": "外在表现（100-150字）：总部位置、标志性建筑、组织标志、制服等",
  "organization_purpose": "组织目的和宗旨：明确目标、长期愿景、行动准则",
  "power_level": 75,
  "location": "所在地点：主要活动区域、势力范围",
  "motto": "组织格言或口号",
  "traits": ["特征1", "特征2", "特征3"],
  "color": "组织代表颜色（如：深红色、金色、黑色等）",
  "organization_members": ["重要成员1", "重要成员2", "重要成员3"]
}}

【字段说明】
- power_level：0-100的整数，表示在世界中的影响力
- organization_members：组织内重要成员名字列表（可关联已有角色）
- 成立时间：在background中描述
</output>

<constraints>
【必须遵守】
✅ 符合世界观：组织设定与项目世界观一致
✅ 主题关联：背景与项目主题关联
✅ 推动剧情：组织能推动故事发展
✅ 有层级结构：内部有明确的层级和结构
✅ 势力互动：与其他势力有互动关系

【组织定位要求】
✅ 有存在必要性：不是可有可无的背景板
✅ 目标合理：不过于理想化或脸谱化
✅ 具体细节：描述详细具体，避免空泛

【格式约束】
✅ 纯JSON对象输出，无markdown标记
✅ 内容描述中严禁使用特殊符号
✅ 专有名词直接书写

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号（引号、方括号等）
❌ 过于理想化或脸谱化的设定
❌ 空泛的描述
❌ 使用“总之/综上/值得注意的是”这类模板连接句
</constraints>"""

    # 情节分析提示词 V2（RTCO框架 + 伏笔ID追踪）
    PLOT_ANALYSIS = """<system>
你是小说剧情复盘搭档，擅长把章节里的钩子、冲突和伏笔拆清楚。
</system>

<task>
【分析任务】
全面分析第{chapter_number}章《{title}》的剧情要素、钩子、伏笔、冲突和角色发展。

【表达要求】
- 分析结论要具体，避免空泛评价
- 术语可用，但描述保持自然中文表达
- 只输出结构化分析结果，不输出流程说明、调度话术或自我评注

【🔴 伏笔追踪任务（重要）】
系统已提供【已埋入伏笔列表】，当你识别到章节中有回收伏笔时：
1. 必须从列表中找出对应的伏笔ID
2. 在 foreshadows 数组中使用 reference_foreshadow_id 字段关联
3. 如果无法确定是哪个伏笔，reference_foreshadow_id 填 null
</task>

<chapter priority="P0">
【章节信息】
章节：第{chapter_number}章
标题：{title}
字数：{word_count}字

【章节内容】
{content}
</chapter>

<existing_foreshadows priority="P1">
【已埋入伏笔列表 - 用于回收匹配】
以下是本项目中已埋入但尚未回收的伏笔，分析时如发现章节内容回收了某个伏笔，请使用对应的ID：

{existing_foreshadows}
</existing_foreshadows>

<characters priority="P1">
【项目角色信息 - 用于角色状态分析】
以下是项目中已有的角色列表，分析 character_states 和 relationship_changes 时请使用这些角色的准确名称：

{characters_info}
</characters>

<analysis_framework priority="P0">
【分析维度】

**1. 剧情钩子 (Hooks)**
识别吸引读者的关键元素：
- 悬念钩子：未解之谜、疑问、谜团
- 情感钩子：引发共鸣的情感点
- 冲突钩子：矛盾对抗、紧张局势
- 认知钩子：颠覆认知的信息

每个钩子需要：
- 类型分类
- 具体内容描述
- 强度评分(1-10)
- 出现位置(开头/中段/结尾)
- **关键词**：【必填】从原文逐字复制8-25字的文本片段，用于精确定位

**2. 伏笔分析 (Foreshadowing) - 🔴 支持ID追踪**
- 埋下的新伏笔：内容、预期作用、隐藏程度(1-10)
- 回收的旧伏笔：【必须】从已埋入伏笔列表中匹配ID
- 伏笔质量：巧妙性和合理性
- **关键词**：【必填】从原文逐字复制8-25字

每个伏笔需要：
- **title**：简洁标题（10-20字，概括伏笔核心）
  - ⚠️ 回收伏笔时，标题应与原伏笔标题保持一致，不要添加"回收"等后缀
  - 例如：原伏笔标题是"绿头发的视觉符号"，回收时标题仍为"绿头发的视觉符号"，而非"绿头发的视觉符号回收"
- **content**：详细描述伏笔内容和预期作用
- **type**：planted（埋下）或 resolved（回收）
- **strength**：强度1-10（对读者的吸引力）
- **subtlety**：隐藏度1-10（越高越隐蔽）
- **reference_chapter**：回收时引用的原埋入章节号，埋下时为null
- **reference_foreshadow_id**：【回收时必填】被回收伏笔的ID（从已埋入伏笔列表中选择），埋下时为null
  - 🔴 重要：回收伏笔时，必须从【已埋入伏笔列表】中找到对应的伏笔ID并填写
  - 如果列表中有标注【ID: xxx】的伏笔，回收时必须使用该ID
  - 如果无法确定是哪个伏笔，才填写null（但应尽量避免）
- **keyword**：【必填】从原文逐字复制8-25字的定位文本
- **category**：分类（identity=身世/mystery=悬念/item=物品/relationship=关系/event=事件/ability=能力/prophecy=预言）
- **is_long_term**：是否长线伏笔（跨10章以上回收为true）
- **related_characters**：涉及的角色名列表
- **estimated_resolve_chapter**：【必填】预估回收章节号（埋下时必须预估，回收时为当前章节）

**3. 冲突分析 (Conflict)**
- 冲突类型：人与人/人与己/人与环境/人与社会
- 冲突各方及立场
- 冲突强度(1-10)
- 解决进度(0-100%)

**4. 情感曲线 (Emotional Arc)**
- 主导情绪（最多10字）
- 情感强度(1-10)
- 情绪变化轨迹

**5. 角色状态追踪 (Character Development)**
对每个出场角色分析：
- 心理状态变化(前→后)
- 关系变化
- 关键行动和决策
- 成长或退步
- **💀 存活状态（重要）**：
  - survival_status: 角色当前存活状态
  - 可选值：active(正常)/deceased(死亡)/missing(失踪)/retired(退场)
  - 默认为null（表示无变化），仅当章节中角色明确死亡、失踪或永久退场时才填写
  - 死亡/失踪需要有明确的剧情依据，不可臆测
- ** 职业变化（可选）**：
  - 仅当章节明确描述职业进展时填写
  - main_career_stage_change: 整数(+1晋升/-1退步/0无变化)
  - sub_career_changes: 副职业变化数组
  - new_careers: 新获得职业
  - career_breakthrough: 突破过程描述
- **🏛️ 组织变化（可选）**：
  - 仅当章节明确描述角色与组织关系变化时填写
  - organization_changes: 组织变动数组
  - 每项包含：organization_name(组织名)、change_type(加入joined/离开left/晋升promoted/降级demoted/开除expelled/叛变betrayed)、new_position(新职位，可选)、loyalty_change(忠诚度变化描述，可选)、description(变化描述)

**5b. 组织状态追踪 (Organization Status) - 可选**
仅当章节涉及组织势力变化时填写，分析出场组织的状态变化：
- 组织名称
- 势力等级变化(power_change: 整数，+N增强/-N削弱/0无变化)
- 据点变化(new_location: 新据点，可选)
- 宗旨/目标变化(new_purpose: 新目标，可选)
- 组织状态描述(status_description: 当前状态概述)
- 关键事件(key_event: 触发变化的事件)
- **💀 组织存续状态（重要）**：
  - is_destroyed: 组织是否被覆灭（true/false，默认false）
  - 仅当章节明确描述组织被彻底消灭、瓦解、灭亡时设为true

**6. 关键情节点 (Plot Points)**
列出3-5个核心情节点：
- 情节内容
- 类型(revelation/conflict/resolution/transition)
- 重要性(0.0-1.0)
- 对故事的影响
- **关键词**：【必填】从原文逐字复制8-25字

**7. 场景与节奏**
- 主要场景
- 叙事节奏(快/中/慢)
- 对话与描写比例

**8. 质量评分（支持小数，严格区分度）**
评分范围：1.0-10.0，支持一位小数（如 6.5、7.8）
每个维度必须根据以下标准严格评分，避免所有内容都打中等分数：

**节奏把控 (pacing)**：
- 1.0-3.9（差）：节奏混乱，该快不快该慢不慢；场景切换生硬；大段无意义描写拖沓
- 4.0-5.9（中下）：节奏基本可读但有明显问题；部分场景过于冗长或仓促
- 6.0-7.9（中上）：节奏整体流畅，偶有小问题；张弛有度但不够精妙
- 8.0-9.4（优秀）：节奏把控精准，高潮迭起；场景切换自然，详略得当
- 9.5-10.0（完美）：节奏大师级，每个段落都恰到好处

**吸引力 (engagement)**：
- 1.0-3.9（差）：内容乏味，缺乏钩子；读者难以继续阅读
- 4.0-5.9（中下）：有基本情节但缺乏亮点；钩子设置生硬或缺失
- 6.0-7.9（中上）：有一定吸引力，钩子有效但不够巧妙
- 8.0-9.4（优秀）：引人入胜，钩子设置精妙；让人欲罢不能
- 9.5-10.0（完美）：极具吸引力，每个段落都有阅读动力

**连贯性 (coherence)**：
- 1.0-3.9（差）：逻辑混乱，前后矛盾；角色行为不合理
- 4.0-5.9（中下）：基本连贯但有明显漏洞；部分情节衔接生硬
- 6.0-7.9（中上）：整体连贯，偶有小瑕疵；角色行为基本合理
- 8.0-9.4（优秀）：逻辑严密，衔接自然；角色行为高度一致
- 9.5-10.0（完美）：无懈可击的连贯性

**整体质量 (overall)**：
- 计算公式：(pacing + engagement + coherence) / 3，保留一位小数
- 可根据综合印象±0.5调整，必须与各项分数保持一致性

**9. 改进建议（与分数关联）**
建议数量必须与整体质量分数关联：
- overall < 4.0：必须提供4-5条具体改进建议，指出严重问题
- overall 4.0-5.9：必须提供3-4条改进建议，指出主要问题
- overall 6.0-7.9：提供1-2条优化建议，指出可提升之处
- overall ≥ 8.0：提供0-1条锦上添花的建议

每条建议必须：
- 指出具体问题位置或类型
- 说明为什么是问题
- 给出明确的改进方向
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（剧情分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”类流程文本
- 信息冲突时按优先级处理：章节内容 > 已埋入伏笔列表 > 角色信息 > 评分经验规则
- 证据不足时使用保守填法：字段置null/空数组，不臆造不存在的剧情事实
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（无markdown标记）：

{{
  "hooks": [
    {{
      "type": "悬念",
      "content": "具体描述",
      "strength": 8,
      "position": "中段",
      "keyword": "从原文逐字复制的8-25字文本"
    }}
  ],
  "foreshadows": [
    {{
      "title": "伏笔简洁标题",
      "content": "伏笔详细内容和预期作用",
      "type": "planted",
      "strength": 7,
      "subtlety": 8,
      "reference_chapter": null,
      "reference_foreshadow_id": null,
      "keyword": "从原文逐字复制的8-25字文本",
      "category": "mystery",
      "is_long_term": false,
      "related_characters": ["角色A", "角色B"],
      "estimated_resolve_chapter": 15
    }},
    {{
      "title": "回收的伏笔标题",
      "content": "伏笔如何被回收的描述",
      "type": "resolved",
      "strength": 8,
      "subtlety": 6,
      "reference_chapter": 5,
      "reference_foreshadow_id": "abc123-已埋入伏笔的ID",
      "keyword": "从原文逐字复制的8-25字文本",
      "category": "mystery",
      "is_long_term": false,
      "related_characters": ["角色A"],
      "estimated_resolve_chapter": 10
    }}
  ],
  "conflict": {{
    "types": ["人与人", "人与己"],
    "parties": ["主角-复仇", "反派-维护现状"],
    "level": 8,
    "description": "冲突描述",
    "resolution_progress": 0.3
  }},
  "emotional_arc": {{
    "primary_emotion": "紧张焦虑",
    "intensity": 8,
    "curve": "平静→紧张→高潮→释放",
    "secondary_emotions": ["期待", "焦虑"]
  }},
  "character_states": [
    {{
      "character_name": "张三",
      "survival_status": null,
      "state_before": "犹豫",
      "state_after": "坚定",
      "psychological_change": "心理变化描述",
      "key_event": "触发事件",
      "relationship_changes": {{"李四": "关系改善"}},
      "career_changes": {{
        "main_career_stage_change": 1,
        "sub_career_changes": [{{"career_name": "炼丹", "stage_change": 1}}],
        "new_careers": [],
        "career_breakthrough": "突破描述"
      }},
      "organization_changes": [
        {{
          "organization_name": "某门派",
          "change_type": "promoted",
          "new_position": "长老",
          "loyalty_change": "忠诚度提升",
          "description": "因立下大功被提拔为长老"
        }}
      ]
    }}
  ],
  "plot_points": [
    {{
      "content": "情节点描述",
      "type": "revelation",
      "importance": 0.9,
      "impact": "推动故事发展",
      "keyword": "从原文逐字复制的8-25字文本"
    }}
  ],
  "scenes": [
    {{
      "location": "地点",
      "atmosphere": "氛围",
      "duration": "时长估计"
    }}
  ],
  "organization_states": [
    {{
      "organization_name": "某门派",
      "power_change": -10,
      "new_location": null,
      "new_purpose": null,
      "status_description": "因内乱势力受损，但核心力量未动摇",
      "key_event": "长老叛变导致分支瓦解",
      "is_destroyed": false
    }}
  ],
  "pacing": "varied",
  "dialogue_ratio": 0.4,
  "description_ratio": 0.3,
  "scores": {{
    "pacing": 6.5,
    "engagement": 5.8,
    "coherence": 7.2,
    "overall": 6.5,
    "score_justification": "节奏整体流畅但中段略显拖沓；钩子设置有效但不够巧妙；逻辑连贯无明显漏洞"
  }},
  "plot_stage": "发展",
  "suggestions": [
    "【节奏问题】第三场景的心理描写过长（约500字），建议精简至200字以内，保留核心情感即可",
    "【吸引力不足】章节中段缺乏有效钩子，建议在主角发现线索后增加一个小悬念"
  ]
}}
</output>

<constraints>
【必须遵守】
✅ keyword字段必填：钩子、伏笔、情节点的keyword不能为空
✅ 逐字复制：keyword必须从原文复制，长度8-25字
✅ 精确定位：keyword能在原文中精确找到
✅ 职业变化可选：仅当章节明确描述时填写
✅ 组织变化可选：仅当章节明确描述角色与组织关系变动时填写（character_states中的organization_changes）
✅ 组织状态可选：仅当章节明确描述组织势力/据点/目标变化时填写（organization_states顶级字段）
✅ 存活状态谨慎：survival_status仅当章节有明确死亡/失踪/退场描写时填写，默认null
✅ 组织覆灭谨慎：is_destroyed仅当组织被彻底消灭时设true，组织受损不算覆灭
✅ 【伏笔ID追踪】回收伏笔时，必须从【已埋入伏笔列表】中查找匹配的ID填入 reference_foreshadow_id

【评分约束 - 严格执行】
✅ 严格按评分标准打分，支持小数（如6.5、7.2、8.3）
✅ 不要默认给7.0-8.0分，差的内容必须给低分（1.0-5.0），好的内容才给高分（8.0-10.0）
✅ score_justification必填：简要说明各项评分的依据
✅ 建议数量必须与overall分数关联：
   - overall≤4.0 → 4-5条建议
   - overall 4.0-6.0 → 3-4条建议
   - overall 6.0-8.0 → 1-2条建议
   - overall≥8.0 → 0-1条建议
✅ 每条建议必须标注问题类型（如【节奏问题】【描写不足】等）
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时可返回null或空数组，不得杜撰事实
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ keyword使用概括或改写的文字
❌ 输出markdown标记
❌ 遗漏必填的keyword字段
❌ 无根据地添加职业变化
❌ 无根据地添加组织变化或组织状态变化
❌ 无确切剧情依据地标记角色死亡或组织覆灭
❌ 所有章节都打7-8分的"安全分"
❌ 高分章节给大量建议，或低分章节不给建议
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 正文质量检查提示词（第三版融合）
    CHAPTER_TEXT_CHECKER = """<system>
你是小说正文质量检查专家，负责找出可导致阅读体验下降的关键问题，并给出可执行修正建议。
</system>

<task>
【检查任务】
对第{chapter_number}章《{chapter_title}》进行结构化质检。

【检查重点】
1. 设定一致性：世界规则、角色能力边界、关系逻辑是否冲突
2. 连贯性：上下文衔接是否自然，是否有重复叙述或断裂
3. 叙事有效性：是否存在大段解释腔、模板腔、空泛总结
4. 对话质量：人物声线是否区分，是否出现轮流讲设定
5. 结尾质量：是否出现感悟式/预告式/全知式收束
6. 可读性：术语密度过高是否缺乏人话解释
</task>

<context priority="P0">
【章节正文】
{chapter_content}
</context>

<anchors priority="P1">
【本章大纲锚点】
{chapter_outline}

【角色信息】
{characters_info}

【世界规则】
{world_rules}
</anchors>

<fusion_contract priority="P0">
【第三版融合约束（正文检查）】
- 只输出JSON，不输出执行步骤、调度术语、自我评注
- 信息冲突时按优先级：章节正文 > 大纲锚点 > 角色信息 > 世界规则
- 证据不足时保持保守：不要杜撰问题，不要强行判错
</fusion_contract>

<output>
【输出格式】
返回纯JSON对象（无markdown）：

{{
  "overall_assessment": "优秀|良好|一般|较差|存在严重问题",
  "severity_counts": {{
    "critical": 0,
    "major": 0,
    "minor": 0
  }},
  "issues": [
    {{
      "severity": "critical|major|minor",
      "category": "设定冲突|逻辑连贯|角色失真|文风表达|对话质量|结尾处理|术语可读性",
      "location": "位置描述（段落/片段）",
      "evidence": "原文证据（可截断）",
      "impact": "问题影响",
      "suggestion": "可直接执行的修正建议"
    }}
  ],
  "priority_actions": [
    "优先修改项1",
    "优先修改项2"
  ],
  "revision_suggestions": [
    "建议1",
    "建议2"
  ]
}}
</output>

<constraints>
【必须遵守】
✅ issues最多输出8条，按严重度降序排列
✅ revision_suggestions最多输出8条，必须是可执行建议
✅ severity_counts必须与issues中的严重度统计一致
✅ location必须可定位，避免“某处/部分地方”这类模糊描述
✅ suggestion必须能直接用于改写，避免空话
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown、代码块、流程日志
❌ 输出“执行X.X/调用Agent/方案A-B/复盘”等调度文本
❌ 没有证据时硬判错误
❌ 输出与正文无关的泛化建议
</constraints>"""

    # 正文自动修订提示词（第三版融合，生成修订草案，不直接落库正文）
    CHAPTER_TEXT_REVISER = """<system>
你是小说正文修订专家。请根据质检报告优先修复严重问题，保持原文风格与剧情主线不变。
</system>

<task>
【修订任务】
对第{chapter_number}章《{chapter_title}》生成一版“可直接替换”的修订草案。

【修订原则】
1. 先修复所有严重问题（critical），再酌情修复中等问题
2. 最小改动优先：能改一句不改一段，避免剧情跑偏
3. 不新增片段外重大事件，不改角色核心关系和能力边界
4. 维持原文风格、人称和叙事节奏
5. 严禁流程化元文本与说明腔
</task>

<context priority="P0">
【原章节正文】
{chapter_content}

【严重问题清单（优先修复）】
{critical_issues_text}
</context>

<checker priority="P1">
【完整质检结果（JSON）】
{checker_result_json}
</checker>

<fusion_contract priority="P0">
【第三版融合约束（正文修订）】
- 只输出JSON结果，不输出执行步骤、调度术语、自我评注
- 信息冲突时按优先级：原章节正文 > 严重问题清单 > 完整质检结果
- 若某问题证据不足，标记为未解决，不强行改写
</fusion_contract>

<output>
【输出格式】
返回纯JSON对象（无markdown）：

{{
  "revised_text": "修订后的完整正文",
  "applied_issues": [
    "已修复问题1",
    "已修复问题2"
  ],
  "unresolved_issues": [
    "未解决问题及原因"
  ],
  "change_summary": "一句话说明本次修订重点"
}}
</output>

<constraints>
【必须遵守】
✅ revised_text必须是完整正文，不得夹带说明文字
✅ applied_issues/unresolved_issues 均为字符串数组，最多各8条
✅ unresolved_issues 只记录无法安全修复的问题，不得空泛
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown、代码块、流程日志
❌ 输出“执行X.X/调用Agent/方案A-B/复盘”等调度文本
❌ 改写为导语、总结、预告式文案
❌ 新增原文不存在的重大剧情转折
</constraints>"""

    # 大纲单批次展开提示词 V2（RTCO框架）
    OUTLINE_EXPAND_SINGLE = """<system>
你是小说大纲展开搭档，擅长把一个大纲节点拆成可直接动笔的章节规划。
</system>

<task>
【展开任务】
将第{outline_order_index}节大纲《{outline_title}》展开为{target_chapter_count}个章节的详细规划。

【展开策略】
{strategy_instruction}

【风格要求】
- `plot_summary` 按“谁在什么场景做了什么、引发什么后果”来写
- 叙述使用中文网文常见表达，避免说明书腔
- 每章都要有可写性，不能只写抽象目标
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 关键事件要落地世界规则：体现规则如何限制、放大或扭转角色选择
- 只输出章节规划JSON，不输出流程说明、调度说明、创作解释
</task>

<project priority="P1">
【项目信息】
小说名称：{project_title}
类型：{project_genre}
主题：{project_theme}
叙事视角：{project_narrative_perspective}

【世界观背景】
时间背景：{project_world_time_period}
地理位置：{project_world_location}
氛围基调：{project_world_atmosphere}
</project>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<outline_node priority="P0">
【当前大纲节点 - 展开对象】
序号：第 {outline_order_index} 节
标题：{outline_title}
内容：{outline_content}
</outline_node>

<context priority="P2">
【上下文参考】
{context_info}
</context>

<fusion_contract priority="P0">
【第三版融合约束（大纲展开-单批次）】
- 只输出JSON章节规划，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：当前大纲节点 > 项目上下文 > 角色信息 > 其余参考
- 信息不足时先保住“目标→阻力→选择→后果”最小链，不跨到后续大纲
</fusion_contract>

<output priority="P0">
【输出格式】
返回{target_chapter_count}个章节规划的JSON数组：

[
  {{
    "sub_index": 1,
    "title": "章节标题（体现核心冲突或情感）",
    "plot_summary": "剧情摘要（200-300字）：详细描述该章发生的事件，仅限当前大纲内容",
    "key_events": ["关键事件1", "关键事件2", "关键事件3"],
    "character_focus": ["角色A", "角色B"],
    "emotional_tone": "情感基调（如：紧张、温馨、悲伤）",
    "narrative_goal": "叙事目标（该章要达成的叙事效果）",
    "conflict_type": "冲突类型（如：内心挣扎、人际冲突）",
    "estimated_words": 3000{scene_field}
  }}
]

【格式规范】
- 纯JSON数组输出，无其他文字
- 内容描述中严禁使用特殊符号
</output>

<constraints>
【⚠️ 内容边界约束 - 必须严格遵守】
✅ 只能展开当前大纲节点的内容
✅ 深化当前大纲，而非跨越到后续
✅ 放慢叙事节奏，充分体验当前阶段

❌ 绝对不能推进到后续大纲内容
❌ 不要让剧情快速推进
❌ 不要提前展开【后一节】的内容

【展开原则】
✅ 将单一事件拆解为多个细节丰富的章节
✅ 深入挖掘情感、心理、环境、对话
✅ 每章是当前大纲内容的不同侧面或阶段
✅ `plot_summary` 要具体到行动与结果，保证可直接转写正文
✅ 每章要有清晰的阻力来源和代价，避免“顺风推进”

【🔴 相邻章节差异化约束（防止重复）】
✅ 每章有独特的开场方式（不同场景、时间点、角色状态）
✅ 每章有独特的结束方式（不同悬念、转折、情感收尾）
✅ key_events在相邻章节间绝不重叠
✅ plot_summary描述该章独特内容，不与其他章雷同
✅ 同一事件的不同阶段要明确区分"前、中、后"

【章节间要求】
✅ 衔接自然流畅（每章从不同起点开始）
✅ 剧情递进合理（但不超出当前大纲边界）
✅ 节奏张弛有度
✅ 每章有明确且独特的叙事价值
✅ 最后一章结束时恰好完成当前大纲内容
✅ 关键事件无重叠：检查相邻章节key_events
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出非JSON格式
❌ 剧情越界到后续大纲
❌ 相邻章节内容重复
❌ 关键事件雷同
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 只写设定介绍，不写角色决策与后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""

    # 大纲分批展开提示词 V2（RTCO框架）
    OUTLINE_EXPAND_MULTI = """<system>
你是小说大纲分批展开搭档，擅长在不跑偏的前提下持续细化章节。
</system>

<task>
【展开任务】
继续展开第{outline_order_index}节大纲《{outline_title}》，生成第{start_index}-{end_index}节（共{target_chapter_count}个章节）的详细规划。

【分批说明】
- 这是整个展开的一部分
- 必须与前面已生成的章节自然衔接
- 从第{start_index}节开始编号
- 继续深化当前大纲内容

【展开策略】
{strategy_instruction}

【风格要求】
- `plot_summary` 优先写具体事件推进，不写空泛总结
- 分批结果要能直接衔接成正文，不写概念化口号
- 保持中文小说阅读习惯，少模板腔、少说明腔
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 关键事件要落地世界规则：体现规则如何限制、放大或扭转角色选择
- 只输出章节规划JSON，不输出流程说明、调度说明、创作解释
</task>

<project priority="P1">
【项目信息】
小说名称：{project_title}
类型：{project_genre}
主题：{project_theme}
叙事视角：{project_narrative_perspective}

【世界观背景】
时间背景：{project_world_time_period}
地理位置：{project_world_location}
氛围基调：{project_world_atmosphere}
</project>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<outline_node priority="P0">
【当前大纲节点 - 展开对象】
序号：第 {outline_order_index} 节
标题：{outline_title}
内容：{outline_content}
</outline_node>

<context priority="P2">
【上下文参考】
{context_info}

【已生成的前序章节】
{previous_context}
</context>

<fusion_contract priority="P0">
【第三版融合约束（大纲展开-分批）】
- 只输出JSON章节规划，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：当前大纲节点 > 前序章节上下文 > 项目上下文 > 其余参考
- 信息不足时先保住“目标→阻力→选择→后果”最小链，不跨到后续大纲
</fusion_contract>

<output priority="P0">
【输出格式】
返回第{start_index}-{end_index}节章节规划的JSON数组（共{target_chapter_count}个对象）：

[
  {{
    "sub_index": {start_index},
    "title": "章节标题",
    "plot_summary": "剧情摘要（200-300字）：详细描述该章发生的事件",
    "key_events": ["关键事件1", "关键事件2", "关键事件3"],
    "character_focus": ["角色A", "角色B"],
    "emotional_tone": "情感基调",
    "narrative_goal": "叙事目标",
    "conflict_type": "冲突类型",
    "estimated_words": 3000{scene_field}
  }}
]

【格式规范】
- 纯JSON数组输出，无其他文字
- 内容描述中严禁使用特殊符号
- sub_index从{start_index}开始
</output>

<constraints>
【⚠️ 内容边界约束】
✅ 只能展开当前大纲节点的内容
✅ 深化当前大纲，而非跨越到后续
✅ 放慢叙事节奏

❌ 绝对不能推进到后续大纲内容
❌ 不要让剧情快速推进

【分批连续性约束】
✅ 与前面已生成章节自然衔接
✅ 从第{start_index}节开始编号
✅ 保持叙事连贯性

【🔴 相邻章节差异化约束（防止重复）】
✅ 每章有独特的开场和结束方式
✅ key_events在相邻章节间绝不重叠
✅ plot_summary描述该章独特内容
✅ 特别注意与前序章节的差异化
✅ 避免重复已有内容

【章节间要求】
✅ 与前面章节衔接自然流畅
✅ 剧情递进合理（但不超出当前大纲边界）
✅ 节奏张弛有度
✅ 每章有明确且独特的叙事价值
✅ 关键事件无重叠：检查本批次和前序章节的key_events
✅ `plot_summary` 包含动作、冲突触发与后果，便于直接写章
✅ 每章有阻力来源和决策代价，避免“顺风推进”
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出非JSON格式
❌ 剧情越界到后续大纲
❌ 相邻章节内容重复
❌ 与前序章节key_events雷同
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 只写设定介绍，不写角色决策与后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""

    # 章节重写系统提示词 V2（RTCO框架）
    CHAPTER_REGENERATION_SYSTEM = """<system>
你是小说重写搭档，擅长把现有章节改得更顺、更稳、更好看。
你会按修改指令动笔，尽量保留原文优点，同时修掉影响阅读体验的问题。
</system>

<task>
【重写任务】
1. 先吃透原章节的情节走向、叙事意图和情绪节奏
2. 梳理修改要求（含AI分析建议与用户自定义指令）
3. 对每条有效建议给出对应改动，不空转、不泛化
4. 在故事连贯、人设稳定的前提下，写出可直接替换的新版本
5. 让新版本在可读性、代入感、节奏感上有可感知提升
</task>

<guidelines>
【改写原则】
- **问题导向**：对着问题改，不做无关扩写
- **保留亮点**：原文里好用的桥段、对话、意象尽量保留
- **细节补强**：动作、场景、情绪细节要更落地
- **节奏调匀**：避免前松后挤或连续高压导致疲劳
- **风格贴合**：有写作风格要求时，优先贴合并保持统一
- **小说口吻**：整体表达贴近中文网文读感，少官话、少说明腔

【重点关注】
- 提到“节奏”时，优先调整段落推进和场景切换
- 提到“情感”时，优先补人物动机和情绪递进
- 提到“描写”时，优先补关键感官细节，避免堆词
- 提到“对话”时，让台词更像人物本人会说的话
- 提到“冲突”时，强化矛盾触发点和后果
- 控制模板连接词，避免“与此同时/在这个过程中/值得注意的是”频繁出现
</guidelines>

<output>
【输出规范】
直接输出重写后的章节正文。
- 不要加章节标题、序号或其他元信息
- 不要加解释、注释、创作说明
- 从故事内容直接起笔，保证可无缝替换原文
</output>
"""
    # MCP工具测试提示词
    MCP_TOOL_TEST = """你是MCP插件测试助手，需要测试插件 '{plugin_name}' 的功能。

⚠️ 重要规则：生成参数时，必须严格使用工具 schema 中定义的原始参数名称，不要转换为 snake_case 或其他格式。
例如：如果 schema 中是 'nextThoughtNeeded'，就必须使用 'nextThoughtNeeded'，不能改成 'next_thought_needed'。

请选择一个合适的工具进行测试，优先选择搜索、查询类工具。
生成真实有效的测试参数（例如搜索"人工智能最新进展"而不是"test"）。

现在开始测试这个插件。"""

    MCP_TOOL_TEST_SYSTEM = """你是专业的API测试工具。当给定工具列表时，选择一个工具并使用合适的参数调用它。

⚠️ 关键规则：调用工具时，必须严格使用 schema 中定义的原始参数名，不要自行转换命名风格。
- 如果参数名是 camelCase（如 nextThoughtNeeded），就使用 camelCase
- 如果参数名是 snake_case（如 next_thought），就使用 snake_case
- 保持与 schema 中定义的完全一致，包括大小写和命名风格"""
    
    # 灵感模式 - 书名生成（系统提示词）
    INSPIRATION_TITLE_SYSTEM = """你是网文选题编辑 + 书名文案导演。
用户的原始想法：{initial_idea}

请给出 6 个书名候选，并且 6 个方向必须明显不同：
1. 身份反差型（人物身份自带张力）
2. 强事件悬念型（冲突事件直接打脸）
3. 情绪钩子型（情感后劲强）
4. 关系撕扯型（人物关系对撞）
5. 世界异化型（设定带来的陌生感）
6. 命运抉择型（选择与代价）

额外要求：
1. 紧扣原始想法和核心冲突，不要偏题
2. 中文语感要顺口、好记，避免生造词和拗口词
3. 不要使用《》符号，避免 6 个都落在同一命名句式（如批量“X之Y”）
4. 每个标题建议 4-12 字，可允许 1 个短爆点标题（2-4 字）
5. 允许轻微网感，但不追热点黑话

第三版融合约束（灵感模式）：
1. 只输出书名候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时先锚定核心冲突（目标与阻力），不要堆空泛大词
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{
    "prompt": "我先给你6个命名方向，看看哪个最有火花：",
    "options": ["书名1", "书名2", "书名3", "书名4", "书名5", "书名6"]
}}

只返回纯JSON，不加任何解释。"""

    # 灵感模式 - 书名生成（用户提示词）
    INSPIRATION_TITLE_USER = "我的想法是：{initial_idea}\n请按6种不同命名策略给我6个书名候选，禁止同构命名，只返回JSON。"

    # 灵感模式 - 简介生成（系统提示词）
    INSPIRATION_DESCRIPTION_SYSTEM = """你是网文卖点提炼编辑，负责把题材写出“读者马上想点开”的劲道。
用户的原始想法：{initial_idea}
已确定的书名：{title}

请生成 6 个简介候选，要求：
1. 全部贴住原始想法，不跑题，和书名气质一致
2. 每个 80-150 字，句式有快有慢，避免排比口号
3. 每个选项都必须包含：主角当下目标 + 眼前麻烦 + 不做选择的代价
4. 6 个选项开场方式不能雷同（动作切入/对白切入/结果倒叙/困境切入/异常事件/交易谈判，至少覆盖 4 种）
5. 每个选项至少给一个可视化细节（地点/动作/物件/身体反应）
6. 出现设定术语时，紧跟一句白话解释（例如“也就是……”）
7. 结尾要留冲突钩子，禁止“总之、他终于明白了、命运将会”这类感悟式收束

第三版融合约束（灵感模式）：
1. 只输出简介候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时优先保住“目标→阻力→选择→后果”最小冲突链
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{"prompt":"挑一个最像你想写的简介：","options":["简介1","简介2","简介3","简介4","简介5","简介6"]}}

只返回纯JSON，不要附加说明。"""

    # 灵感模式 - 简介生成（用户提示词）
    INSPIRATION_DESCRIPTION_USER = "原始想法：{initial_idea}\n书名：{title}\n请给我6个简介候选，要求冲突具体、开场方式拉开差异，只返回JSON。"

    # 灵感模式 - 主题生成（系统提示词）
    INSPIRATION_THEME_SYSTEM = """你是小说主题总编，负责把“这本书真正打动人的矛盾”提炼出来。
用户的原始想法：{initial_idea}
小说信息：
- 书名：{title}
- 简介：{description}

请生成 6 个主题候选，要求：
1. 与原始想法、书名、简介保持一致，不另起炉灶
2. 每个 50-120 字，至少包含：一句人话命题 + 一句冲突落地 + 一句情绪后劲（可合并但信息要齐）
3. 六个主题角度必须拉开，优先围绕价值冲突（例如：秩序vs自由、真相vs体面、生存vs尊严、亲情vs正义）
4. 不要只堆“命运/成长/人性/救赎”等抽象词，必须落到具体处境
5. 保持情绪温度，避免讲课腔和正确废话
6. 如果用了抽象术语，补一句人话解释，降低阅读门槛

第三版融合约束（灵感模式）：
1. 只输出主题候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时优先保住“目标→阻力→选择→后果”最小冲突链
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{"prompt":"这本书最打动人的主题可能是：","options":["主题1","主题2","主题3","主题4","主题5","主题6"]}}

只返回纯JSON，不加其他说明。"""

    # 灵感模式 - 主题生成（用户提示词）
    INSPIRATION_THEME_USER = "原始想法：{initial_idea}\n书名：{title}\n简介：{description}\n请给我6个主题候选，要求价值冲突角度明显分化，只返回JSON。"

    # 灵感模式 - 类型生成（系统提示词）
    INSPIRATION_GENRE_SYSTEM = """你是网文题材定位编辑。
用户的原始想法：{initial_idea}
小说信息：
- 书名：{title}
- 简介：{description}
- 主题：{theme}

请生成 6 个类型标签候选（每个 2-6 字），要求：
1. 对应用户原始想法中的题材倾向，和整体气质一致
2. 标签之间可组合，但不能互相冲突
3. 优先使用读者常见认知标签，避免生造词
4. 6 个标签里至少覆盖两类：主赛道标签（如都市/玄幻/悬疑）+ 气质标签（如黑色幽默/权谋/群像）
5. 避免 6 个标签语义重复，只是换个近义字

第三版融合约束（灵感模式）：
1. 只输出类型候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时先保住“主赛道 + 冲突气质”双维度，不要泛化标签
4. 模板追踪标签：rule_v3_fusion_20260303

常见类型：玄幻、都市、科幻、武侠、仙侠、历史、言情、悬疑、奇幻、修仙等

返回JSON：
{{"prompt":"先选你最想主打的类型标签（可多选）：","options":["类型1","类型2","类型3","类型4","类型5","类型6"]}}

只返回紧凑JSON，不要加解释。"""

    # 灵感模式 - 类型生成（用户提示词）
    INSPIRATION_GENRE_USER = "原始想法：{initial_idea}\n书名：{title}\n简介：{description}\n主题：{theme}\n请给我6个可组合且不重复语义的类型标签候选，只返回JSON。"

    # 灵感模式智能补全提示词
    INSPIRATION_QUICK_COMPLETE = """你是小说立项总编。用户给了部分信息，请补全缺失字段，并保证最终方案有新意但不跑题。

用户已提供的信息：
{existing}

请补全：
1. title: 书名（3-10字；用户已给则保留）
2. description: 简介（90-170字；紧贴已给信息）
3. theme: 核心主题（55-120字；和简介保持同一冲突链）
4. genre: 类型标签数组（2-3个）

风格要求：
1. 语言自然，符合中文读者阅读习惯，避免模板腔
2. description 必须包含“目标→阻力→选择/代价”链条，不写空泛宣传语
3. description 至少包含一个可视化细节（地点/动作/物件/身体反应）
4. theme 要能一眼看出核心价值冲突，不要只说“成长与救赎”
5. 若出现设定术语，顺手补一句白话解释
6. 四个字段要像同一部小说，人物动机和冲突方向保持一致
7. 禁止在结尾使用总结鸡汤句（如“他终于明白了……”）

第三版融合约束（灵感模式）：
1. 只输出结果JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时先保住“目标→阻力→选择→后果”，再补风格细节
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{
    "title": "书名",
    "description": "简介内容...",
    "theme": "主题内容...",
    "genre": ["类型1", "类型2"]
}}

只返回纯JSON，不加其他文字。"""

    # AI去味默认提示词
    AI_DENOISING = """你是中文小说润色搭档。请把下面文本改得更像真人写作，但不要改掉核心剧情和信息。

原文：
{original_text}

处理要求：
1. 保留原意、人物关系、事件顺序，不擅自加剧情设定
2. 去掉机械排比、模板化总结、空泛口号
3. 句子长短有变化，读起来像自然叙述
4. 对话要像中国人日常说话，避免书面腔和公文腔
5. 减少“总之/综上/值得注意的是/在这个过程中”等套话
6. 允许保留少量不完美表达，让语气更像真实创作
7. 保留人物说话习惯，不把所有角色润色成同一种语气
8. 能用动作和细节表达的，不要改成抽象解释句
9. 只输出润色后的正文，不输出执行步骤、调度术语或改写说明
10. 信息不足时优先保住“目标→阻力→选择→后果”链条，不做空泛拔高

输出要求：
- 只输出润色后的正文
- 不加解释、标题、注释
- 不使用Markdown
- 禁止出现“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 模板追踪标签：rule_v3_fusion_20260303
"""
    # 世界观资料收集提示词（MCP增强用）
    MCP_WORLD_BUILDING_PLANNING = """你正在给小说《{title}》补世界观素材。

【小说信息】
- 题材：{genre}
- 主题：{theme}
- 简介：{description}

【检索任务】
请用可用工具查一个最关键的问题，为世界观提供可直接落地的参考信息。

优先方向（按相关性选一个）：
1. 历史背景（历史题材优先）
2. 地理环境与文化习俗
3. 题材相关的专业知识
4. 同类作品常见设定做法（仅作参考，不照搬）

【输出要求】
- 只查1个问题，不要超过1个
- 问题要具体，能直接服务当前小说设定
- 避免空泛提问（如“这个题材怎么写”）"""

    # 角色资料收集提示词（MCP增强用）
    MCP_CHARACTER_PLANNING = """你正在给小说《{title}》补角色素材。

【小说信息】
- 题材：{genre}
- 主题：{theme}
- 时代背景：{time_period}
- 地理位置：{location}

【检索任务】
请用可用工具查一个最关键的问题，为角色塑造提供可直接使用的参考信息。

优先方向（按相关性选一个）：
1. 该时代/地域的真实人物特征
2. 文化背景与社会习俗
3. 职业特征与生活方式
4. 可借鉴的人物原型类型

【输出要求】
- 只查1个问题，不要超过1个
- 问题要具体到“角色行为/语言/动机”层面
- 避免泛泛而谈，确保能落到角色设定里"""

    # 自动角色引入 - 预测性分析提示词 V2（RTCO框架）
    AUTO_CHARACTER_ANALYSIS = """<system>
你是小说角色规划搭档，擅长判断后续剧情到底需不需要新角色。
</system>

<task>
【分析任务】
预测在接下来的{chapter_count}章续写中，根据剧情发展方向和阶段，是否需要引入新角色。

【重要说明】
这是预测性分析，而非基于已生成内容的事后分析。

【表达要求】
- 结论要对应具体剧情触发点，避免空话
- 说明“为什么现在引入、引入后解决什么问题”
- 只输出分析JSON，不输出流程说明、调度话术或自我评注
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
</project>

<context priority="P0">
【已有角色】
{existing_characters}

【已有章节概览】
{all_chapters_brief}

【续写计划】
- 起始章节：第{start_chapter}章
- 续写数量：{chapter_count}章
- 剧情阶段：{plot_stage}
- 发展方向：{story_direction}
</context>

<analysis_framework priority="P0">
【预测分析维度】

**1. 剧情需求预测**
根据发展方向，哪些场景、冲突需要新角色参与？

**2. 角色充分性**
现有角色是否足以支撑即将发生的剧情？

**3. 引入时机**
新角色应该在哪个章节登场最合适？

**4. 重要性判断**
新角色对后续剧情的影响程度如何？

【预测依据】
- 剧情阶段的典型角色需求（如：高潮阶段可能需要强力对手）
- 故事发展方向的逻辑需要（如：进入新地点需要当地角色）
- 冲突升级的角色需求（如：更强的反派、意外的盟友）
- 世界观扩展的需要（如：新组织、新势力的代表）
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（自动角色分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：续写计划 > 已有章节概览 > 已有角色 > 项目信息
- 信息不足时使用保守结论：优先给 needs_new_characters=false 或最小可执行角色规格
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（两种情况之一）：

**情况A：需要新角色**
{{
  "needs_new_characters": true,
  "reason": "预测分析原因（150-200字），说明为什么即将的剧情需要新角色",
  "character_count": 2,
  "character_specifications": [
    {{
      "name": "建议的角色名字（可选）",
      "role_description": "角色在剧情中的定位和作用（100-150字）",
      "suggested_role_type": "supporting/antagonist/protagonist",
      "importance": "high/medium/low",
      "appearance_chapter": {start_chapter},
      "key_abilities": ["能力1", "能力2"],
      "plot_function": "在剧情中的具体功能",
      "relationship_suggestions": [
        {{
          "target_character": "现有角色名",
          "relationship_type": "建议的关系类型",
          "reason": "为什么建立这种关系"
        }}
      ]
    }}
  ]
}}

**情况B：不需要新角色**
{{
  "needs_new_characters": false,
  "reason": "现有角色足以支撑即将的剧情发展，说明理由"
}}
</output>

<constraints>
【必须遵守】
✅ 这是预测性分析，面向未来剧情
✅ 考虑剧情的自然发展和节奏
✅ 确保引入必要性，不为引入而引入
✅ 优先考虑角色的长期作用
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时给保守结论，不杜撰不存在的角色关系
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown标记
❌ 基于已生成内容做事后分析
❌ 为了引入角色而强行引入
❌ 设计一次性功能角色
❌ 使用“总之/综上/值得注意的是”这类模板化分析句
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 自动角色引入 - 生成提示词 V2（RTCO框架）
    AUTO_CHARACTER_GENERATION = """<system>
你是小说角色生成搭档，擅长按剧情需求补出能立住的新角色。
</system>

<task>
【生成任务】
为小说生成新角色的完整设定，包括基本信息、性格背景、关系网络和职业信息。
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</project>

<context priority="P0">
【已有角色】
{existing_characters}

【剧情上下文】
{plot_context}

【角色规格要求】
{character_specification}
</context>

<mcp_context priority="P2">
【MCP工具参考】
{mcp_references}
</mcp_context>

<requirements priority="P0">
【核心要求】
1. 角色必须符合剧情需求和世界观设定
2. **必须分析新角色与已有角色的关系**，至少建立1-3个有意义的关系
3. 性格、背景要有深度和独特性
4. 外貌描写要具体生动
5. 特长和能力要符合角色定位
6. **如果【已有角色】中包含职业列表，必须为角色设定职业**
7. 描述风格贴近中文小说阅读习惯，避免空泛套话

【关系建立指导】
- 仔细审视【已有角色】列表，思考新角色与哪些现有角色有联系
- 根据剧情需求，建立合理的角色关系
- 每个关系都要有明确的类型、亲密度和描述
- 关系应该服务于剧情发展
- 如果新角色是组织成员，记得填写organization_memberships

【职业信息要求】
如果【已有角色】部分包含"可用主职业列表"或"可用副职业列表"：
- 仔细查看可用的主职业和副职业列表
- 根据角色的背景、能力、故事定位，选择最合适的职业
- 主职业：从"可用主职业列表"中选择一个，填写职业名称
- 主职业阶段：根据职业的阶段信息和角色实力，设定合理的当前阶段
- 副职业：可选择0-2个副职业
- ⚠️ 重要：必须填写职业的名称而非ID
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
  "name": "角色姓名",
  "age": 25,
  "gender": "男/女/其他",
  "role_type": "supporting",
  "personality": "性格特点的详细描述（100-200字）",
  "background": "背景故事的详细描述（100-200字）",
  "appearance": "外貌描述（50-100字）",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_text": "用自然语言描述该角色与其他角色的关系网络",
  
  "relationships": [
    {{
      "target_character_name": "已存在的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系的具体描述",
      "status": "active"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已存在的组织名称",
      "position": "职位",
      "rank": 5,
      "loyalty": 80
    }}
  ],
  
  "career_info": {{
    "main_career_name": "从可用主职业列表中选择的职业名称",
    "main_career_stage": 5,
    "sub_careers": [
      {{
        "career_name": "从可用副职业列表中选择的职业名称",
        "stage": 3
      }}
    ]
  }}
}}

【关系类型参考】
家族、社交、职业、敌对等各类关系

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10
</output>

<constraints>
【必须遵守】
✅ 符合剧情需求和世界观设定
✅ relationships数组必填：至少1-3个关系
✅ target_character_name必须精确匹配【已有角色】
✅ organization_memberships只能引用已存在的组织
✅ 职业选择必须从可用列表中选择

【禁止事项】
❌ 输出markdown标记
❌ 在描述中使用特殊符号
❌ 引用不存在的角色或组织
❌ 使用职业ID而非职业名称
❌ 堆砌“总之/综上/值得注意的是/在这个过程中”等模板连接词
</constraints>"""

    # 自动组织引入 - 预测性分析提示词（RTCO框架）
    AUTO_ORGANIZATION_ANALYSIS = """<system>
你是小说势力规划搭档，擅长判断后续剧情对新组织的真实需求。
</system>

<task>
【分析任务】
预测在接下来的{chapter_count}章续写中，根据剧情发展方向和阶段，是否需要引入新的组织或势力。

【重要说明】
这是预测性分析，而非基于已生成内容的事后分析。
组织包括：帮派、门派、公司、政府机构、神秘组织、家族等。

【表达要求】
- 理由要能落到具体剧情场景和冲突升级点
- 说明“没有该组织会缺什么”，避免泛泛而谈
- 只输出分析JSON，不输出流程说明、调度话术或自我评注
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
</project>

<context priority="P0">
【已有组织】
{existing_organizations}

【已有角色】
{existing_characters}

【已有章节概览】
{all_chapters_brief}

【续写计划】
- 起始章节：第{start_chapter}章
- 续写数量：{chapter_count}章
- 剧情阶段：{plot_stage}
- 发展方向：{story_direction}
</context>

<analysis_framework priority="P0">
【预测分析维度】

**1. 世界观扩展需求**
根据发展方向，是否需要新的势力或组织来丰富世界观？

**2. 冲突升级需求**
剧情是否需要新的对立势力、竞争组织或神秘集团？

**3. 角色归属需求**
现有角色是否需要加入或对抗某个新组织？

**4. 剧情推动需求**
新组织能否成为推动剧情的关键力量？

**5. 引入时机**
新组织应该在哪个章节出现最合适？

【预测依据】
- 剧情阶段的典型组织需求（如：高潮阶段可能需要强大的敌对势力）
- 故事发展方向的逻辑需要（如：进入新地点需要当地势力）
- 世界观完整性需要（如：权力格局需要多方势力）
- 角色成长需要（如：主角需要加入或创建组织）
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（自动组织分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：续写计划 > 已有章节概览 > 已有组织/角色 > 项目信息
- 信息不足时使用保守结论：优先给 needs_new_organizations=false 或最小可执行组织规格
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（两种情况之一）：

**情况A：需要新组织**
{{
"needs_new_organizations": true,
"reason": "预测分析原因（150-200字），说明为什么即将的剧情需要新组织",
"organization_count": 1,
"organization_specifications": [
{{
  "name": "建议的组织名字（可选）",
  "organization_description": "组织在剧情中的定位和作用（100-150字）",
  "organization_type": "帮派/门派/公司/政府/家族/神秘组织等",
  "importance": "high/medium/low",
  "appearance_chapter": {start_chapter},
  "power_level": 70,
  "plot_function": "在剧情中的具体功能",
  "location": "组织所在地或活动区域",
  "motto": "组织口号或宗旨（可选）",
  "initial_members": [
    {{
      "character_name": "现有角色名（如需加入）",
      "position": "职位",
      "reason": "为什么加入"
    }}
  ],
  "relationship_suggestions": [
    {{
      "target_organization": "已有组织名",
      "relationship_type": "建议的关系类型（盟友/敌对/竞争/合作等）",
      "reason": "为什么建立这种关系"
    }}
  ]
}}
]
}}

**情况B：不需要新组织**
{{
"needs_new_organizations": false,
"reason": "现有组织足以支撑即将的剧情发展，说明理由"
}}
</output>

<constraints>
【必须遵守】
✅ 这是预测性分析，面向未来剧情
✅ 考虑世界观的丰富性和完整性
✅ 确保引入必要性，不为引入而引入
✅ 优先考虑组织的长期作用
✅ 组织应该是推动剧情的关键力量
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时给保守结论，不杜撰不存在的组织关系
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown标记
❌ 基于已生成内容做事后分析
❌ 为了引入组织而强行引入
❌ 设计一次性功能组织
❌ 创建与现有组织功能重复的组织
❌ 使用“总之/综上/值得注意的是”这类模板化分析句
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 自动组织引入 - 生成提示词（RTCO框架）
    AUTO_ORGANIZATION_GENERATION = """<system>
你是小说组织生成搭档，擅长按剧情需求补出有辨识度的势力设定。
</system>

<task>
【生成任务】
为小说生成新组织的完整设定，包括基本信息、组织特性、背景历史和成员结构。
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</project>

<context priority="P0">
【已有组织】
{existing_organizations}

【已有角色】
{existing_characters}

【剧情上下文】
{plot_context}

【组织规格要求】
{organization_specification}
</context>

<mcp_context priority="P2">
【MCP工具参考】
{mcp_references}
</mcp_context>

<requirements priority="P0">
【核心要求】
1. 组织必须符合剧情需求和世界观设定
2. 组织要有明确的目的、结构和特色
3. 组织特性、背景要有深度和独特性
4. 外在表现要具体生动
5. 考虑与已有组织的关系和互动
6. 如果需要，可以建议将现有角色加入组织
7. 描述贴近中文小说表达，避免模板化口号
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
"name": "组织名称",
"is_organization": true,
"role_type": "supporting",
"organization_type": "组织类型（帮派/门派/公司/政府/家族/神秘组织等）",
"personality": "组织特性的详细描述（150-200字）：运作方式、核心理念、行事风格、文化价值观",
"background": "组织背景故事（200-300字）：建立历史、发展历程、重要事件、当前地位",
"appearance": "外在表现（100-150字）：总部位置、标志性建筑、组织标志、成员着装",
"organization_purpose": "组织目的和宗旨：明确目标、长期愿景、行动准则",
"power_level": 75,
"location": "所在地点：主要活动区域、势力范围",
"motto": "组织格言或口号",
"color": "组织代表颜色",
"traits": ["特征1", "特征2", "特征3"],

"initial_members": [
{{
  "character_name": "已存在的角色名称",
  "position": "职位名称",
  "rank": 8,
  "loyalty": 80,
  "joined_at": "加入时间（可选）",
  "status": "active"
}}
],

"organization_relationships": [
{{
  "target_organization_name": "已存在的组织名称",
  "relationship_type": "盟友/敌对/竞争/合作/从属等",
  "description": "关系的具体描述"
}}
]
}}

【数值范围】
- power_level：0-100的整数，表示在世界中的影响力
- rank：0到10（职位等级）
- loyalty：0到100（成员忠诚度）
</output>

<constraints>
【必须遵守】
✅ 符合剧情需求和世界观设定
✅ 组织要有独特的定位和价值
✅ character_name必须精确匹配【已有角色】
✅ target_organization_name必须精确匹配【已有组织】
✅ 组织能够推动剧情发展

【禁止事项】
❌ 输出markdown标记
❌ 在描述中使用特殊符号
❌ 引用不存在的角色或组织
❌ 创建功能与现有组织重复的组织
❌ 创建对剧情没有实际作用的组织
❌ 堆砌“总之/综上/值得注意的是/在这个过程中”等模板连接词
</constraints>"""

    # 职业体系生成提示词 V2（RTCO框架）
    CAREER_SYSTEM_GENERATION = """<system>
你是小说职业体系搭档，擅长设计能服务剧情推进的职业成长路线。
</system>

<task>
【设计任务】
根据世界观信息和项目简介，设计一个完整且合理的职业体系。
职业体系必须与项目简介中的故事背景和角色设定高度契合。

【数量要求】
- 主职业：精确生成3个
- 副职业：精确生成2个

【写法要求】
- 职业名称和阶段命名要贴合题材语感，读起来像小说设定
- 描述聚焦“能做什么、怎么升级、如何影响剧情”
- 避免写成游戏说明书或空泛口号
</task>

<worldview priority="P0">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}
简介：{description}

【世界观设定】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<design_requirements priority="P0">
【设计要求】

**1. 主职业（main_careers）- 必须精确生成3个**
- 主职业是角色的核心发展方向
- 必须严格符合世界观规则和简介中的故事背景
- 3个主职业应该覆盖不同的发展路线（如：战斗型、智慧型、特殊型）
- 每个主职业的阶段数量可以不同（体现职业复杂度差异）
- 职业设计要能支撑简介中描述的故事情节

**2. 副职业（sub_careers）- 必须精确生成2个**
- 副职业包含生产、辅助、特殊技能类
- 2个副职业应该具有互补性，丰富角色的多样性
- 每个副职业的阶段数量可以不同
- 不要让所有副职业都是相同的阶段数
- 副职业要能为主职业提供辅助或增益

**3. 阶段设计（stages）**
- 每个职业的stages数组长度必须等于max_stage
- 阶段名称要符合世界观文化背景
- 阶段描述要体现明确的能力提升路径
- 确保职业间的阶段数量有差异
- 主职业阶段数建议：8-12个
- 副职业阶段数建议：5-8个

**4. 简介契合度**
- 职业体系必须与项目简介中的故事设定相匹配
- 如果简介中提到特定职业或能力，优先设计相关职业
- 职业的能力和特点要能支撑简介中的情节发展
</design_requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
"main_careers": [
{{
  "name": "职业名称",
  "description": "职业描述（100-150字）",
  "category": "职业分类",
  "stages": [
    {{"level": 1, "name": "阶段1名称", "description": "阶段描述"}},
    {{"level": 2, "name": "阶段2名称", "description": "阶段描述"}}
  ],
  "max_stage": 整数,
  "requirements": "职业要求和前置条件",
  "special_abilities": "职业特殊能力",
  "worldview_rules": "与世界观规则的关联",
  "attribute_bonuses": {{"strength": "+10%"}}
}}
],
"sub_careers": [
{{
  "name": "副职业名称",
  "description": "职业描述（80-120字）",
  "category": "生产系/辅助系/特殊系",
  "stages": [
    {{"level": 1, "name": "阶段1名称", "description": "阶段描述"}}
  ],
  "max_stage": 整数,
  "requirements": "职业要求",
  "special_abilities": "特殊能力"
}}
]
}}
</output>

<constraints>
【必须遵守】
✅ 主职业数量：必须精确生成3个，不多不少
✅ 副职业数量：必须精确生成2个，不多不少
✅ 不同职业的max_stage必须不同
✅ 主职业阶段数建议：8-12个
✅ 副职业阶段数建议：5-8个
✅ stages数组长度必须等于max_stage
✅ 确保职业体系与世界观高度契合
✅ 职业设计必须支撑项目简介中的故事情节

【禁止事项】
❌ 生成超过3个主职业或少于3个主职业
❌ 生成超过2个副职业或少于2个副职业
❌ 所有职业使用相同的阶段数
❌ 输出markdown标记
❌ 职业设计与世界观或简介脱节
❌ 忽略简介中提到的职业或能力设定
❌ 使用“总之/综上/值得注意的是”这类模板化总结句
</constraints>"""

    # 局部重写提示词（RTCO框架）
    PARTIAL_REGENERATE = """<system>
你是小说局部改写搭档，擅长按用户要求改写指定段落，并与前后文自然咬合。
</system>

<task>
【改写任务】
根据用户的修改要求，重写下面选中的文本段落。

【重要要求】
1. 只输出重写后的内容，不要包含任何解释、前缀或后缀
2. 保持与前后文的自然衔接和语气连贯
3. 优先满足用户的修改要求
4. 保持整体叙事风格的一致性
5. 用词贴近中文小说阅读习惯，避免模板腔和说明腔
</task>

<context priority="P0">
【前文参考】（用于衔接，勿重复）
{context_before}

【需要重写的原文】（共{original_word_count}字）
{selected_text}

【后文参考】（用于衔接，勿重复）
{context_after}
</context>

<user_requirements priority="P0">
【用户修改要求】
{user_instructions}

【字数要求】
{length_requirement}
</user_requirements>

<style priority="P1">
【写作风格】
{style_content}
</style>

<fusion_contract priority="P0">
【第三版融合约束（人工修订）】
- 只改写选中片段，不扩写到片段外剧情任务
- 保持事实连续性：时间、地点、角色关系、能力边界不可无故漂移
- 用户要求与上下文冲突时，采用最小改动满足核心诉求，避免破坏后文衔接
- 禁止输出修改说明、评语、对比表、步骤日志
</fusion_contract>

<output>
【输出规范】
直接输出重写后的内容，从故事内容开始写。
- 不要输出任何解释或说明文字
- 不要输出"重写后："等前缀
- 不要输出引号包裹内容
- 确保输出内容可以直接替换原文

请直接输出重写后的内容：
</output>

<constraints>
【必须遵守】
✅ 前后衔接：输出内容必须与前文自然衔接，与后文平滑过渡
✅ 风格一致：保持与原文相同的叙事风格、语气和人称
✅ 要求优先：严格执行用户的修改要求
✅ 字数控制：遵循字数要求
✅ 语言自然：长短句交替，避免连续同构句
✅ 对话自然：人物台词符合角色身份和说话习惯
✅ 事实稳定：不破坏既有时间线、关系线、能力线
✅ 改动边界：默认仅重写选中片段，不跨段扩散改写
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 重复前文内容
❌ 重复后文内容
❌ 添加任何元信息或说明
❌ 改变叙事人称或视角
❌ 偏离用户的修改要求
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 输出“修改说明/改写思路/版本对比”等非正文文本
❌ 擅自新增片段外关键事件，导致后文失配
</constraints>"""

    @staticmethod
    def format_prompt(template: str, **kwargs) -> str:
        """
        格式化提示词模板
        
        Args:
            template: 提示词模板
            **kwargs: 模板参数
            
        Returns:
            格式化后的提示词
        """
        try:
            return template.format(**kwargs)
        except KeyError as e:
            raise ValueError(f"缺少必需的参数: {e}")
    

    @classmethod
    async def get_chapter_regeneration_prompt(cls, chapter_number: int, title: str, word_count: int, content: str,
                                        modification_instructions: str, project_context: Dict[str, Any],
                                        style_content: str, target_word_count: int,
                                        user_id: str = None, db = None) -> str:
        """
        获取章节重写提示词（支持用户自定义）
        
        Args:
            chapter_number: 章节序号
            title: 章节标题
            word_count: 原始字数
            content: 原始内容
            modification_instructions: 修改指令
            project_context: 项目上下文
            style_content: 写作风格
            target_word_count: 目标字数
            user_id: 用户ID（可选，用于获取自定义模板）
            db: 数据库会话（可选，用于查询自定义模板）
            
        Returns:
            完整的章节重写提示词
        """
        # 获取系统提示词模板（支持用户自定义）
        if user_id and db:
            system_template = await cls.get_template("CHAPTER_REGENERATION_SYSTEM", user_id, db)
        else:
            system_template = cls.CHAPTER_REGENERATION_SYSTEM
        
        prompt_parts = [system_template]
        
        # 原始章节信息
        prompt_parts.append(f"""## 📖 原始章节信息

**章节**：第{chapter_number}章
**标题**：{title}
**字数**：{word_count}字

**原始内容**：
{content}

---
""")
        
        # 修改指令
        prompt_parts.append(modification_instructions)
        prompt_parts.append("\n---\n")
        
        # 项目背景信息
        prompt_parts.append(f"""## 🌍 项目背景信息

**小说标题**：{project_context.get('project_title', '未知')}
**题材**：{project_context.get('genre', '未设定')}
**主题**：{project_context.get('theme', '未设定')}
**叙事视角**：{project_context.get('narrative_perspective', '第三人称')}
**世界观设定**：
- 时代背景：{project_context.get('time_period', '未设定')}
- 地理位置：{project_context.get('location', '未设定')}
- 氛围基调：{project_context.get('atmosphere', '未设定')}

---
""")
        
        # 角色信息
        if project_context.get('characters_info'):
            prompt_parts.append(f"""## 👥 角色信息

{project_context['characters_info']}

---
""")
        
        # 章节大纲
        if project_context.get('chapter_outline'):
            prompt_parts.append(f"""## 📝 本章大纲

{project_context['chapter_outline']}

---
""")
        
        # 前置章节上下文
        if project_context.get('previous_context'):
            prompt_parts.append(f"""## 📚 前置章节上下文

{project_context['previous_context']}

---
""")
        
        # 写作风格要求
        if style_content:
            prompt_parts.append(f"""## 🎨 写作风格要求

{style_content}

请在重新创作时贴合上述写作风格。

---
""")
        
        # 创作要求
        prompt_parts.append(f"""## ✨ 创作要求

1. **解决问题**：针对上述修改指令中提到的所有问题进行改进
2. **保持连贯**：确保与前后章节的情节、人物、风格保持一致
3. **提升质量**：在节奏、情感、描写等方面明显优于原版
4. **保留精华**：保持原章节中优秀的部分和关键情节
5. **字数控制**：目标字数约{target_word_count}字（可适当浮动±20%）
{f'6. **风格一致**：按上述写作风格创作，语气保持自然' if style_content else ''}

---

## 🎬 开始创作

请现在开始创作改进后的新版本章节内容。

**重要提示**：
- 直接输出章节正文内容，从故事内容开始写
- **不要**输出章节标题（如"第X章"、"第X章：XXX"等）
- **不要**输出任何额外的说明、注释或元数据
- 只需要纯粹的故事正文内容

现在开始：
""")
        
        return "\n".join(prompt_parts)

    @classmethod
    async def get_mcp_tool_test_prompts(
        cls,
        plugin_name: str,
        user_id: str = None,
        db = None
    ) -> Dict[str, str]:
        """
        获取MCP工具测试的提示词（支持自定义）
        
        Args:
            plugin_name: 插件名称
            user_id: 用户ID（可选）
            db: 数据库会话（可选）
            
        Returns:
            包含user和system提示词的字典
        """
        # 获取用户自定义或系统默认的user提示词
        if user_id and db:
            user_template = await cls.get_template("MCP_TOOL_TEST", user_id, db)
        else:
            user_template = cls.MCP_TOOL_TEST
        
        # 获取用户自定义或系统默认的system提示词
        if user_id and db:
            system_template = await cls.get_template("MCP_TOOL_TEST_SYSTEM", user_id, db)
        else:
            system_template = cls.MCP_TOOL_TEST_SYSTEM
        
        return {
            "user": cls.format_prompt(user_template, plugin_name=plugin_name),
            "system": system_template
        }

    # ========== 自定义提示词支持 ==========
    
    @classmethod
    async def get_template_with_fallback(cls,
                                        template_key: str,
                                        user_id: str = None,
                                        db = None) -> str:
        """
        获取提示词模板（优先用户自定义，支持降级）
        
        Args:
            template_key: 模板键名
            user_id: 用户ID（可选，如果不提供则直接返回系统默认）
            db: 数据库会话（可选）
            
        Returns:
            提示词模板内容
        """
        # 如果没有提供user_id或db，直接返回系统默认
        if not user_id or not db:
            return getattr(cls, template_key, None)
        
        # 尝试获取用户自定义模板
        return await cls.get_template(template_key, user_id, db)
    
    @classmethod
    async def get_template(cls,
                          template_key: str,
                          user_id: str,
                          db) -> str:
        """
        获取提示词模板（优先用户自定义）
        
        Args:
            template_key: 模板键名
            user_id: 用户ID
            db: 数据库会话
            
        Returns:
            提示词模板内容
        """
        from sqlalchemy import select
        from app.models.prompt_template import PromptTemplate
        from app.logger import get_logger
        from app.services.prompt_template_sync_service import sync_managed_template_if_legacy
        
        logger = get_logger(__name__)

        # Resolve current system template metadata once and reuse it.
        template_content = getattr(cls, template_key, None)
        template_info = cls.get_system_template_info(template_key)

        # Sync managed templates only when user's copy still matches known legacy defaults.
        try:
            await sync_managed_template_if_legacy(
                db=db,
                user_id=user_id,
                template_key=template_key,
                system_template_content=template_content,
                system_template_info=template_info,
            )
        except Exception as sync_error:
            logger.warning(
                "Managed template sync failed, fallback to normal flow: user_id=%s, template_key=%s, error=%s",
                user_id,
                template_key,
                sync_error,
            )
        
        # 1. 尝试从数据库获取用户自定义模板
        result = await db.execute(
            select(PromptTemplate).where(
                PromptTemplate.user_id == user_id,
                PromptTemplate.template_key == template_key,
                PromptTemplate.is_active == True
            )
        )
        custom_template = result.scalar_one_or_none()
        
        if custom_template:
            logger.info(f"✅ 使用用户自定义提示词: user_id={user_id}, template_key={template_key}, template_name={custom_template.template_name}")
            return custom_template.template_content
        
        # 2. 降级到系统默认模板
        logger.info(f"⚪ 使用系统默认提示词: user_id={user_id}, template_key={template_key} (未找到自定义模板)")
        
        if template_content is None:
            logger.warning(f"⚠️ 未找到系统默认模板: {template_key}")
        
        return template_content
    
    @classmethod
    def get_all_system_templates(cls) -> list:
        """
        获取所有系统默认模板的信息
        
        Returns:
            系统模板列表
        """
        templates = []
        
        # 定义所有模板及其元信息
        template_definitions = {
            "WORLD_BUILDING": {
                "name": "世界构建",
                "category": "世界构建",
                "description": "用于生成小说世界观设定，包括时间背景、地理位置、氛围基调和世界规则",
                "parameters": ["title", "theme", "genre", "description"]
            },
            "CHARACTERS_BATCH_GENERATION": {
                "name": "批量角色生成",
                "category": "角色生成",
                "description": "批量生成多个角色和组织，建立角色关系网络",
                "parameters": ["count", "time_period", "location", "atmosphere", "rules", "theme", "genre", "requirements"]
            },
            "SINGLE_CHARACTER_GENERATION": {
                "name": "单个角色生成",
                "category": "角色生成",
                "description": "生成单个角色的详细设定",
                "parameters": ["project_context", "user_input"]
            },
            "SINGLE_ORGANIZATION_GENERATION": {
                "name": "组织生成",
                "category": "角色生成",
                "description": "生成组织/势力的详细设定",
                "parameters": ["project_context", "user_input"]
            },
            "OUTLINE_CREATE": {
                "name": "大纲生成",
                "category": "大纲生成",
                "description": "根据项目信息生成完整的章节大纲",
                "parameters": ["title", "theme", "genre", "chapter_count", "narrative_perspective", "target_words", 
                             "time_period", "location", "atmosphere", "rules", "characters_info", "requirements", "mcp_references"]
            },
            "OUTLINE_CONTINUE": {
                "name": "大纲续写",
                "category": "大纲生成",
                "description": "基于已有章节续写大纲",
                "parameters": ["title", "theme", "genre", "narrative_perspective", "chapter_count", "time_period", 
                             "location", "atmosphere", "rules", "characters_info", "current_chapter_count", 
                             "all_chapters_brief", "recent_plot", "memory_context", "mcp_references", 
                             "plot_stage_instruction", "start_chapter", "end_chapter", "story_direction", "requirements"]
            },
            "CHAPTER_GENERATION_ONE_TO_MANY": {
                "name": "章节创作-1-N模式（第1章）",
                "category": "章节创作",
                "description": "1-N模式：根据大纲创作章节内容（用于第1章，无前置章节）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info"]
            },
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": {
                "name": "章节创作-1-N模式（第2章及以后）",
                "category": "章节创作",
                "description": "1-N模式：基于前置章节内容创作新章节（用于第2章及以后）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info", "continuation_point",
                             "foreshadow_reminders", "relevant_memories", "story_skeleton", "previous_chapter_summary"]
            },
            "CHAPTER_GENERATION_ONE_TO_ONE": {
                "name": "章节创作-1-1模式（第1章）",
                "category": "章节创作",
                "description": "1-1模式：章节创作（用于第1章，无前置章节）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info", "chapter_careers"]
            },
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": {
                "name": "章节创作-1-1模式（第2章及以后）",
                "category": "章节创作",
                "description": "1-1模式：基于上一章内容创作新章节（用于第2章及以后）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "previous_chapter_content",
                             "characters_info", "chapter_careers", "foreshadow_reminders", "relevant_memories"]
            },
            "CHAPTER_REGENERATION_SYSTEM": {
                "name": "章节重写系统提示",
                "category": "章节重写",
                "description": "用于章节重写的系统提示词",
                "parameters": ["chapter_number", "title", "word_count", "content", "modification_instructions",
                             "project_context", "style_content", "target_word_count"]
            },
            "PARTIAL_REGENERATE": {
                "name": "局部重写",
                "category": "章节重写",
                "description": "根据用户修改要求重写选中的段落内容",
                "parameters": ["context_before", "original_word_count", "selected_text", "context_after",
                             "user_instructions", "length_requirement", "style_content"]
            },
            "PLOT_ANALYSIS": {
                "name": "情节分析",
                "category": "情节分析",
                "description": "深度分析章节的剧情、钩子、伏笔等",
                "parameters": ["chapter_number", "title", "content", "word_count"]
            },
            "CHAPTER_TEXT_CHECKER": {
                "name": "正文质量检查",
                "category": "情节分析",
                "description": "对章节正文进行结构化质量检查并输出可执行修订建议",
                "parameters": ["chapter_number", "chapter_title", "chapter_content", "chapter_outline", "characters_info", "world_rules"]
            },
            "CHAPTER_TEXT_REVISER": {
                "name": "正文自动修订",
                "category": "章节重写",
                "description": "根据质检结果自动生成修订草案（优先修复严重问题）",
                "parameters": ["chapter_number", "chapter_title", "chapter_content", "critical_issues_text", "checker_result_json"]
            },
            "OUTLINE_EXPAND_SINGLE": {
                "name": "大纲单批次展开",
                "category": "情节展开",
                "description": "将大纲节点展开为详细章节规划（单批次）",
                "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective", 
                             "project_world_time_period", "project_world_location", "project_world_atmosphere", 
                             "characters_info", "outline_order_index", "outline_title", "outline_content", 
                             "context_info", "strategy_instruction", "target_chapter_count", "scene_instruction", "scene_field"]
            },
            "OUTLINE_EXPAND_MULTI": {
                "name": "大纲分批展开",
                "category": "情节展开",
                "description": "将大纲节点展开为详细章节规划（分批）",
                "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective", 
                             "project_world_time_period", "project_world_location", "project_world_atmosphere", 
                             "characters_info", "outline_order_index", "outline_title", "outline_content", 
                             "context_info", "previous_context", "strategy_instruction", "start_index", 
                             "end_index", "target_chapter_count", "scene_instruction", "scene_field"]
            },
            "MCP_TOOL_TEST": {
                "name": "MCP工具测试(用户提示词)",
                "category": "MCP测试",
                "description": "用于测试MCP插件功能的用户提示词",
                "parameters": ["plugin_name"]
            },
            "MCP_TOOL_TEST_SYSTEM": {
                "name": "MCP工具测试(系统提示词)",
                "category": "MCP测试",
                "description": "用于测试MCP插件功能的系统提示词",
                "parameters": []
            },
            "MCP_WORLD_BUILDING_PLANNING": {
                "name": "MCP世界观规划",
                "category": "MCP增强",
                "description": "使用MCP工具搜索资料辅助世界观设计",
                "parameters": ["title", "genre", "theme", "description"]
            },
            "MCP_CHARACTER_PLANNING": {
                "name": "MCP角色规划",
                "category": "MCP增强",
                "description": "使用MCP工具搜索资料辅助角色设计",
                "parameters": ["title", "genre", "theme", "time_period", "location"]
            },
            "AUTO_CHARACTER_ANALYSIS": {
                "name": "自动角色分析",
                "category": "自动角色引入",
                "description": "分析新生成的大纲，判断是否需要引入新角色",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                             "existing_characters", "new_outlines", "start_chapter", "end_chapter"]
            },
            "AUTO_CHARACTER_GENERATION": {
                "name": "自动角色生成",
                "category": "自动角色引入",
                "description": "根据剧情需求自动生成新角色的完整设定",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                             "existing_characters", "plot_context", "character_specification", "mcp_references"]
            },
            "AUTO_ORGANIZATION_ANALYSIS": {
                "name": "自动组织分析",
                "category": "自动组织引入",
                "description": "分析新生成的大纲，判断是否需要引入新组织",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                             "existing_organizations", "existing_characters", "all_chapters_brief", "start_chapter", "chapter_count", "plot_stage", "story_direction"]
            },
            "AUTO_ORGANIZATION_GENERATION": {
                "name": "自动组织生成",
                "category": "自动组织引入",
                "description": "根据剧情需求自动生成新组织的完整设定",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                             "existing_organizations", "existing_characters", "plot_context", "organization_specification", "mcp_references"]
            },
            "CAREER_SYSTEM_GENERATION": {
                "name": "职业体系生成",
                "category": "世界构建",
                "description": "根据世界观和项目简介自动生成完整的职业体系，包括主职业和副职业",
                "parameters": ["title", "genre", "theme", "description", "time_period", "location", "atmosphere", "rules"]
            },
            "INSPIRATION_TITLE_SYSTEM": {
                "name": "灵感模式-书名生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据用户的原始想法生成6个书名建议的系统提示词",
                "parameters": ["initial_idea"]
            },
            "INSPIRATION_TITLE_USER": {
                "name": "灵感模式-书名生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据用户的原始想法生成6个书名建议的用户提示词",
                "parameters": ["initial_idea"]
            },
            "INSPIRATION_DESCRIPTION_SYSTEM": {
                "name": "灵感模式-简介生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据用户想法和书名生成6个简介选项的系统提示词",
                "parameters": ["initial_idea", "title"]
            },
            "INSPIRATION_DESCRIPTION_USER": {
                "name": "灵感模式-简介生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据用户想法和书名生成6个简介选项的用户提示词",
                "parameters": ["initial_idea", "title"]
            },
            "INSPIRATION_THEME_SYSTEM": {
                "name": "灵感模式-主题生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据书名和简介生成6个深刻的主题选项的系统提示词",
                "parameters": ["initial_idea", "title", "description"]
            },
            "INSPIRATION_THEME_USER": {
                "name": "灵感模式-主题生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据书名和简介生成6个深刻的主题选项的用户提示词",
                "parameters": ["initial_idea", "title", "description"]
            },
            "INSPIRATION_GENRE_SYSTEM": {
                "name": "灵感模式-类型生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据小说信息生成6个合适的类型标签的系统提示词",
                "parameters": ["initial_idea", "title", "description", "theme"]
            },
            "INSPIRATION_GENRE_USER": {
                "name": "灵感模式-类型生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据小说信息生成6个合适的类型标签的用户提示词",
                "parameters": ["initial_idea", "title", "description", "theme"]
            },
            "INSPIRATION_QUICK_COMPLETE": {
                "name": "灵感模式-智能补全",
                "category": "灵感模式",
                "description": "根据用户提供的部分信息智能补全完整的小说方案",
                "parameters": ["existing"]
            },
            "AI_DENOISING": {
                "name": "AI去味",
                "category": "文本润色",
                "description": "将文本改写为更自然的中文表达，降低模板腔和AI腔",
                "parameters": ["original_text"]
            }
        }
        
        for key, info in template_definitions.items():
            template_content = getattr(cls, key, None)
            if template_content:
                templates.append({
                    "template_key": key,
                    "template_name": info["name"],
                    "category": info["category"],
                    "description": info["description"],
                    "parameters": info["parameters"],
                    "content": template_content
                })
        
        return templates
    
    @classmethod
    def get_system_template_info(cls, template_key: str) -> dict:
        """
        获取指定系统模板的信息
        
        Args:
            template_key: 模板键名
            
        Returns:
            模板信息字典
        """
        all_templates = cls.get_all_system_templates()
        for template in all_templates:
            if template["template_key"] == template_key:
                return template
        return None

# ========== 全局实例 ==========
prompt_service = PromptService()
