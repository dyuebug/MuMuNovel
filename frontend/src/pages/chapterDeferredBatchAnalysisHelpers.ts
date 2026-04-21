import { message } from 'antd';
import { chapterApi } from '../services/modularApi';
import type { AnalysisTask, Chapter } from '../types';

export function selectDeferredBatchAnalysisCandidates({
  startChapterNumber,
  count,
  latestChapters,
}: {
  startChapterNumber: number;
  count: number;
  latestChapters: Chapter[];
}): Chapter[] {
  const targetChapterNumbers = new Set(
    Array.from({ length: count }, (_, index) => startChapterNumber + index)
  );

  return latestChapters.filter((chapter) => (
    targetChapterNumbers.has(chapter.chapter_number)
    && Boolean(chapter.content && chapter.content.trim() !== '')
  ));
}

export async function queueDeferredBatchAnalysis({
  projectId,
  startChapterNumber,
  count,
  latestChapters,
  analysisTasksMap,
  startPollingTask,
  loadAnalysisTasks,
}: {
  projectId: string;
  startChapterNumber: number;
  count: number;
  latestChapters: Chapter[];
  analysisTasksMap: Record<string, AnalysisTask>;
  startPollingTask: (chapterId: string) => void;
  loadAnalysisTasks: (chaptersToLoad?: Chapter[]) => Promise<void>;
}): Promise<void> {
  if (count <= 0) {
    return;
  }

  const candidateChapters = selectDeferredBatchAnalysisCandidates({
    startChapterNumber,
    count,
    latestChapters,
  });

  if (candidateChapters.length === 0) {
    return;
  }

  let queuedCount = 0;
  let skippedCount = 0;
  let failedCount = 0;

  const ensureAnalysisTask = async (chapter: Chapter) => {
    const localTask = analysisTasksMap[chapter.id];
    if (localTask?.has_task && ['pending', 'running', 'completed'].includes(localTask.status)) {
      skippedCount += 1;
      if (localTask.status === 'pending' || localTask.status === 'running') {
        startPollingTask(chapter.id);
      }
      return;
    }

    try {
      const remoteTask = await chapterApi.getChapterAnalysisStatus(chapter.id, projectId);
      if (remoteTask.has_task && ['pending', 'running', 'completed'].includes(remoteTask.status)) {
        skippedCount += 1;
        if (remoteTask.status === 'pending' || remoteTask.status === 'running') {
          startPollingTask(chapter.id);
        }
        return;
      }
    } catch (error) {
      console.error('Failed to query existing analysis task.', error);
    }

    try {
      await chapterApi.triggerChapterAnalysis(chapter.id, projectId);
      queuedCount += 1;
      startPollingTask(chapter.id);
    } catch (error) {
      failedCount += 1;
      console.error('Failed to queue analysis for chapter ' + chapter.chapter_number + '.', error);
    }
  };

  const chunkSize = 3;
  for (let index = 0; index < candidateChapters.length; index += chunkSize) {
    const chunk = candidateChapters.slice(index, index + chunkSize);
    await Promise.all(chunk.map(ensureAnalysisTask));
  }

  if (queuedCount > 0) {
    message.info(`Queued analysis for ${queuedCount} chapter(s).`);
  } else if (skippedCount > 0 && failedCount === 0) {
    message.info('Analysis tasks already exist for the selected chapters.');
  }

  if (failedCount > 0) {
    message.warning(`Failed to queue analysis for ${failedCount} chapter(s).`);
  }

  await loadAnalysisTasks(latestChapters);
}
