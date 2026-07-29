# PRD：后台任务快照原子化

## 目标

将 Rust 后台任务注册表的磁盘快照从直接覆盖 JSON 文件，升级为可崩溃恢复的双槽持久化协议。进程退出、磁盘写入中断或主机异常时，不应轻易留下唯一一份半写快照；启动加载应能隔离损坏文件并回退到上一份有效快照。

## 用户价值

- 服务重启后尽可能保留后台任务进度、结果和诊断信息。
- 单次保存失败不会破坏上一份可恢复状态。
- 损坏文件不会在每次启动时重复阻塞加载，并可保留用于排障。
- Windows 与 Linux 使用同一套可验证的快照协议。

## 已确认事实

1. 当前 owner 是 `backend-rs/src/tasks/persistence.rs`。
2. `save_to_disk()` 使用 `tokio::fs::write(background_tasks.json)` 直接截断并覆盖目标文件。
3. `load_from_disk()` 只尝试主文件；解析失败时仅记录日志，不隔离损坏文件，也不回退。
4. 保存周期为 1.5 秒，当前没有显式的保存互斥边界。
5. 生产启动顺序是 load snapshot → recover orphan tasks → start periodic save。
6. Tokio 已启用 `full` feature，现有依赖足以提供文件写入、flush、sync 和 rename，不需要新增 crate。
7. Windows 不保证 rename 可以覆盖已存在目标，因此不能只依赖 Unix 风格的 `rename(temp, primary)`。

## 功能要求

1. 快照必须先写入与主文件同目录的临时文件。
2. 临时文件必须完成 `write_all`、`flush` 和 `sync_all` 后才可参与提交。
3. 保存新快照前，上一份有效主快照必须保留为 backup。
4. 提交协议必须兼容 Windows：不得要求 rename 直接覆盖已有主文件。
5. 任意保存失败不得主动截断或覆盖上一份有效主快照。
6. 加载必须按明确顺序尝试主快照、backup 和已同步临时快照。
7. JSON 解析失败或快照版本不支持时，损坏候选必须移动到唯一的 `.corrupt-*` 隔离文件。
8. 找到有效 fallback 后必须加载注册表，并记录使用了哪个候选。
9. 周期保存和显式保存必须由同一个进程内互斥边界串行化。
10. 生产公开入口 `load_from_disk()`、`save_to_disk()` 和 `start_periodic_save()` 保持兼容。
11. 测试必须能够注入临时目录，不能读写真实 `data/runtime`。
12. 所有新增文本和代码保持 UTF-8 无 BOM。

## 验收标准

- [x] 首次保存生成可解析的 `background_tasks.json`。
- [x] 第二次成功保存后，primary 是新快照，backup 是上一份有效快照。
- [x] 临时文件在提交前完成 flush 和 sync。
- [x] primary 损坏时，启动加载会隔离它并从 backup 恢复。
- [x] primary 缺失时，可以从 backup 恢复。
- [x] primary 和 backup 不可用时，可以从完整有效的 temp 恢复。
- [x] 不支持的 snapshot version 被视为损坏候选而不是静默加载。
- [x] temp 写入失败时，既有 primary 内容保持不变。
- [x] 并发保存完成后 primary/backup 都不会出现半写 JSON。
- [x] 原有生产函数签名和启动调用无需修改。
- [x] targeted tests、完整 Rust tests、fmt、check 和 Clippy 增量门禁通过。

### 验收证据（2026-07-16）

快照原子写入、backup/temp 恢复、损坏隔离和并发保存合同已由本任务 `implement.md` 的定向测试与
路线级 Rust 回归覆盖。本次仅收口 PRD 勾选，不修改 snapshot 格式、生产函数签名或启动调用。

## 非目标

- 不在本任务中实现按 `task_type` 分类的恢复策略；该能力属于 R2。
- 不将后台任务快照迁移到 PostgreSQL。
- 不改变 `TaskRecord` JSON 字段或 snapshot version。
- 不修改前端任务中心、SSE 或业务任务执行逻辑。
- 不提供跨进程文件锁；当前生产部署只有一个 Rust runtime owner。
- 不保证断电场景下所有操作系统和文件系统都提供相同级别的目录元数据持久性。

## 兼容性约束

- 主文件继续使用 `data/runtime/background_tasks.json`。
- 旧的 version 1 主快照必须可以直接加载。
- backup/temp/corrupt 文件必须位于同一目录，保证 rename 不跨文件系统。
- 日志不得包含任务 payload 或用户敏感内容。

## 开放问题

无阻塞问题。双槽协议、候选加载顺序和损坏隔离策略均可由现有代码与路线文档确定。
