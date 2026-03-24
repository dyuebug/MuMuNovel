/* eslint-disable @typescript-eslint/no-explicit-any */
import { Button, Empty, Popconfirm, Space, Tag } from 'antd';

import { buildStoryCreationSnapshotDiffLabels } from '../utils/storyCreationDraft';
import { renderCompactSelectionSummary, renderCompactStoryControlHeader } from './storyCreationCommonUi';

type StoryCreationSnapshotPanelProps = {
  scopeLabel: 'single' | 'batch';
  emptyText: string;
  snapshots: any[];
  currentDraft: any;
  canSave: boolean;
  onSave: () => void;
  onApply: (snapshot: any) => void;
  onDelete: (snapshotId: string) => void;
  onCopy: (content: string | undefined, scopeLabel: 'single' | 'batch') => Promise<void>;
  includeNarrativePerspective?: boolean;
  promptWarnThreshold: number;
};

const STORY_CREATION_SNAPSHOT_PREVIEW_LIMIT = 5;

export default function StoryCreationSnapshotPanel({
  scopeLabel,
  emptyText,
  snapshots,
  currentDraft,
  canSave,
  onSave,
  onApply,
  onDelete,
  onCopy,
  includeNarrativePerspective = false,
  promptWarnThreshold,
}: StoryCreationSnapshotPanelProps) {
  const recentSnapshots = snapshots.slice(0, STORY_CREATION_SNAPSHOT_PREVIEW_LIMIT);

  return (
    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
      {renderCompactStoryControlHeader(
        '快照',
        recentSnapshots.length > 0
          ? `${scopeLabel === 'single' ? '单章' : '批量'}配置已保留最近版本，需要时可快速回退或复制当时提示词。`
          : '当前还没有可回退的配置版本。',
        {
          tagText: snapshots.length > 0 ? `共 ${snapshots.length} 条` : '尚无记录',
          tagColor: snapshots.length > 0 ? 'purple' : 'default',
          action: (
            <Button size="small" onClick={onSave} disabled={!canSave}>
              保存快照
            </Button>
          ),
          style: { marginBottom: 10 },
        },
      )}
      {recentSnapshots.length > 0 ? (
        <Space direction="vertical" size={8} style={{ display: 'flex' }}>
          {recentSnapshots.map((snapshot) => {
            const diffLabels = buildStoryCreationSnapshotDiffLabels(
              snapshot,
              currentDraft,
              includeNarrativePerspective,
            );

            return (
              <div
                key={snapshot.id}
                style={{
                  padding: '10px 12px',
                  border: '1px solid #f0f0f0',
                  borderRadius: 8,
                  background: '#fafafa',
                }}
              >
                {renderCompactStoryControlHeader(
                  snapshot.label,
                  new Date(snapshot.createdAt).toLocaleString(),
                  {
                    tagText: snapshot.reason === 'manual' ? '手动' : '自动',
                    tagColor: snapshot.reason === 'manual' ? 'green' : 'purple',
                    style: { marginBottom: 8 },
                  },
                )}
                {renderCompactSelectionSummary(
                  [
                    {
                      label: '字符',
                      value: `${snapshot.promptCharCount ?? 0}`,
                      color: (snapshot.promptCharCount ?? 0) >= promptWarnThreshold ? 'gold' : 'blue',
                    },
                    {
                      label: '提示词',
                      value: snapshot.prompt ? '已保存' : '仅参数',
                      color: snapshot.prompt ? 'cyan' : 'default',
                    },
                    ...(snapshot.promptLayerLabels?.length
                      ? [{ label: '层级', value: `${snapshot.promptLayerLabels.length} 项`, color: 'processing' }]
                      : []),
                    ...(diffLabels.length > 0
                      ? [{ label: '差异', value: `${diffLabels.length} 项`, color: 'orange' }]
                      : []),
                  ],
                  { style: { marginBottom: 8 } },
                )}
                {snapshot.promptLayerLabels?.length ? (
                  <Space wrap size={[6, 6]} style={{ marginBottom: 8 }}>
                    {snapshot.promptLayerLabels.map((item: string) => (
                      <Tag key={`${snapshot.id}-${item}`} color="processing">{item}</Tag>
                    ))}
                  </Space>
                ) : null}
                {diffLabels.length > 0 ? (
                  <Space wrap size={[6, 6]} style={{ marginBottom: 8 }}>
                    {diffLabels.map((item: string) => (
                      <Tag key={`${snapshot.id}-${item}`} color="orange">{item}</Tag>
                    ))}
                  </Space>
                ) : null}
                <Space.Compact>
                  <Button size="small" onClick={() => onApply(snapshot)}>
                    应用
                  </Button>
                  <Button
                    size="small"
                    disabled={!snapshot.prompt}
                    onClick={() => void onCopy(snapshot.prompt, scopeLabel)}
                  >
                    复制
                  </Button>
                  <Popconfirm
                    title="删除这个快照？"
                    okText="删除"
                    cancelText="取消"
                    onConfirm={() => onDelete(snapshot.id)}
                  >
                    <Button size="small" danger>
                      删除
                    </Button>
                  </Popconfirm>
                </Space.Compact>
              </div>
            );
          })}
        </Space>
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={emptyText} />
      )}
    </div>
  );
}
