# 叙事规则 DSL 标准库 v1.1

版本: v1.1
日期: 2026-03-11
范围: 42 个模板（通用 12 / 言情 10 / 玄幻 8 / 悬疑 8 / 权谋 4）
定位: 将 DSL 语法推进到可复用策略模板库，输出可编译的章节任务、场景骨架、预算动作与验收清单。

## 命名与版本
- 模板命名: CATEGORY/PATTERN
- 版本标记: @v1.1
- 标签: tags=["..."]

## 元数据结构
每个模板必须包含以下字段:
- DESCRIPTION
- TAGS
- INPUTS
- OUTPUTS

## 通用输入参数
- target_relation / target_character / target_arc
- intensity: low | medium | high
- stage: setup | midgame | pre_climax | climax
- allow / forbid
- budget_bias: 资源偏好数组
- hook_type: emotional-suspend | info-gap | stance-split
- style_bias: restrained | sharp | warm | cold

## 通用输出
- ChapterTask
- SceneSkeleton
- BudgetActionPack
- ValidationChecklist

## 组合规则 v1.1
- 单章最多启用 2 个高价值模板。
- 使用 Pacing/Preclimax_Hold 时，twist 与 world_expansion 自动进入冷却优先级。
- Romance/Near_Miss_Confession 与 Romance/Misunderstanding_Release 同章需降低亲密轴增量。
- Mystery/Red_Herring 与 Mystery/Reveal_Partial 同章必须降低揭露强度。
- Fantasy/World_Expand_Soft 与 Info/Reveal_Controlled 低阈值冲突时，优先保人物推进。

## 越权协议
作者可显式越权，系统必须记录并回写:
- OVERRIDE TEMPLATE "..." WITH "..."
- KEEP_ROUGH line("...")
- IGNORE_WARNING "..."

---

# 通用模板（12）

## CORE/CHAPTER_FOCUS @v1.1
用途: 章节聚焦单主任务
Inputs: goal, forbid[]
Outputs: chapter_goal, validation_rules

## CORE/SCENE_FUNCTION_BALANCE @v1.1
用途: 场景功能配比控制
Inputs: primary, secondary
Outputs: scene_skeleton, budget_actions

## CORE/LOW_EXPLAIN_MODE @v1.1
用途: 低解释密度模式
Inputs: density_target
Outputs: writing_constraints

## CORE/SUBTEXT_PUSH @v1.1
用途: 潜台词增强
Inputs: intensity
Outputs: writing_constraints

## BUDGET/LOW_COST_SUBSTITUTE @v1.1
用途: 低成本替代高成本
Inputs: substitute_pair
Outputs: budget_actions

## BUDGET/COOLDOWN_RESOURCE @v1.1
用途: 资源冷却
Inputs: resource, cooldown
Outputs: budget_actions

## BUDGET/REPAY_SMALL @v1.1
用途: 小额偿还情绪或信任
Inputs: repay_target
Outputs: budget_actions, scene_skeleton

## PACING/MIDGAME_STABILIZE @v1.1
用途: 中盘稳态
Inputs: primary_goal, budget_bias[]
Outputs: chapter_goal, budget_actions

## PACING/PRECLIMAX_HOLD @v1.1
用途: 高潮前保峰
Inputs: hold_resources[]
Outputs: budget_actions, validation_rules

## VALIDATE/CANON_GATE_BASIC @v1.1
用途: 基础正史闸门
Inputs: min_pass_rate
Outputs: validation_rules

## VALIDATE/RISK_AUDIT @v1.1
用途: 风险清单审计
Inputs: risk_tags[]
Outputs: validation_rules

## CORE/REL_AXIS_ONE_STEP @v1.1
用途: 关系单轴推进
Inputs: target_relation, axis, delta
Outputs: chapter_goal, validation_rules

---

# 言情模板（10）

## ROM/SLOWBURN_STEP @v1.1
用途: 慢燃推进一小步
Inputs: target_relation
Outputs: chapter_goal, scene_skeleton

## ROM/DISTANCE_REPAIR @v1.1
用途: 冷却后修复距离
Inputs: target_relation
Outputs: chapter_goal, budget_actions

## ROM/NEAR_MISS_CONFESSION @v1.1
用途: 差一点告白但未说出口
Inputs: target_relation
Outputs: scene_skeleton, hook_type

## ROM/SHARED_RISK_BIND @v1.1
用途: 共同冒险绑定
Inputs: target_relation, risk_level
Outputs: scene_skeleton, budget_actions

## ROM/MISUNDERSTANDING_RELEASE @v1.1
用途: 误解部分澄清
Inputs: target_relation
Outputs: chapter_goal, validation_rules

## ROM/QUIET_CONFIRMATION @v1.1
用途: 无台词确认关系
Inputs: target_relation
Outputs: scene_skeleton

## ROM/JEALOUSY_CONTROLLED @v1.1
用途: 控制型吃醋
Inputs: target_relation
Outputs: budget_actions, hook_type

## ROM/BOUNDARY_TEST @v1.1
用途: 关系边界测试
Inputs: target_relation
Outputs: chapter_goal, validation_rules

## ROM/EMO_PAYBACK_SMALL @v1.1
用途: 情绪小回收
Inputs: target_character
Outputs: budget_actions, scene_skeleton

## ROM/POST_CONFLICT_SOFTEN @v1.1
用途: 冲突后软化
Inputs: target_relation
Outputs: chapter_goal, writing_constraints

---

# 玄幻模板（8）

## FANT/WORLD_EXPAND_SOFT @v1.1
用途: 软扩容世界设定
Inputs: scope, cost_limit
Outputs: budget_actions, scene_skeleton

## FANT/POWER_STEP_WITH_COST @v1.1
用途: 小幅提升伴随代价
Inputs: target_character, cost_type
Outputs: chapter_goal, validation_rules

## FANT/SECT_PRESSURE @v1.1
用途: 门派压力推动人物选择
Inputs: target_character
Outputs: scene_skeleton

## FANT/HEAVEN_RULE_HINT @v1.1
用途: 天道规则暗示
Inputs: rule_hint_level
Outputs: info_reveal_constraints

## FANT/ARTIFACT_TENSION @v1.1
用途: 宝物诱发冲突
Inputs: artifact_level
Outputs: scene_skeleton, hook_type

## FANT/REALM_STALL_FIX @v1.1
用途: 境界停滞修复
Inputs: target_character
Outputs: chapter_goal, budget_actions

## FANT/TRIBULATION_BUFFER @v1.1
用途: 雷劫前缓冲
Inputs: target_character
Outputs: pacing, budget_actions

## FANT/FACTION_BALANCE_SHIFT @v1.1
用途: 势力平衡微调
Inputs: target_world
Outputs: world_state_update, validation_rules

---

# 悬疑模板（8）

## MYS/CLUE_DRIP @v1.1
用途: 线索滴灌
Inputs: clue_level
Outputs: scene_skeleton, info_reveal_constraints

## MYS/RED_HERRING @v1.1
用途: 误导线索投放
Inputs: mislead_level
Outputs: scene_skeleton, hook_type

## MYS/REVEAL_PARTIAL @v1.1
用途: 部分揭示
Inputs: secret_id
Outputs: budget_actions

## MYS/RECALL_ECHO @v1.1
用途: 旧线索回声
Inputs: clue_id
Outputs: scene_skeleton

## MYS/ALIBI_CRACK @v1.1
用途: 证词裂缝
Inputs: target_character
Outputs: scene_skeleton, hook_type

## MYS/TENSION_RISE_SILENT @v1.1
用途: 无声压强上升
Inputs: tension_level
Outputs: writing_constraints

## MYS/INVESTIGATION_STALL_FIX @v1.1
用途: 调查停滞修复
Inputs: target_arc
Outputs: chapter_goal

## MYS/REVEAL_CONTROLLED @v1.1
用途: 控制式揭露
Inputs: max_reveal_level
Outputs: info_reveal_constraints

---

# 权谋模板（4）

## POL/POWER_SHIFT_SMALL @v1.1
用途: 权力小转移
Inputs: target_relation
Outputs: scene_skeleton, relation_shift

## POL/ALLY_TEST @v1.1
用途: 盟友试探
Inputs: target_relation
Outputs: scene_skeleton, hook_type

## POL/COIN_TRADEOFF @v1.1
用途: 交易换取资源
Inputs: cost_type
Outputs: budget_actions, validation_rules

## POL/COURT_RUMOR @v1.1
用途: 宫廷流言发酵
Inputs: rumor_level
Outputs: scene_skeleton, info_reveal_constraints

---

# 使用示例

```dsl
USE TEMPLATE "ROM/SLOWBURN_STEP" {
  target_relation = relation("Lin:Qin")
}
```

```dsl
USE TEMPLATE "FANT/POWER_STEP_WITH_COST" {
  target_character = character("Lin")
  cost_type = "injury"
}
```

```dsl
USE TEMPLATE "MYS/REVEAL_PARTIAL" {
  secret_id = debt("QinIdentity")
}
```

```dsl
USE TEMPLATE "POL/ALLY_TEST" {
  target_relation = relation("Lin:Minister")
}
```

---

# 维护建议
- 模板必须可解释、可覆写、可回写。
- 单模板不得覆盖作者锁定节点。
- 任何模板输出不得直接进入正史，需经 VALIDATE 规则。
- 推荐在 v1.2 引入子类型变体与行业细分阈值。
