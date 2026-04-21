import type { StoryCreationSnapshot } from '../utils/storyCreationDraft';
import {
  deleteChapterStoryCreationSnapshot,
  type LoadStoryCreationPersistence,
} from './chapterStoryCreationSnapshotHelpers';

export function deleteSingleStoryCreationSnapshotCallback({
  storageKey,
  snapshotId,
  loadStoryCreationPersistence,
  setSnapshots,
}: {
  storageKey: string | null;
  snapshotId: string;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
}): Promise<void> {
  return deleteChapterStoryCreationSnapshot({
    storageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots,
    successMessage: '\u5DF2\u5220\u9664\u521B\u4F5C\u5FEB\u7167',
  });
}

export function deleteBatchStoryCreationSnapshotCallback({
  storageKey,
  snapshotId,
  loadStoryCreationPersistence,
  setSnapshots,
}: {
  storageKey: string | null;
  snapshotId: string;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
}): Promise<void> {
  return deleteChapterStoryCreationSnapshot({
    storageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots,
    successMessage: '\u5DF2\u5220\u9664\u6279\u91CF\u5FEB\u7167',
  });
}