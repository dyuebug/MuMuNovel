# 章节 API 网关边界说明

## 1. 目的

章节相关接口是 MuMuNovel 中最复杂、最容易继续膨胀的一条业务链路。

本文档用于说明：

- 路由层应该保留什么职责
- 哪些逻辑应该下沉到 workflow / service / helper
- 后续继续拆分时应该遵守哪些边界

## 2. 总体原则

章节 API 路由层应只承担以下职责：

- request boundary
- 参数解析与基础校验
- 权限与上下文接入
- 调用下层 workflow / service
- 输出兼容响应结构

路由层不应长期承载：

- 大段业务决策逻辑
- 多分支流程编排
- 复杂状态组装
- 大量与展示层强绑定的兼容拼接

## 3. 建议边界划分

### 3.1 `chapter_draft`

适合承载：

- 草稿生成
- 自动修订草稿
- 候选草稿处理
- 与草稿状态相关的查询与落库协调

### 3.2 `chapter_crud`

适合承载：

- 章节基础创建、更新、删除
- 章节基础查询
- 常规内容持久化相关逻辑

### 3.3 `chapter_regeneration`

适合承载：

- 章节重生成任务创建与查询
- regeneration 相关工作流编排
- 任务恢复、状态查询与阶段输出

### 3.4 `chapter_batch_generation`

适合承载：

- 批量生成任务入口
- 批量任务状态流转
- 批量任务恢复、取消、进度查询
- 与批量分析、修复策略联动的协调逻辑

当前在 Rust 端已经验证可行的细化边界：

- route 只保留 HTTP / SSE boundary、参数解析、权限检查、`AIConfig` 构建
- route 负责 `tokio::spawn` 外壳，但不再内联 task create / checkpoint / stream payload 组装
- create / cancel / resume 进入 workflow-style service helper
- status / active list / SSE event payload 进入 query / stream helper
- runtime snapshot、checkpoint 推进、single / batch executor 进入 runtime service helper

## 4. Route seam 约定

所谓 seam，可以理解为“可继续抽离的结构边界”。

在章节 API 中，优先保留以下 seam：

- route → workflow
- route → query service
- route → compat response adapter
- route → access / request context helper
- route → stream event builder
- route → runtime executor

这样做的价值在于：

- 便于逐步拆分，而不是一次性重写
- 便于为高风险链路补测试
- 便于在不改变接口的前提下继续重构内部结构

对 `chapter_batch_generation` 而言，这条 seam 现在应被视为稳定约束：

1. route 不再拥有任务创建计划拼装逻辑
2. route 不再拥有 checkpoint / snapshot 持久化逻辑
3. route 不再拥有 active task / status response 拼装逻辑
4. route 不再拥有 SSE polling state 与 event payload 拼装逻辑
5. route 可以暂时保留 `tokio::spawn`、请求上下文提取与兼容响应收口

## 5. 测试建议

每次沿 seam 做拆分时，建议至少补以下验证：

1. route delegation regression
2. 输入参数与错误分支验证
3. 兼容响应结构验证
4. 高风险任务状态流转验证
5. stream event shape 与 polling 结束条件验证
6. active task / status list 响应组装 helper 单测

## 6. 变更建议

后续继续重构章节 API 时，应遵守以下顺序：

1. 先抽离 query / workflow / helper
2. 再收缩 route 文件体积
3. 最后考虑是否引入更细粒度 domain service

不建议直接大规模重写 route 文件，否则容易同时引入结构变更和行为变更。

对 Rust `chapter_batch_generation` 的下一步建议：

1. 默认复用 `create plan` / `view context` / `stream builder` / `runtime executor` 四类 helper 模式
2. 只有出现重复模式时，才继续往更细的 domain service 抽象
3. 在没有明确收益前，不继续为了“文件更小”而拆 route

## 7. 结论

章节 API 的关键不是“把文件拆小”，而是让边界稳定：

- 路由只负责边界
- 工作流负责流程
- 服务负责领域逻辑
- 兼容层负责旧响应与旧调用的平滑过渡

只要 seam 保持清晰，后续重构就可以持续小步推进。
