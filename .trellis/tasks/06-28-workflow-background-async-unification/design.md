# 全流程后台异步任务统一化设计

## Scope

第一阶段改 Inspiration 灵感模式，第二阶段改章节局部重写。目标是证明“页面调用 -> 创建后台任务 -> 轮询完成 -> 消费原结果结构”这条路径可复用。
第三阶段改拆书导入 apply/retry，目标是证明“已有页面内 SSE 流程 -> 统一后台任务 -> 保留失败步骤重试能力”也可以复用同一套任务中心。
第四阶段改 Wizard 世界观重生成，目标是在后端已有 `world_regenerate` task type 的前提下移除前端旧 SSE 调度入口。
第五阶段改 feature command 中的大纲/角色生成，目标是避免非页面入口绕过统一后台任务中心。
第六阶段改 AI 去味，目标是让单条/批量润色也能通过统一后台任务执行和呈现。
第七阶段补齐捕获型 SSE 后台任务进度桥接，目标是让复用旧 SSE 服务实现的后台任务也能在任务中心持续更新 `message/progress`，而不是只在最终完成时同步结果。
第八阶段改整章重生成，目标是让仍有 UI 调用方的 `/chapters/{chapter_id}/regenerate-stream` 长耗时 AI 路径默认通过 `background_tasks` 执行，旧 SSE 路由仅保留为兼容入口。

## Backend Contract

复用 `POST /api/background-tasks`：

- `task_type = inspiration_generate_options`
- `task_type = inspiration_refine_options`
- `task_type = inspiration_quick_generate`
- `task_type = chapter_regenerate`
- `task_type = chapter_partial_regenerate`

任务 payload 使用原同步接口 payload，不额外发明新 schema。任务完成后 `TaskRecord.result` 写入原同步接口返回的 JSON。

## Frontend Contract

`frontend/src/services/modules/inspiration.ts` 保留原同步方法，同时新增后台任务封装：

- 创建对应 task type。
- 使用 `waitForBackgroundTaskCompletion()` 轮询 `/background-tasks/{task_id}`。
- 返回值保持 `InspirationOptionResponse` 或 quick-generate 原结构。

页面层切换调用方法，避免重构聊天状态机；确认阶段额外提供“智能补全并创建”，通过 `quickGenerateInBackground()` 后台补齐当前配置后再进入执行设置。

`frontend/src/services/modules/chapterPartialRegeneration.ts` 新增后台任务封装，保持返回：

- `new_text`
- `word_count`
- `original_word_count`
- `start_position`
- `end_position`

`PartialRegenerateModal` 不再直连 SSE stream；改为后台任务轮询完成后一次性展示结果。

`ChapterRegenerationModal` 不再直连 `/chapters/{chapter_id}/regenerate-stream`；改为创建 `chapter_regenerate` 后台任务并轮询完成，`project_id` 使用当前项目 ID，`chapter_id` 放入 payload/checkpoint，最终消费 `{ content, word_count, task_id, analysis_task_id }`。

`BookImport` 也保留原任务与导入页面结构，但把 `apply-stream` 和 `retry-stream` 替换为后台任务封装，失败步骤在最终 `result` 中回填，供页面继续重试。

`wizardStreamApi.regenerateWorldBuildingStream` 保持函数名兼容调用方，但内部改为 `runBackgroundTaskWithPolling('world_regenerate', projectId, payload)`。

`useOutlineCommands.generateOutlines` 和 `useCharacterCommands.generateCharacter` 保持命令 API 不变，但内部改为创建 `outline_generate` / `character_generate` 后台任务并轮询完成，再同步 Zustand store。

`polishApi.polishTextInBackground` 和 `polishApi.polishBatchInBackground` 新增后台封装；后端 `background_tasks` 反序列化原 `/polish` payload，复用 `api/polish.rs` 的执行函数。

捕获型 SSE 后台任务使用 `SseChannel::with_captures` 时，同时启动 `spawn_channel_progress_bridge()`：

- 桥接器轮询 `SseTaskCapture` 的 `message/progress/status`。
- 仅当进度状态变化时更新 `TaskRecord.message/progress/updated_at` 并通过 `TaskStreamHub` fanout `progress` 事件。
- 桥接器不写入 `Completed/Failed` 终态，最终状态仍由 `sync_channel_state_to_task()`、`complete_task()`、`fail_task()` 负责，避免状态竞争。
- 当前接入 wizard world/career/characters/outline、world regenerate、book import apply/retry。

## Compatibility

原 `/inspiration/generate-options`、`/inspiration/refine-options`、`/inspiration/quick-generate` 保持不变，作为兼容入口和调试回退。
原 `/chapters/{chapter_id}/regenerate-stream` 保持不变，作为兼容入口和调试回退。
原 `/chapters/{chapter_id}/partial-regenerate-stream` 保持不变，作为兼容入口和调试回退。
原 `/book-import/tasks/{task_id}/apply-stream` 和 `/book-import/tasks/{task_id}/retry-stream` 保持不变，作为兼容入口和调试回退。
原 `/wizard-stream/world-building/{project_id}/regenerate` 保持不变，作为兼容入口和调试回退。

## Risks

- Inspiration 没有项目 ID，后台任务创建目前只允许 `wizard_world_building` 缺省 `project_id`。需要扩展白名单。
- 后台任务类型 union 需要同时更新服务类型、store 显示和任务中心兼容逻辑。
- 任务失败和降级响应必须区分：临时 AI 错误应作为成功 result 返回，配置错误才失败。
- 局部重写从 chunk 流式显示改为后台完成后显示完整结果，交互反馈要依赖任务进度而不是文本增量。
- 拆书导入后台化保留原 book-import task 作为业务上下文，global background task 只承载执行状态，不接管导入页面的 task 生命周期。
- Wizard 更新世界观和清理向导数据仍保留原 SSE 入口；更新是轻量编辑，清理是删除类操作，不纳入本轮生成类长任务后台化范围。
- `outlineApi.generateOutline` 和 `characterApi.generateCharacter` 仍作为兼容同步 service 方法保留，但默认 command 调用不再使用它们。
- `/polish` 和 `/polish/batch` 原同步路由保留；后台任务封装作为新默认可用入口，调用方可逐步切换。
- 捕获型 SSE 进度桥接只同步非终态进度，若底层 SSE 服务缺少中间 `progress()` 调用，则仍只能看到开始与最终完成状态。
- 整章重生成后台任务目前不保留正文 chunk 增量预览；页面完成后一次性应用最终正文，运行中反馈依赖任务中心进度。
