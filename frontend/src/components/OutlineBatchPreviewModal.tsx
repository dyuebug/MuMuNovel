import { useEffect, useState } from 'react';
import { Card, Empty, List, Modal, Space, Tag, Typography, theme } from 'antd';
import { CheckCircleOutlined } from '@ant-design/icons';
import type { BatchOutlineExpansionResponse, ChapterPlanItem, OutlineExpansionResponse } from '../types';

const { Text } = Typography;

interface SceneInfo {
  location: string;
  characters: string[];
  purpose: string;
}

interface OutlineBatchPreviewModalProps {
  visible: boolean;
  data: BatchOutlineExpansionResponse | null;
  onOk: () => void | Promise<void>;
  onCancel: () => void;
}

export default function OutlineBatchPreviewModal({
  visible,
  data,
  onOk,
  onCancel,
}: OutlineBatchPreviewModalProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const [selectedOutlineIdx, setSelectedOutlineIdx] = useState(0);
  const [selectedChapterIdx, setSelectedChapterIdx] = useState(0);

  useEffect(() => {
    if (visible) {
      setSelectedOutlineIdx(0);
      setSelectedChapterIdx(0);
    }
  }, [visible, data]);

  if (!visible || !data) {
    return null;
  }

  const selectedOutline = data.expansion_results[selectedOutlineIdx];
  const selectedChapter = selectedOutline?.chapter_plans[selectedChapterIdx];
  const totalChapterCount = data.expansion_results.reduce(
    (sum: number, result: OutlineExpansionResponse) => sum + result.actual_chapter_count,
    0,
  );

  const columnShellStyle = {
    borderRadius: 20,
    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
    background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
    padding: 14,
    minHeight: 0,
    overflow: 'hidden',
  } as const;

  const sectionLabelStyle = {
    fontSize: 11,
    letterSpacing: '0.08em',
    textTransform: 'uppercase' as const,
    color: token.colorTextTertiary,
    marginBottom: 6,
  };
  const outlineBatchPreviewGuideSteps = [
    '先从左侧大纲列表里锁定本轮真正要复核的展开结果，不必一开始就在多个大纲之间频繁来回切换。',
    '再在中间章节规划列里逐章抽查节奏、情绪和冲突方向，把最需要确认的章节优先看完。',
    '最后再到右侧细读摘要、目标、关键事件和场景，确认无误后再执行正式创建章节。',
  ];
  const outlineBatchPreviewWorkspaceFocus = data.skipped_outlines && data.skipped_outlines.length > 0
    ? {
        title: '先复核被跳过的大纲与当前预览范围',
        note: '当前批量展开里存在被跳过的大纲，适合先确认哪些结果已经可用、哪些条目需要后续单独处理，再决定是否直接创建章节。',
      }
    : !selectedOutline
      ? {
          title: '先挑选这一轮要检查的大纲',
          note: '当前还没有锁定具体的大纲结果，适合先从左侧列表选择一条展开结果，再继续进入章节级复核。',
        }
      : !selectedChapter
        ? {
            title: `先查看《${selectedOutline.outline_title}》的章节规划`,
            note: '当前已经进入目标大纲，但还没有锁定具体章节，适合先从中间列表挑出最关键的章节再看右侧细节。',
          }
        : {
            title: `复核《${selectedOutline.outline_title}》的第 ${selectedChapterIdx + 1} 章`,
            note: '当前已经定位到具体章节，适合优先检查剧情摘要、叙事目标、关键事件和场景信息是否支持正式创建章节。',
          };

  return (
    <Modal
      title={
        <div>
          <Space size={10} align="start">
            <div
              style={{
                width: 34,
                height: 34,
                borderRadius: 12,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.9)} 0%, ${alphaColor(token.colorPrimaryBg, 0.9)} 100%)`,
                border: `1px solid ${alphaColor(token.colorSuccess, 0.16)}`,
              }}
            >
              <CheckCircleOutlined style={{ color: token.colorSuccess }} />
            </div>
            <div>
              <Text style={sectionLabelStyle}>Batch Expansion Preview</Text>
              <Text strong style={{ display: 'block', fontSize: 18 }}>
                批量展开预览
              </Text>
              <Text type="secondary">
                先逐条检查大纲与章节规划，再决定是否正式创建章节。
              </Text>
            </div>
          </Space>
        </div>
      }
      open={visible}
      onOk={onOk}
      onCancel={onCancel}
      width={1200}
      centered
      okText="确认创建章节"
      cancelText="暂不创建"
      okButtonProps={{ danger: true }}
    >
      <div>
        <Card
          size="small"
          style={{
            marginBottom: 16,
            borderRadius: 22,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.82)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          }}
          styles={{ body: { padding: 16 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
              gap: 16,
            }}
          >
            <div>
              <Text style={sectionLabelStyle}>Preview Guide</Text>
              <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
                本轮批量展开工作台
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                这里不会改变原有批量展开结果，只是把预览顺序和确认重点提前说明，帮助你先筛选真正重要的大纲与章节，再决定是否正式创建。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {outlineBatchPreviewGuideSteps.map((item, index) => (
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
              <Text style={sectionLabelStyle}>当前工作焦点</Text>
              <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
                {outlineBatchPreviewWorkspaceFocus.title}
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                {outlineBatchPreviewWorkspaceFocus.note}
              </Text>
              <Space wrap size={[8, 8]}>
                <Tag color="blue">已展开大纲：{data.total_outlines_expanded} 条</Tag>
                <Tag color="green">计划创建章节：{totalChapterCount} 章</Tag>
                <Tag color="purple">
                  {selectedOutline ? `当前大纲：${selectedOutlineIdx + 1}` : '尚未选中大纲'}
                </Tag>
                <Tag color="orange">
                  {selectedChapter ? `当前章节：${selectedChapterIdx + 1}` : '尚未选中章节'}
                </Tag>
                {data.skipped_outlines && data.skipped_outlines.length > 0 ? (
                  <Tag color="warning">跳过：{data.skipped_outlines.length} 条</Tag>
                ) : (
                  <Tag color="success">全部结果可预览</Tag>
                )}
              </Space>
            </div>
          </div>
        </Card>

        {data.skipped_outlines && data.skipped_outlines.length > 0 ? (
          <Card
            size="small"
            style={{
              marginBottom: 16,
              borderRadius: 20,
              border: `1px solid ${alphaColor(token.colorWarning, 0.2)}`,
              background: `linear-gradient(135deg, ${alphaColor(token.colorWarningBg, 0.96)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            }}
            styles={{ body: { padding: 14 } }}
          >
            <Text style={sectionLabelStyle}>Skipped Outlines</Text>
            <Text strong style={{ display: 'block', marginBottom: 10, color: token.colorWarning }}>
              以下大纲已跳过
            </Text>
            <Space direction="vertical" size="small" style={{ width: '100%' }}>
              {data.skipped_outlines.map((skipped, idx: number) => (
                <div
                  key={idx}
                  style={{
                    padding: '10px 12px',
                    borderRadius: 14,
                    border: `1px solid ${alphaColor(token.colorWarning, 0.12)}`,
                    background: alphaColor(token.colorBgContainer, 0.78),
                    fontSize: 13,
                    color: token.colorTextSecondary,
                  }}
                >
                  {idx + 1}. {skipped.outline_title} <Tag color="default" style={{ fontSize: 11 }}>{skipped.reason}</Tag>
                </div>
              ))}
            </Space>
          </Card>
        ) : null}

        <div style={{ display: 'flex', gap: 16, height: 520 }}>
          <div
            style={{
              width: 280,
              ...columnShellStyle,
            }}
          >
            <Text style={sectionLabelStyle}>Outlines</Text>
            <Text strong style={{ display: 'block', marginBottom: 12 }}>
              大纲列表
            </Text>
            <div style={{ height: 436, overflowY: 'auto', paddingRight: 4 }}>
              <List
                size="small"
                dataSource={data.expansion_results}
                renderItem={(result: OutlineExpansionResponse, idx: number) => (
                  <List.Item
                    key={idx}
                    onClick={() => {
                      setSelectedOutlineIdx(idx);
                      setSelectedChapterIdx(0);
                    }}
                    style={{
                      cursor: 'pointer',
                      padding: 0,
                      border: 'none',
                      marginBottom: 8,
                    }}
                  >
                    <div
                      style={{
                        width: '100%',
                        padding: '12px 14px',
                        background: selectedOutlineIdx === idx
                          ? `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.96)} 100%)`
                          : alphaColor(token.colorBgContainer, 0.84),
                        borderRadius: 16,
                        border: `1px solid ${selectedOutlineIdx === idx ? alphaColor(token.colorPrimary, 0.2) : alphaColor(token.colorBorderSecondary, 0.84)}`,
                        boxShadow: selectedOutlineIdx === idx ? token.boxShadowTertiary : 'none',
                      }}
                    >
                      <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>
                        {idx + 1}. {result.outline_title}
                      </div>
                      <Space size={4} wrap>
                        <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>{result.expansion_strategy}</Tag>
                        <Tag color="green" style={{ fontSize: 11, margin: 0 }}>{result.actual_chapter_count} 章</Tag>
                      </Space>
                    </div>
                  </List.Item>
                )}
              />
            </div>
          </div>

          <div
            style={{
              width: 320,
              ...columnShellStyle,
            }}
          >
            <Text style={sectionLabelStyle}>Chapter Plans</Text>
            <Text strong style={{ display: 'block', marginBottom: 12 }}>
              章节规划（{selectedOutline?.actual_chapter_count || 0} 章）
            </Text>
            {selectedOutline ? (
              <div style={{ height: 436, overflowY: 'auto', paddingRight: 4 }}>
                <List
                  size="small"
                  dataSource={selectedOutline.chapter_plans}
                  renderItem={(plan: ChapterPlanItem, idx: number) => (
                    <List.Item
                      key={idx}
                      onClick={() => setSelectedChapterIdx(idx)}
                      style={{
                        cursor: 'pointer',
                        padding: 0,
                        border: 'none',
                        marginBottom: 8,
                      }}
                    >
                      <div
                        style={{
                          width: '100%',
                          padding: '12px 14px',
                          background: selectedChapterIdx === idx
                            ? `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgElevated, 0.96)} 100%)`
                            : alphaColor(token.colorBgContainer, 0.84),
                          borderRadius: 16,
                          border: `1px solid ${selectedChapterIdx === idx ? alphaColor(token.colorPrimary, 0.18) : alphaColor(token.colorBorderSecondary, 0.84)}`,
                          boxShadow: selectedChapterIdx === idx ? token.boxShadowSecondary : 'none',
                        }}
                      >
                        <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>
                          {idx + 1}. {plan.title}
                        </div>
                        <Space size={4} wrap>
                          <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>{plan.emotional_tone}</Tag>
                          <Tag color="orange" style={{ fontSize: 11, margin: 0 }}>{plan.conflict_type}</Tag>
                          <Tag color="green" style={{ fontSize: 11, margin: 0 }}>{plan.estimated_words} 字</Tag>
                        </Space>
                      </div>
                    </List.Item>
                  )}
                />
              </div>
            ) : null}
          </div>

          <div style={{ flex: 1, ...columnShellStyle }}>
            <Text style={sectionLabelStyle}>Chapter Details</Text>
            <Text strong style={{ display: 'block', marginBottom: 12 }}>
              章节详情
            </Text>
            {selectedChapter ? (
              <div style={{ height: 436, overflowY: 'auto', paddingRight: 4 }}>
                <Space direction="vertical" size="middle" style={{ width: '100%' }}>
                <Card
                  size="small"
                  title="剧情摘要"
                  style={{
                    borderRadius: 18,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    background: alphaColor(token.colorBgContainer, 0.96),
                  }}
                >
                  {selectedChapter.plot_summary}
                </Card>
                <Card
                  size="small"
                  title="叙事目标"
                  style={{
                    borderRadius: 18,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    background: alphaColor(token.colorBgContainer, 0.96),
                  }}
                >
                  {selectedChapter.narrative_goal}
                </Card>
                <Card
                  size="small"
                  title="关键事件"
                  style={{
                    borderRadius: 18,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    background: alphaColor(token.colorBgContainer, 0.96),
                  }}
                >
                  <Space direction="vertical" size="small" style={{ width: '100%' }}>
                    {(selectedChapter.key_events as string[]).map((event: string, eventIdx: number) => (
                      <div key={eventIdx}>• {event}</div>
                    ))}
                  </Space>
                </Card>
                <Card
                  size="small"
                  title="关注角色"
                  style={{
                    borderRadius: 18,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    background: alphaColor(token.colorBgContainer, 0.96),
                  }}
                >
                  <Space wrap>
                    {(selectedChapter.character_focus as string[]).map((character: string, characterIdx: number) => (
                      <Tag key={characterIdx} color="purple">{character}</Tag>
                    ))}
                  </Space>
                </Card>
                {selectedChapter.scenes && selectedChapter.scenes.length > 0 ? (
                  <Card
                    size="small"
                    title="场景列表"
                    style={{
                      borderRadius: 18,
                      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                      background: alphaColor(token.colorBgContainer, 0.96),
                    }}
                  >
                    <Space direction="vertical" size="small" style={{ width: '100%' }}>
                      {selectedChapter.scenes.map((scene, sceneIdx: number) => {
                        const currentScene = scene as SceneInfo;
                        return (
                          <Card
                            key={sceneIdx}
                            size="small"
                            style={{
                              borderRadius: 16,
                              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.8)}`,
                              background: alphaColor(token.colorFillQuaternary, 0.54),
                            }}
                          >
                            <div><strong>地点：</strong>{currentScene.location}</div>
                            <div><strong>角色：</strong>{currentScene.characters.join('、')}</div>
                            <div><strong>目的：</strong>{currentScene.purpose}</div>
                          </Card>
                        );
                      })}
                    </Space>
                  </Card>
                ) : null}
                </Space>
              </div>
            ) : (
              <div
                style={{
                  height: 436,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  borderRadius: 18,
                  border: `1px dashed ${alphaColor(token.colorBorder, 0.92)}`,
                  background: alphaColor(token.colorFillAlter, 0.65),
                }}
              >
                <Empty description="请选择一个章节查看详情" />
              </div>
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}
