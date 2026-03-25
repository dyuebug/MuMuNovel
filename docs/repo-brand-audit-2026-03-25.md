# 仓库命名与品牌残留审计（2026-03-25）

## 结论摘要

本次扫描聚焦两类关键字符串：

- `xiamuceer-j`（旧仓库 owner / 历史作者）
- `MuMuAINovel`（旧品牌名 / 历史项目名）

基于本轮清理后的结果，可以把残留分为三类：

1. **仓库地址错误**：会导致跳错仓库、Issue 或 clone 地址。这一类已经清理到 **0 个剩余**。
2. **品牌/产品命名**：仍广泛存在于 UI 文案、部署配置、Docker 镜像名、环境变量默认值、文档标题中。这一类暂时 **保留**，因为它属于产品命名而不是仓库地址错误。
3. **历史/归属/工具元数据**：少量残留于作者字段、工具缓存或辅助索引中，不影响当前发版链路。

## 已修复的仓库地址错误

本轮额外修复了以下明确错误引用：

- `CLAUDE.md` 中的 clone 地址、项目仓库地址、目录进入示例
- `.claude/team-plan/api-compatibility-summary.md` 中的旧 Issue 地址

结合前面已经完成的修复，当前仓库相关地址已统一到：

- 仓库：`https://github.com/dyuebug/MuMuNovel`
- Issue：`https://github.com/dyuebug/MuMuNovel/issues`

## 分类结果

### A. 仓库地址错误（已清零）

当前扫描结果中，已不再发现以下错误类型：

- `https://github.com/xiamuceer-j/MuMuAINovel`
- `https://github.com/xiamuceer-j/MuMuAINovel.git`
- `git@github.com:xiamuceer-j/MuMuAINovel.git`
- `https://github.com/xiamuceer-j/MuMuAINovel/issues`
- `cd MuMuAINovel`（作为 clone 后目录进入示例）

### B. 品牌 / 产品命名残留（建议按“是否要正式改名”单独处理）

这些内容当前更像“产品名 / 服务名 / 镜像名 / UI 文案”，并不等于错误：

- `README.md:1`：项目标题仍为 `MuMuAINovel`
- `docker-compose.yml:70`：`APP_NAME=${APP_NAME:-MuMuAINovel}`
- `backend/scripts/entrypoint.sh:20`：默认 `APP_NAME` 仍为 `MuMuAINovel`
- `frontend/src/config/version.ts:16`：`projectName: 'MuMuAINovel'`
- `frontend/src/pages/Login.tsx:344`：登录页品牌文案
- `frontend/src/pages/ProjectList.tsx:529`：页面标题品牌文案
- `frontend/src/pages/Sponsor.tsx:113`：赞助页品牌文案
- `docs/01-项目概览.md:3`：文档标题仍为 `MuMuAINovel`
- `docs/09-部署指南.md:422`：systemd 描述 `MuMuAINovel Backend`
- `.github/workflows/docker-build.yml:10`：Docker 镜像名 `mumujie/mumuainovel`

**判断**：这些内容应视为“是否执行完整品牌更名”的决策范围，而不应混入仓库地址清理中顺手修改。

### C. 历史 / 归属 / 工具元数据残留（低优先级）

这一类不会影响当前版本发布，但反映了历史来源：

- `frontend/src/config/version.ts:28`：`author: 'xiamuceer-j'`
- `.claude/index.json:3`：工具索引里的历史 `project_name`
- `.gitignore:111`：`mumuainovel.md` 忽略项
- `.env:2` / `.env:8`：本地环境文件中的旧项目名与 `APP_NAME`

**判断**：
- `author: 'xiamuceer-j'` 更像历史作者归属信息，不建议机械替换为仓库 owner。
- `.claude/index.json` 属于工具索引，后续可通过工具重建而非手改。
- `.env` 属于本地环境文件，不建议在自动清理里主动改动。

## 风险评估

如果继续做“全量品牌更名”，主要风险在于：

- Docker 镜像名、容器名、服务名变化会影响现有部署和脚本兼容性
- UI 文案与历史项目名切换，可能影响用户认知和既有截图/文档对应关系
- `APP_NAME`、systemd description、镜像仓库名、工作流产物名需要一起设计，不能零散改
- 旧品牌名可能还出现在第三方平台、镜像仓库、社区贴文和下载链接中

## 建议的后续路线

### 方案 1：维持“仓库已迁移，产品名暂不改”

适合当前状态，优点是风险最低：

- 保持 `MuMuAINovel` 作为产品名
- 保持 Docker 镜像 / 服务名不变
- 仅保证仓库地址、Release 地址、Issue 地址全部正确

### 方案 2：执行完整品牌更名（单独开任务）

如果你准备把产品名也统一到 `MuMuNovel`，建议单独立项，至少分 4 批处理：

1. **UI 与文案层**：标题、登录页、页脚、赞助页、文档标题
2. **配置与运行层**：`APP_NAME`、systemd、Docker Compose、容器名
3. **镜像与发布层**：Docker image、工作流、README 部署示例
4. **工具与元数据层**：索引、缓存、示例配置、历史说明

## 本次审计范围说明

扫描时已排除以下目录或生成产物，避免噪音：

- `.git`
- `node_modules`
- `backend/static`
- `__pycache__`
- `logs`
- `data`
- `.codex-tmp`
- `profiling-artifacts`
- `.serena`

因此，这份结果更接近“源码 / 文档 / 配置层面的真实残留”，适合作为后续更名决策依据。
