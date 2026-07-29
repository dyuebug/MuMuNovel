# Design：后台任务快照原子化

## 架构边界

R1 只修改 Rust 后台任务文件持久化 owner：

```text
TaskRegistry
  -> tasks::persistence
       -> background_tasks.json.tmp
       -> background_tasks.json
       -> background_tasks.json.bak
       -> background_tasks.json*.corrupt-<timestamp>-<suffix>
```

不修改 API、TaskRecord schema、任务执行器或 R2 恢复分类。

## 文件协议

生产目录仍为 `data/runtime`，文件角色如下：

| 文件 | 角色 |
|---|---|
| `background_tasks.json` | 当前主快照 |
| `background_tasks.json.bak` | 上一份已验证主快照 |
| `background_tasks.json.tmp` | 已写入但尚未提交的新快照 |
| `*.corrupt-*` | 解析失败或版本不支持的隔离证据 |

所有候选均位于同一目录，避免跨文件系统 rename。

## 保存数据流

```text
acquire process-local save mutex
  -> collect and sort TaskRecord
  -> serialize Snapshot(version=1)
  -> create snapshot directory
  -> open temp(create + truncate)
  -> write_all
  -> flush
  -> sync_all(temp)
  -> validate serialized temp bytes as Snapshot v1
  -> inspect existing primary
       valid   -> remove stale backup, rename primary -> backup
       corrupt -> quarantine primary, keep existing valid backup
       missing -> continue
  -> rename temp -> primary
  -> best-effort sync parent directory where supported
  -> release mutex
```

如果 temp 提交失败且 primary 已旋转为 backup，则执行 best-effort rollback：当 primary 缺失时尝试 `backup -> primary`。即使 rollback 失败，启动加载仍会读取 backup。

## 加载数据流

候选顺序：

```text
primary -> backup -> temp
```

每个候选的处理：

1. NotFound：继续下一个候选；
2. I/O error：记录错误并继续 fallback，不删除文件；
3. JSON parse/version error：移动到唯一 `.corrupt-*`，继续 fallback；
4. valid v1：加载 `snapshot.items`，停止搜索。

如果所有候选均不可用，注册表保持空并输出可诊断日志。

## API 设计

生产入口保持：

```rust
pub async fn load_from_disk(registry: &TaskRegistry)
pub async fn save_to_disk(registry: &TaskRegistry)
pub fn start_periodic_save(registry: TaskRegistry)
```

模块内部增加可测试 owner：

```rust
async fn load_from_dir(registry: &TaskRegistry, dir: &Path) -> Result<LoadOutcome, SnapshotPersistenceError>
async fn save_to_dir(registry: &TaskRegistry, dir: &Path) -> Result<(), SnapshotPersistenceError>
```

生产函数只负责使用默认目录并将结构化错误转为日志。内部错误不得包含 snapshot payload。

## 并发模型

使用模块级 `tokio::sync::Mutex<()>` 串行化所有保存操作。当前单 runtime owner 不需要跨进程锁；R1 不引入文件锁或 lockfile。

加载只发生在周期保存启动前，因此无需与生产保存竞争。测试中的并发保存也走相同互斥边界。

## Windows/Linux 兼容策略

不依赖 rename 覆盖目标：

1. 若 primary 有效，先将其 rename 到 backup；
2. 再将 temp rename 到已不存在的 primary。

这两个 rename 各自是同目录原子操作。两步之间若崩溃，backup 仍是有效恢复候选。Unix 可额外 best-effort sync 父目录；Windows 不把无法打开目录进行 sync 视为保存失败。

## 错误与恢复矩阵

| 失败点 | 保留状态 | 后续恢复 |
|---|---|---|
| serialize 失败 | primary 未触碰 | 继续使用 primary |
| create/open temp 失败 | primary 未触碰 | 继续使用 primary |
| write/flush/sync temp 失败 | primary 未触碰 | 下次覆盖 temp |
| rotate primary 失败 | primary 仍存在 | 保存失败，继续使用 primary |
| commit temp 失败 | backup 已保留 | rollback 或启动从 backup 恢复 |
| primary parse 失败 | primary 被隔离 | 尝试 backup |
| primary/backup 均失败 | 损坏候选被隔离 | 尝试完整 temp |

## 测试策略

测试使用 `std::env::temp_dir()` + UUID 创建独立目录，并通过 RAII guard 清理。

至少覆盖：

- first save；
- second save backup rotation；
- corrupted primary fallback；
- missing primary fallback；
- valid temp fallback；
- unsupported version quarantine；
- temp open/write failure preserves primary；
- concurrent saves keep all committed candidates parseable。

## 回滚

仅需回滚 `tasks/persistence.rs`。主文件格式保持 version 1，因此新实现生成的 primary 可以被旧实现读取；旧实现会忽略新增 backup/temp/corrupt 文件。
