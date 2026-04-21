export {
  api,
  getAxiosErrorStatus,
  silentRequestConfig,
  type RequestConfigWithToastControl,
} from './core/httpClient';

export { authApi } from './modules/auth';
export { settingsApi } from './modules/settings';
export { userApi } from './modules/users';

export { projectApi } from './modules/projects';
export { characterApi } from './modules/characters';
export { writingStyleApi } from './modules/writingStyles';
export { mcpPluginApi } from './modules/mcpPlugins';
export { bookImportApi } from './modules/bookImport';
export { inspirationApi } from './modules/inspiration';
export { adminApi } from './modules/admin';
export { foreshadowApi } from './modules/foreshadows';
export { promptWorkshopApi } from './modules/promptWorkshop';
export { polishApi } from './modules/polish';
export { wizardStreamApi } from './modules/wizardStreams';
export { backgroundTaskApi, type BackgroundTaskListResponse, type BackgroundTaskStatus } from './modules/backgroundTasks';
export { chapterApi } from './modules/chapters';
export { outlineApi } from './modules/outlines';

export {
  getBatchManualReviewInfo,
  type ChapterBatchFailedChapter,
  type ChapterBatchManualReviewInfo,
} from './modules/chapterTaskState';
export { chapterBatchTaskApi } from './modules/chapterBatchTasks';
export { chapterSingleTaskApi } from './modules/chapterSingleTasks';
export {
  type ChapterBatchActiveResponse,
  type ChapterBatchActiveTask,
  type ChapterBatchCancelResponse,
  type ChapterBatchGenerateResponse,
  type ChapterBatchGenerateStatusResponse,
  type ChapterBatchResumeResponse,
  type ChapterSingleGenerateResponse,
} from './modules/chapterTaskTypes';
