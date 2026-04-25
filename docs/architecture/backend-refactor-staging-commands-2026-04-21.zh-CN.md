# 后端重构分阶段命令清单（2026-04-21）

## 1. 目的

本文档用于记录后端重构过程中推荐执行的命令，便于本地自检、阶段性收口和交接复核。

## 2. 编码体检

```bash
python backend/tools/check_text_encoding_health.py
```

如需额外检查文档：

```bash
python backend/tools/check_text_encoding_health.py --include-docs
```

## 3. 指定测试

```bash
pytest backend/tests/test_tools/test_check_text_encoding_health.py -q
```

如果某轮重构涉及章节 API，请同步执行对应 API 测试。

## 4. 分阶段建议

### 阶段一：改动前

- 先跑编码体检
- 先确认当前测试基线
- 明确本轮只改一个职责批次

### 阶段二：改动中

- 小步提交
- 每完成一个边界调整就做局部验证
- 不把文档、结构与行为变更混在一起

### 阶段三：改动后

- 再跑编码体检
- 再跑相关测试
- 更新交付文档与风险说明