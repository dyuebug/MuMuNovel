import { message } from 'antd';
import { projectApi } from '../services/modularApi';
import type { ChapterQualityMetrics, ChapterUpdate, Project } from '../types';

export function closeChapterEditor({
  setChapterQualityMetrics,
  setIsEditorOpen,
}: {
  setChapterQualityMetrics: (metrics: ChapterQualityMetrics | null) => void;
  setIsEditorOpen: (open: boolean) => void;
}): void {
  setChapterQualityMetrics(null);
  setIsEditorOpen(false);
}

export async function submitChapterEditorUpdate({
  editingId,
  currentProjectId,
  values,
  updateChapter,
  setCurrentProject,
  closeEditor,
}: {
  editingId: string;
  currentProjectId: string;
  values: ChapterUpdate;
  updateChapter: (id: string, values: ChapterUpdate) => Promise<unknown>;
  setCurrentProject: (project: Project | null) => void;
  closeEditor: () => void;
}): Promise<void> {
  try {
    await updateChapter(editingId, values);
    const updatedProject = await projectApi.getProject(currentProjectId);
    setCurrentProject(updatedProject);
    message.success('Chapter updated successfully.');
    closeEditor();
  } catch {
    message.error('Failed to update chapter.');
  }
}

export async function submitChapterEditorWorkflow({
  editingId,
  currentProjectId,
  values,
  updateChapter,
  setCurrentProject,
  setChapterQualityMetrics,
  setIsEditorOpen,
}: {
  editingId: string | null;
  currentProjectId?: string;
  values: ChapterUpdate;
  updateChapter: (id: string, values: ChapterUpdate) => Promise<unknown>;
  setCurrentProject: (project: Project | null) => void;
  setChapterQualityMetrics: (metrics: ChapterQualityMetrics | null) => void;
  setIsEditorOpen: (open: boolean) => void;
}): Promise<void> {
  if (!editingId || !currentProjectId) {
    return;
  }

  await submitChapterEditorUpdate({
    editingId,
    currentProjectId,
    values,
    updateChapter,
    setCurrentProject,
    closeEditor: () => closeChapterEditor({
      setChapterQualityMetrics,
      setIsEditorOpen,
    }),
  });
}