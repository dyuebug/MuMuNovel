# Chapter Render Profiling Checklist

## 目标

这份清单用于排查章节相关界面的重复渲染、状态扩散和不必要的重计算问题，重点关注以下组件：

- `ChapterEditorModalContent`
- `ChapterBatchGenerateModal`
- `ChapterEditorAiSection`
- `PartialRegenerateModal`

## 开启 render debug

先在浏览器控制台开启全局调试开关：

```js
window.enableNovelRenderDebug()
```

如果只想观察指定组件，可以只传入需要关注的组件名：

```js
window.enableNovelRenderDebug([
  'ChapterEditorModalContent',
  'ChapterBatchGenerateModal',
  'PartialRegenerateModal',
])
```

排查结束后记得关闭：

```js
window.disableNovelRenderDebug()
```

## React DevTools Profiler 操作

### 场景 1：打开章节页并进入编辑器

1. 打开章节列表页。
2. 启动 React DevTools Profiler。
3. 点击进入章节编辑器。
4. 观察首个 commit。

**关注点：**
- `Chapters` 是否因为无关状态被整页重渲染。
- `ChapterEditorModalContent` 首次打开是否只触发必要渲染。
- `ChapterEditorAiSection` 是否因为 props 抖动发生重复计算。

### 场景 2：生成任务状态刷新

1. 触发章节生成或批量生成。
2. 观察任务在 `pending` / `running` 之间切换。
3. 对照 profiler 结果和 render debug 日志。

**关注点：**
- 是否只有受影响的章节卡片或弹窗刷新。
- `changedKeys` 是否指向真实变化的字段。
- 任务状态变更是否把无关子树也带着刷新。

### 场景 3：批量生成配置切换

1. 打开批量生成弹窗。
2. 切换模型、风格、目标字数等配置。
3. 观察 `ChapterBatchGenerateModal` 的 render debug 输出。

**关注点：**
- `selectedModel`、`selectedStyleId`、`sortedChapters` 是否被不必要重建。
- `batchGenerating`、`batchProgress` 是否导致无关区域跟着重渲染。

### 场景 4：编辑器局部操作

1. 打开章节编辑器。
2. 连续输入大约 10 次文本。
3. 选中局部段落。
4. 触发局部重写或 AI 操作。

**关注点：**
- 输入正文时，是否拖着 `ChapterEditorModalContent` 整体刷新。
- `Chapters` 页面是否被编辑器局部状态反向影响。
- `PartialRegenerateModal` 打开/关闭时 editor lazy chunk 是否重复加载。

## 观察 render debug 输出

日志格式示例：

```text
[render-debug] ComponentName #N
```

排查时重点看：

- `changedKeys`：确认是哪一组 props 或 store 字段触发了重渲染。
- `snapshot`：确认日志记录的状态是否真的发生了业务变化。

如果发现同一组件在无关操作下连续重渲染，优先检查：

- `changedKeys` 是否暴露出稳定引用被重复创建的问题。
- 是否把临时 UI state 提升到了过高层级。
- 是否缺少 `memo`、`useMemo`、`useCallback` 等边界。

## 最小验证

完成一轮修复后，至少执行以下检查：

```powershell
npm exec eslint src/pages/Chapters.tsx src/components/ChapterEditorModalContent.tsx src/components/ChapterEditorAiSection.tsx src/components/ChapterBatchGenerateModal.tsx src/components/PartialRegenerateModal.tsx src/App.tsx
npm exec tsc -b
npm run build:analyze
```

## 常见结论

- `Chapters` 经常会因为列表派生数据不稳定而重复渲染。
- 编辑器区域容易因为 `ChapterEditorAiSection` 的 props 变化产生级联刷新。
- 批量生成相关弹窗如果复用过多父层状态，通常会放大渲染范围。
