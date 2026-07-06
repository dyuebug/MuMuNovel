import { Suspense, lazy, memo } from 'react';
import { FloatButton } from 'antd';
import type { FloatingIndexPanelState } from '../utils/floatingIndexPanelState';
import type { FloatingIndexPanelTriggerProps } from '../utils/floatingIndexPanelTriggerProps';
import WorkflowEntryFallback from './WorkflowEntryFallback';

const LazyFloatingIndexPanel = lazy(() => import('./FloatingIndexPanel'));

type FloatingIndexPanelEntryProps = {
  floatingIndexPanelState: FloatingIndexPanelState | null;
  floatingIndexPanelTriggerProps: FloatingIndexPanelTriggerProps;
  onClose: () => void;
  onChapterSelect: (chapterId: string) => void;
};

function FloatingIndexPanelEntry({
  floatingIndexPanelState,
  floatingIndexPanelTriggerProps,
  onClose,
  onChapterSelect,
}: FloatingIndexPanelEntryProps) {
  return (
    <>
      <FloatButton {...floatingIndexPanelTriggerProps} />

      {floatingIndexPanelState ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Chapter Index"
              title="正在整理章节浮动索引面板"
              message="系统正在恢复章节分组索引、跳转入口与侧边浏览面板，原有触发按钮和章节跳转逻辑保持不变。"
              tags={[
                { label: '浮动索引', color: 'geekblue' },
                { label: '目录面板恢复中', color: 'processing' },
                { label: '跳转逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyFloatingIndexPanel
            visible={floatingIndexPanelState.visible}
            onClose={onClose}
            groupedChapters={floatingIndexPanelState.groupedChapters}
            onChapterSelect={onChapterSelect}
          />
        </Suspense>
      ) : null}
    </>
  );
}

export default memo(FloatingIndexPanelEntry);
