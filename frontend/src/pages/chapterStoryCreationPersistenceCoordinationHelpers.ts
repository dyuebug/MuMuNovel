import type { StoryCreationSnapshot } from '../utils/storyCreationDraft';
import {
  persistBatchStoryCreationDraft,
  persistSingleChapterStoryCreationDraft,
} from './chapterStoryCreationDraftPersistHelpers';
import {
  loadChapterStoryCreationSnapshots,
  restoreBatchStoryCreationDraft,
  restoreSingleChapterStoryCreationDraft,
} from './chapterStoryCreationRestoreHelpers';

type RestoreSingleStoryCreationPersistenceWorkflowArgs = Parameters<typeof restoreSingleChapterStoryCreationDraft>[0] & {
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
};

type RestoreBatchStoryCreationPersistenceWorkflowArgs = Parameters<typeof restoreBatchStoryCreationDraft>[0] & {
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
};

type PersistSingleStoryCreationDraftWorkflowArgs = Parameters<typeof persistSingleChapterStoryCreationDraft>[0];
type PersistBatchStoryCreationDraftWorkflowArgs = Parameters<typeof persistBatchStoryCreationDraft>[0];

export async function restoreSingleStoryCreationPersistenceWorkflow({
  setSnapshots,
  ...restoreArgs
}: RestoreSingleStoryCreationPersistenceWorkflowArgs): Promise<void> {
  await Promise.all([
    restoreSingleChapterStoryCreationDraft(restoreArgs),
    loadChapterStoryCreationSnapshots({
      storageKey: restoreArgs.storageKey,
      loadStoryCreationPersistence: restoreArgs.loadStoryCreationPersistence,
      setSnapshots,
      isCancelled: restoreArgs.isCancelled,
    }),
  ]);
}

export async function restoreBatchStoryCreationPersistenceWorkflow({
  setSnapshots,
  ...restoreArgs
}: RestoreBatchStoryCreationPersistenceWorkflowArgs): Promise<void> {
  await Promise.all([
    restoreBatchStoryCreationDraft(restoreArgs),
    loadChapterStoryCreationSnapshots({
      storageKey: restoreArgs.storageKey,
      loadStoryCreationPersistence: restoreArgs.loadStoryCreationPersistence,
      setSnapshots,
      isCancelled: restoreArgs.isCancelled,
    }),
  ]);
}

export function persistSingleStoryCreationDraftWorkflow(
  args: PersistSingleStoryCreationDraftWorkflowArgs,
): void {
  persistSingleChapterStoryCreationDraft(args);
}

export function persistBatchStoryCreationDraftWorkflow(
  args: PersistBatchStoryCreationDraftWorkflowArgs,
): void {
  persistBatchStoryCreationDraft(args);
}