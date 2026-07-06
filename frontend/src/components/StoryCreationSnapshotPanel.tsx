/* eslint-disable @typescript-eslint/no-explicit-any */
import { Button, Empty, Popconfirm, Space, Tag, Typography, theme } from 'antd';

import { buildStoryCreationSnapshotDiffLabels } from '../utils/storyCreationDraft';
import { renderCompactSelectionSummary, renderCompactStoryControlHeader } from './storyCreationCommonUi';

const { Text } = Typography;

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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const recentSnapshots = snapshots.slice(0, STORY_CREATION_SNAPSHOT_PREVIEW_LIMIT);
  const snapshotGuideSteps = [
    '先看最近一次保存的范围和差异标签，确认它对应的是当前单章还是批量创作上下文。',
    '再决定是直接应用、复制提示词，还是继续保留当前草稿，不要在没比对前贸然覆盖。',
    '把快照当作创作回退点和提示词参考，而不是新的业务状态入口。',
  ];
  const snapshotWorkspaceFocus = recentSnapshots.length > 0
    ? {
        title: `优先比较最近 ${recentSnapshots.length} 条快照和当前草稿的差异`,
        note: '当前已经有可回退版本，更适合先看差异项、字符数和保存层级，再决定是否应用或复制提示词。',
      }
    : {
        title: '先保存一个可回退的创作快照',
        note: '当前还没有历史版本，建议先在关键节点保留一个快照，方便后续回看提示词与参数变化。',
      };
  const scopeText = scopeLabel === 'single' ? '单章' : '批量';

  return (
    <div
      style={{
        padding: '10px 12px',
        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
        borderRadius: 16,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
      }}
    >
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
          gap: 16,
          marginBottom: 12,
          padding: 4,
        }}
      >
        <div>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Snapshot Guide
          </Text>
          <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
            创作快照工作区
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            这里负责保留创作过程中的可回退版本。当前只调整阅读顺序和焦点说明，不改变保存、应用、复制或删除的既有交互逻辑。
          </Text>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {snapshotGuideSteps.map((item, index) => (
              <span
                key={item}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '6px 12px',
                  borderRadius: 999,
                  background: token.colorBgContainer,
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                  color: token.colorText,
                  fontSize: 12,
                }}
              >
                <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                {item}
              </span>
            ))}
          </div>
        </div>
        <div
          style={{
            borderRadius: 18,
            padding: '16px 18px 14px',
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            当前工作焦点
          </Text>
          <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
            {snapshotWorkspaceFocus.title}
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            {snapshotWorkspaceFocus.note}
          </Text>
          <Space wrap>
            <Tag color="blue">范围: {scopeText}</Tag>
            <Tag color="purple">总快照: {snapshots.length}</Tag>
            <Tag color="cyan">预览: {recentSnapshots.length}</Tag>
            <Tag color={canSave ? 'green' : 'default'}>{canSave ? '可保存新快照' : '当前无新变更可保存'}</Tag>
          </Space>
        </div>
      </div>

      <div
        style={{
          padding: '12px 12px 10px',
          borderRadius: 14,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
          background: alphaColor(token.colorBgContainer, 0.9),
        }}
      >
        {renderCompactStoryControlHeader(
          '快照列表',
          recentSnapshots.length > 0
            ? `${scopeText}配置已保留最近版本，需要时可快速回退或复制当时提示词。`
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
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
                    borderRadius: 12,
                    background: alphaColor(token.colorFillAlter, 0.72),
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
    </div>
  );
}
