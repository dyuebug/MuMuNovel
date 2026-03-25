# Changelog

## v1.3.9 - 2026-03-25

### 新增
- 新增一键发版脚本 `release.ps1` 与 `release.bat`，可串联版本校验、tag 推送和 GitHub Release 创建
- 新增 `docs/releases/v1.3.9.md` 正式发布说明，方便复用到 Release 页面与外部公告

### 修复
- 修正应用内版本检查与更新日志抓取的数据源，统一指向当前仓库 `dyuebug/MuMuNovel`
- 对齐 `frontend/package.json`、后端配置与环境示例中的版本元数据到 `1.3.9`
- 更新 README 中的版本徽章与仓库链接，避免发布信息继续指向旧仓库

### 验证
- 执行 `npm run build`，验证前端版本检查与更新日志相关改动可正常构建
- 执行 `python -m py_compile backend/app/api/changelog.py backend/app/config.py`
- 执行 `release.ps1 -Version v1.3.9 -DryRun`，验证 changelog、release notes、tag、push 与 GitHub Release 流程

## v1.3.8 - 2026-03-25

### 新增
- 打通项目默认质量偏好到章节生成链路，减少重复配置并提升长篇创作一致性
- 批量生成弹窗新增 `质量预设`、`额外质量要求` 与“恢复项目默认”操作
- 剧情分析页新增章节“质量验收”面板，集中展示得分、最弱项与修订建议

### 优化
- 统一透传 `quality_preset` 与 `quality_notes` 到单章生成和批量生成请求
- 分析页聚合展示质量建议、修订草稿摘要与质量画像信息，减少信息割裂

### 验证
- 前端构建通过
- 应用重部署成功并通过健康检查
- 浏览器冒烟验证通过，确认批量生成弹窗新增字段和质量验收卡均可用

### 相关提交
- `6d262b8` `feat(outline): apply project defaults across initial generation`
- `d40055e` `feat(chapters): carry quality defaults into generation`
- `e53bf06` `feat(analysis): add chapter quality acceptance panel`
