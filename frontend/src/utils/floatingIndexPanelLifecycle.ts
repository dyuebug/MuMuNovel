export type FloatingIndexPanelVisibilitySetter = (visible: boolean) => void;

export function openFloatingIndexPanel({
  setIsIndexPanelVisible,
}: {
  setIsIndexPanelVisible: FloatingIndexPanelVisibilitySetter;
}): void {
  setIsIndexPanelVisible(true);
}

export function closeFloatingIndexPanel({
  setIsIndexPanelVisible,
}: {
  setIsIndexPanelVisible: FloatingIndexPanelVisibilitySetter;
}): void {
  setIsIndexPanelVisible(false);
}