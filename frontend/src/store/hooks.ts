/**
 * Store Hooks - 项目状态同步工具
 * 为常用实体提供便捷 hooks，用于同步 API 数据与 store
 */

import { useChapterQueries } from '../features/chapters/queries';
import { useChapterCommands } from '../features/chapters/commands';
import { useChapterGenerationWorkflow } from '../features/chapters/workflows/generationWorkflow';
import { useCharacterQueries } from '../features/characters/queries';
import { useCharacterCommands } from '../features/characters/commands';
import { useOutlineQueries } from '../features/outlines/queries';
import { useOutlineCommands } from '../features/outlines/commands';
import { useProjectQueries } from '../features/projects/queries';
import { useProjectCommands } from '../features/projects/commands';

export { isProjectCollectionFresh } from './projectCollectionRefresh';
export { syncProjectToStoreById } from './projectSyncHelpers';

export { loadProjectCharacters } from '../features/characters/queries';
export { loadProjectOutlines } from '../features/outlines/queries';
export { loadProjectChapters } from '../features/chapters/queries';

export function useProjectSync() {
  const { refreshProjects } = useProjectQueries();
  const { createProject, updateProject, deleteProject } = useProjectCommands();

  return {
    refreshProjects,
    createProject,
    updateProject,
    deleteProject,
  };
}

export function useCharacterSync() {
  const { refreshCharacters } = useCharacterQueries();
  const { createCharacter, updateCharacter, deleteCharacter, generateCharacter } = useCharacterCommands();

  return {
    refreshCharacters,
    createCharacter,
    updateCharacter,
    deleteCharacter,
    generateCharacter,
  };
}

export function useOutlineSync() {
  const { refreshOutlines } = useOutlineQueries();
  const { createOutline, updateOutline, deleteOutline, generateOutlines } = useOutlineCommands();

  return {
    refreshOutlines,
    createOutline,
    updateOutline,
    deleteOutline,
    generateOutlines,
  };
}

export function useChapterSync() {
  const { refreshChapters } = useChapterQueries();
  const { createChapter, updateChapter, deleteChapter } = useChapterCommands();
  const { generateChapterContentStream } = useChapterGenerationWorkflow({
    refreshChapters: () => refreshChapters(),
  });

  return {
    refreshChapters,
    createChapter,
    updateChapter,
    deleteChapter,
    generateChapterContentStream,
  };
}
