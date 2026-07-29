import { useEffect } from 'react';

import { backgroundTaskApi } from '../services/modules/backgroundTasks';
import { useModelOutputStream } from './useModelOutputStream';

/**
 * 订阅后台任务的瞬时模型输出预览。
 *
 * 任务状态仍由既有 HTTP polling 负责；此 Hook 只管理 React 内存中的输出状态，
 * 并委托 backgroundTaskApi 管理可选 SSE 连接。
 */
export const useBackgroundTaskOutputStream = (
  taskId: string | null,
  enabled = true,
) => {
  const {
    reasoningContent,
    generatedContent,
    reasoningTruncated,
    contentTruncated,
    resetModelOutput,
    onReasoningChunk,
    onChunk,
  } = useModelOutputStream();

  useEffect(() => {
    if (!enabled || !taskId) {
      return;
    }

    // 后台任务在相邻 Tick 之间会短暂清空 active task id。此时保留上一调用的
    // 内存预览，避免模型刚完成用户就看不到输出；新 task id 到来时再开始新面板。
    resetModelOutput();
    return backgroundTaskApi.subscribeTaskStream(taskId, {
      onChunk,
      onReasoningChunk,
    });
  }, [enabled, onChunk, onReasoningChunk, resetModelOutput, taskId]);

  return {
    reasoningContent,
    generatedContent,
    reasoningTruncated,
    contentTruncated,
  };
};

export default useBackgroundTaskOutputStream;
