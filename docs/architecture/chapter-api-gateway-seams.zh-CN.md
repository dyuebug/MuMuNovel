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

## 4. Route seam 约定

所谓 seam，可以理解为“可继续抽离的结构边界”。

在章节 API 中，优先保留以下 seam：

- route → workflow
- route → query service
- route → compat response adapter
- route → access / request context helper

这样做的价值在于：

- 便于逐步拆分，而不是一次性重写
- 便于为高风险链路补测试
- 便于在不改变接口的前提下继续重构内部结构

## 5. 测试建议

每次沿 seam 做拆分时，建议至少补以下验证：

1. route delegation regression
2. 输入参数与错误分支验证
3. 兼容响应结构验证
4. 高风险任务状态流转验证

## 6. 变更建议

后续继续重构章节 API 时，应遵守以下顺序：

1. 先抽离 query / workflow / helper
2. 再收缩 route 文件体积
3. 最后考虑是否引入更细粒度 domain service

不建议直接大规模重写 route 文件，否则容易同时引入结构变更和行为变更。

## 7. 结论

章节 API 的关键不是“把文件拆小”，而是让边界稳定：

- 路由只负责边界
- 工作流负责流程
- 服务负责领域逻辑
- 兼容层负责旧响应与旧调用的平滑过渡

只要 seam 保持清晰，后续重构就可以持续小步推进。