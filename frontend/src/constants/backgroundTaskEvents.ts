export const OPEN_BACKGROUND_TASK_CENTER_EVENT = 'background-task-center:open';

let pendingBackgroundTaskCenterOpen = false;

export const requestBackgroundTaskCenterOpen = () => {
  pendingBackgroundTaskCenterOpen = true;
  window.dispatchEvent(new Event(OPEN_BACKGROUND_TASK_CENTER_EVENT));
};

export const consumePendingBackgroundTaskCenterOpen = (): boolean => {
  const pending = pendingBackgroundTaskCenterOpen;
  pendingBackgroundTaskCenterOpen = false;
  return pending;
};

export const CHAPTER_TASK_RESUMED_EVENT = 'chapter-task:resumed';
