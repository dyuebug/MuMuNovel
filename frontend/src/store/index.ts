import { create } from 'zustand';
import type { Project, Outline, Character, Chapter } from '../types';

const areProjectsEqual = (left: Project | null, right: Project | null): boolean => {
  if (left === right) {
    return true;
  }

  if (!left || !right) {
    return left === right;
  }

  return (
    left.id === right.id
    && left.title === right.title
    && (left.description ?? '') === (right.description ?? '')
    && (left.theme ?? '') === (right.theme ?? '')
    && (left.genre ?? '') === (right.genre ?? '')
    && (left.target_words ?? null) === (right.target_words ?? null)
    && left.current_words === right.current_words
    && left.status === right.status
    && (left.wizard_status ?? '') === (right.wizard_status ?? '')
    && (left.wizard_step ?? null) === (right.wizard_step ?? null)
    && left.outline_mode === right.outline_mode
    && (left.world_time_period ?? '') === (right.world_time_period ?? '')
    && (left.world_location ?? '') === (right.world_location ?? '')
    && (left.world_atmosphere ?? '') === (right.world_atmosphere ?? '')
    && (left.world_rules ?? '') === (right.world_rules ?? '')
    && (left.chapter_count ?? null) === (right.chapter_count ?? null)
    && (left.narrative_perspective ?? '') === (right.narrative_perspective ?? '')
    && (left.character_count ?? null) === (right.character_count ?? null)
    && (left.default_creative_mode ?? '') === (right.default_creative_mode ?? '')
    && (left.default_story_focus ?? '') === (right.default_story_focus ?? '')
    && (left.default_plot_stage ?? '') === (right.default_plot_stage ?? '')
    && (left.default_story_creation_brief ?? '') === (right.default_story_creation_brief ?? '')
    && (left.default_quality_preset ?? '') === (right.default_quality_preset ?? '')
    && (left.default_quality_notes ?? '') === (right.default_quality_notes ?? '')
    && left.created_at === right.created_at
    && left.updated_at === right.updated_at
  );
};

const areOutlinesEqual = (left: Outline, right: Outline): boolean => (
  left.id === right.id
  && left.project_id === right.project_id
  && left.title === right.title
  && left.content === right.content
  && (left.structure ?? '') === (right.structure ?? '')
  && left.order_index === right.order_index
  && Boolean(left.has_chapters) === Boolean(right.has_chapters)
  && left.updated_at === right.updated_at
);

const areOutlineCollectionsEqual = (left: Outline[], right: Outline[]): boolean => (
  left.length === right.length
  && left.every((item, index) => areOutlinesEqual(item, right[index]))
);

const areChaptersEqual = (left: Chapter, right: Chapter): boolean => (
  left.id === right.id
  && left.project_id === right.project_id
  && left.title === right.title
  && (left.content ?? '') === (right.content ?? '')
  && (left.summary ?? '') === (right.summary ?? '')
  && left.chapter_number === right.chapter_number
  && left.word_count === right.word_count
  && left.status === right.status
  && (left.expansion_plan ?? '') === (right.expansion_plan ?? '')
  && (left.outline_id ?? '') === (right.outline_id ?? '')
  && (left.sub_index ?? null) === (right.sub_index ?? null)
  && (left.outline_title ?? '') === (right.outline_title ?? '')
  && (left.outline_order ?? null) === (right.outline_order ?? null)
  && left.updated_at === right.updated_at
);

const areChapterCollectionsEqual = (left: Chapter[], right: Chapter[]): boolean => (
  left.length === right.length
  && left.every((item, index) => areChaptersEqual(item, right[index]))
);

const mergeChapterCollectionsPreservingReferences = (
  previousChapters: Chapter[],
  nextChapters: Chapter[],
): Chapter[] => {
  if (areChapterCollectionsEqual(previousChapters, nextChapters)) {
    return previousChapters;
  }

  const previousById = new Map(previousChapters.map((chapter) => [chapter.id, chapter]));
  let hasReferenceChanges = previousChapters.length !== nextChapters.length;

  const mergedChapters = nextChapters.map((chapter, index) => {
    const previousChapter = previousById.get(chapter.id);
    if (previousChapter && areChaptersEqual(previousChapter, chapter)) {
      if (!hasReferenceChanges && previousChapters[index] !== previousChapter) {
        hasReferenceChanges = true;
      }
      return previousChapter;
    }

    hasReferenceChanges = true;
    return chapter;
  });

  return hasReferenceChanges ? mergedChapters : previousChapters;
};

interface AppState {
  currentProject: Project | null;
  setCurrentProject: (project: Project | null) => void;

  projects: Project[];
  setProjects: (projects: Project[]) => void;
  addProject: (project: Project) => void;
  updateProject: (id: string, project: Partial<Project>) => void;
  removeProject: (id: string) => void;

  outlines: Outline[];
  setOutlines: (outlines: Outline[]) => void;
  addOutline: (outline: Outline) => void;
  updateOutline: (id: string, outline: Partial<Outline>) => void;
  removeOutline: (id: string) => void;

  characters: Character[];
  setCharacters: (characters: Character[]) => void;
  addCharacter: (character: Character) => void;
  updateCharacter: (id: string, character: Partial<Character>) => void;
  removeCharacter: (id: string) => void;

  chapters: Chapter[];
  setChapters: (chapters: Chapter[]) => void;
  addChapter: (chapter: Chapter) => void;
  updateChapter: (id: string, chapter: Partial<Chapter>) => void;
  removeChapter: (id: string) => void;

  currentChapter: Chapter | null;
  setCurrentChapter: (chapter: Chapter | null) => void;

  loading: boolean;
  setLoading: (loading: boolean) => void;

  lastUpdated: {
    projects?: number;
    outlines?: number;
    characters?: number;
    chapters?: number;
  };
  markUpdated: (key: 'projects' | 'outlines' | 'characters' | 'chapters') => void;

  clearProjectData: () => void;
}

export const useStore = create<AppState>((set) => ({
  currentProject: null,
  setCurrentProject: (project) => set((state) => (
    areProjectsEqual(state.currentProject, project)
      ? state
      : { currentProject: project }
  )),

  projects: [],
  setProjects: (projects) => set({ projects }),
  addProject: (project) => set((state) => ({ 
    projects: [...state.projects, project] 
  })),
  updateProject: (id, updatedProject) => set((state) => ({
    projects: state.projects.map((p) => 
      p.id === id ? { ...p, ...updatedProject } : p
    ),
    currentProject: state.currentProject?.id === id 
      ? { ...state.currentProject, ...updatedProject } 
      : state.currentProject,
  })),
  removeProject: (id) => set((state) => ({
    projects: state.projects.filter((p) => p.id !== id),
    currentProject: state.currentProject?.id === id ? null : state.currentProject,
  })),

  outlines: [],
  setOutlines: (outlines) => set((state) => (
    areOutlineCollectionsEqual(state.outlines, outlines)
      ? state
      : { outlines }
  )),
  addOutline: (outline) => set((state) => ({ 
    outlines: [...state.outlines, outline] 
  })),
  updateOutline: (id, updatedOutline) => set((state) => ({
    outlines: state.outlines.map((o) => 
      o.id === id ? { ...o, ...updatedOutline } : o
    ),
  })),
  removeOutline: (id) => set((state) => ({
    outlines: state.outlines.filter((o) => o.id !== id),
  })),

  characters: [],
  setCharacters: (characters) => set({ characters }),
  addCharacter: (character) => set((state) => ({ 
    characters: [...state.characters, character] 
  })),
  updateCharacter: (id, updatedCharacter) => set((state) => ({
    characters: state.characters.map((c) => 
      c.id === id ? { ...c, ...updatedCharacter } : c
    ),
  })),
  removeCharacter: (id) => set((state) => ({
    characters: state.characters.filter((c) => c.id !== id),
  })),

  chapters: [],
  setChapters: (chapters) => set((state) => {
    const mergedChapters = mergeChapterCollectionsPreservingReferences(state.chapters, chapters);
    const nextCurrentChapter = state.currentChapter
      ? (mergedChapters.find((chapter) => chapter.id === state.currentChapter?.id) ?? null)
      : state.currentChapter;

    if (mergedChapters === state.chapters && nextCurrentChapter === state.currentChapter) {
      return state;
    }

    return {
      chapters: mergedChapters,
      currentChapter: nextCurrentChapter,
    };
  }),
  addChapter: (chapter) => set((state) => ({ 
    chapters: [...state.chapters, chapter] 
  })),
  updateChapter: (id, updatedChapter) => set((state) => ({
    chapters: state.chapters.map((c) => 
      c.id === id ? { ...c, ...updatedChapter } : c
    ),
    currentChapter: state.currentChapter?.id === id 
      ? { ...state.currentChapter, ...updatedChapter } 
      : state.currentChapter,
  })),
  removeChapter: (id) => set((state) => ({
    chapters: state.chapters.filter((c) => c.id !== id),
    currentChapter: state.currentChapter?.id === id ? null : state.currentChapter,
  })),

  currentChapter: null,
  setCurrentChapter: (chapter) => set({ currentChapter: chapter }),

  loading: false,
  setLoading: (loading) => set({ loading }),

  lastUpdated: {},
  markUpdated: (key) => set((state) => ({
    lastUpdated: {
      ...state.lastUpdated,
      [key]: Date.now(),
    },
  })),

  clearProjectData: () => set({
    outlines: [],
    characters: [],
    chapters: [],
    currentChapter: null,
  }),
}));
