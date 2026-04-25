/**
 * Store Hooks - 项目状态同步工具
 * 为常用实体提供便捷 hooks，用于同步 API 数据与 store
 */

import { useCallback } from 'react';
import { useStore } from './index';
import { startChapterGenerationWorkflow } from './chapterGenerationWorkflow';
import { useBackgroundTaskStore } from './backgroundTasks';
import { refreshProjectsToStore } from './projectSyncHelpers';
import {
  loadProjectCollection,
  type RefreshCollectionOptions,
} from './projectCollectionRefresh';
export { isProjectCollectionFresh } from './projectCollectionRefresh';
export { syncProjectToStoreById } from './projectSyncHelpers';
import { outlineApi, chapterApi } from '../services/modularApi';
import { projectApi, characterApi } from '../services/modularApi';
import type {
  Outline,
  Character,
  Chapter,
  ProjectCreate,
  ProjectUpdate,
  OutlineCreate,
  OutlineUpdate,
  ChapterCreate,
  ChapterUpdate,
  CharacterUpdate,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
  GenerateOutlineRequest,
  GenerateCharacterRequest,
} from '../types';
import { normalizeStoreItems, runStoreMutation } from './storeMutationHelpers';
import { useEntityCrudSync } from './entityCrudSyncHooks';

type CharacterCreatePayload = Parameters<typeof characterApi.createCharacter>[0];

const characterRefreshPromises = new Map<string, Promise<Character[]>>();
const outlineRefreshPromises = new Map<string, Promise<Outline[]>>();
const chapterRefreshPromises = new Map<string, Promise<Chapter[]>>();

export async function loadProjectCharacters(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Character>({
    projectId,
    options,
    refreshPromises: characterRefreshPromises,
    collection: 'characters',
    request: (id) => characterApi.getCharacters(id),
    updateStore: (characters) => useStore.getState().setCharacters(characters),
    errorLogLabel: 'Failed to load characters:',
    errorMessage: 'Failed to load characters',
  });
}

export async function loadProjectOutlines(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Outline>({
    projectId,
    options,
    refreshPromises: outlineRefreshPromises,
    collection: 'outlines',
    request: (id) => outlineApi.getOutlines(id),
    updateStore: (outlines) => useStore.getState().setOutlines(outlines),
    errorLogLabel: 'Failed to load outlines:',
    errorMessage: 'Failed to load outlines',
  });
}

export async function loadProjectChapters(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Chapter>({
    projectId,
    options,
    refreshPromises: chapterRefreshPromises,
    collection: 'chapters',
    request: (id) => chapterApi.getChapters(id),
    updateStore: (chapters) => useStore.getState().setChapters(chapters),
    errorLogLabel: 'Failed to load chapters:',
    errorMessage: 'Failed to load chapters',
  });
}

/**
 * 项目同步 Hook
 */
export function useProjectSync() {
  const { addProject, updateProject, removeProject } = useStore();

  const refreshProjects = useCallback(async () => {
    return refreshProjectsToStore();
  }, []);

  const createProject = useCallback(async (data: ProjectCreate) => {
    return runStoreMutation({
      request: () => projectApi.createProject(data),
      onSuccess: addProject,
      errorLogLabel: 'Failed to create project:',
    });
  }, [addProject]);

  const updateProjectSync = useCallback(async (id: string, data: ProjectUpdate) => {
    return runStoreMutation({
      request: () => projectApi.updateProject(id, data),
      onSuccess: (updated) => updateProject(id, updated),
      errorLogLabel: 'Failed to update project:',
    });
  }, [updateProject]);

  const deleteProject = useCallback(async (id: string) => {
    return runStoreMutation({
      request: async () => {
        await projectApi.deleteProject(id);
        return id;
      },
      onSuccess: (deletedId) => {
        removeProject(deletedId);
        useBackgroundTaskStore.getState().removeTasksByProjectId(deletedId);
      },
      errorLogLabel: 'Failed to delete project:',
    });
  }, [removeProject]);

  return {
    refreshProjects,
    createProject,
    updateProject: updateProjectSync,
    deleteProject,
  };
}

/**
 * 角色同步 Hook
 */
export function useCharacterSync() {
  const addCharacter = useStore((state) => state.addCharacter);
  const updateCharacter = useStore((state) => state.updateCharacter);
  const removeCharacter = useStore((state) => state.removeCharacter);

  const {
    refreshItems: refreshCharacters,
    createItem: createCharacter,
    updateItem: updateCharacterSync,
    deleteItem: deleteCharacter,
  } = useEntityCrudSync<Character, CharacterCreatePayload, CharacterUpdate>({
    refresh: loadProjectCharacters,
    create: (data) => characterApi.createCharacter(data),
    update: (id, data) => characterApi.updateCharacter(id, data),
    remove: (id) => characterApi.deleteCharacter(id),
    addToStore: addCharacter,
    updateInStore: updateCharacter,
    removeFromStore: removeCharacter,
    createErrorLabel: 'Failed to create character:',
    updateErrorLabel: 'Failed to update character:',
    deleteErrorLabel: 'Failed to delete character:',
  });

  const generateCharacter = useCallback(async (data: GenerateCharacterRequest) => {
    return runStoreMutation({
      request: () => characterApi.generateCharacter(data),
      onSuccess: addCharacter,
      errorLogLabel: 'AI character generation failed:',
    });
  }, [addCharacter]);

  return {
    refreshCharacters,
    createCharacter,
    updateCharacter: updateCharacterSync,
    deleteCharacter,
    generateCharacter,
  };
}

/**
 * 大纲同步 Hook
 */
export function useOutlineSync() {
  const addOutline = useStore((state) => state.addOutline);
  const updateOutline = useStore((state) => state.updateOutline);
  const removeOutline = useStore((state) => state.removeOutline);

  const {
    refreshItems: refreshOutlines,
    createItem: createOutline,
    updateItem: updateOutlineSync,
    deleteItem: deleteOutline,
  } = useEntityCrudSync<Outline, OutlineCreate, OutlineUpdate>({
    refresh: loadProjectOutlines,
    create: (data) => outlineApi.createOutline(data),
    update: (id, data) => outlineApi.updateOutline(id, data),
    remove: (id) => outlineApi.deleteOutline(id),
    addToStore: addOutline,
    updateInStore: updateOutline,
    removeFromStore: removeOutline,
    createErrorLabel: 'Failed to create outline:',
    updateErrorLabel: 'Failed to update outline:',
    deleteErrorLabel: 'Failed to delete outline:',
  });

  const generateOutlines = useCallback(async (data: GenerateOutlineRequest) => {
    return runStoreMutation({
      request: async () => normalizeStoreItems<Outline>(await outlineApi.generateOutline(data)),
      onSuccess: (outlines) => {
        outlines.forEach((outline) => addOutline(outline));
      },
      errorLogLabel: 'AI outline generation failed:',
    });
  }, [addOutline]);

  return {
    refreshOutlines,
    createOutline,
    updateOutline: updateOutlineSync,
    deleteOutline,
    generateOutlines,
  };
}

/**
 * 章节同步 Hook
 */
export function useChapterSync() {
  const currentProject = useStore((state) => state.currentProject);
  const addChapter = useStore((state) => state.addChapter);
  const updateChapter = useStore((state) => state.updateChapter);
  const removeChapter = useStore((state) => state.removeChapter);

  const {
    refreshItems: refreshChapters,
    createItem: createChapter,
    updateItem: updateChapterSync,
    deleteItem: deleteChapter,
  } = useEntityCrudSync<Chapter, ChapterCreate, ChapterUpdate>({
    refresh: loadProjectChapters,
    create: (data) => chapterApi.createChapter(data),
    update: (id, data) => chapterApi.updateChapter(id, data),
    remove: (id) => chapterApi.deleteChapter(id),
    addToStore: addChapter,
    updateInStore: updateChapter,
    removeFromStore: removeChapter,
    createErrorLabel: 'Failed to create chapter:',
    updateErrorLabel: 'Failed to update chapter:',
    deleteErrorLabel: 'Failed to delete chapter:',
  });

  const generateChapterContentStream = useCallback(async (
    chapterId: string,
    onProgress?: (content: string) => void,
    styleId?: number,
    targetWordCount?: number,
    onProgressUpdate?: (message: string, progress: number) => void,
    model?: string,
    narrativePerspective?: string,
    creativeMode?: CreativeMode,
    storyFocus?: StoryFocus,
    plotStage?: PlotStage,
    storyCreationBrief?: string,
    qualityPreset?: QualityPreset,
    qualityNotes?: string,
    storyRepairSummary?: string,
    storyRepairTargets?: string[],
    storyPreserveStrengths?: string[],
  ) => {
    return startChapterGenerationWorkflow({
      chapterId,
      projectId: currentProject?.id,
      refreshChapters: () => refreshChapters(),
      onProgress,
      styleId,
      targetWordCount,
      onProgressUpdate,
      model,
      narrativePerspective,
      creativeMode,
      storyFocus,
      plotStage,
      storyCreationBrief,
      qualityPreset,
      qualityNotes,
      storyRepairSummary,
      storyRepairTargets,
      storyPreserveStrengths,
    });
  }, [refreshChapters, currentProject?.id]);

  return {
    refreshChapters,
    createChapter,
    updateChapter: updateChapterSync,
    deleteChapter,
    generateChapterContentStream,
  };
}
