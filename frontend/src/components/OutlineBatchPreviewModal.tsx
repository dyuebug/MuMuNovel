import { useEffect, useState } from 'react';
import { Card, Empty, List, Modal, Space, Tag } from 'antd';
import { CheckCircleOutlined } from '@ant-design/icons';
import type { BatchOutlineExpansionResponse, ChapterPlanItem, OutlineExpansionResponse } from '../types';

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

  return (
    <Modal
      title={
        <Space>
          <CheckCircleOutlined style={{ color: 'var(--color-success)' }} />
          <span>批量展开预览</span>
        </Space>
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
        <div style={{ marginBottom: 16 }}>
          <Tag color="blue">已展开大纲：{data.total_outlines_expanded} 条</Tag>
          <Tag color="green">
            计划创建章节：{data.expansion_results.reduce((sum: number, result: OutlineExpansionResponse) => sum + result.actual_chapter_count, 0)} 章
          </Tag>
          <Tag color="orange">待确认创建</Tag>
          {data.skipped_outlines && data.skipped_outlines.length > 0 ? (
            <Tag color="warning">跳过：{data.skipped_outlines.length} 条</Tag>
          ) : null}
        </div>

        {data.skipped_outlines && data.skipped_outlines.length > 0 ? (
          <div
            style={{
              marginBottom: 16,
              padding: 12,
              background: 'var(--color-warning-bg)',
              borderRadius: 4,
              border: '1px solid #ffe58f',
            }}
          >
            <div style={{ fontWeight: 500, marginBottom: 8, color: 'var(--color-warning)' }}>
              以下大纲已跳过
            </div>
            <Space direction="vertical" size="small" style={{ width: '100%' }}>
              {data.skipped_outlines.map((skipped, idx: number) => (
                <div key={idx} style={{ fontSize: 13, color: '#666' }}>
                  ? {skipped.outline_title} <Tag color="default" style={{ fontSize: 11 }}>{skipped.reason}</Tag>
                </div>
              ))}
            </Space>
          </div>
        ) : null}

        <div style={{ display: 'flex', gap: 16, height: 500 }}>
          <div
            style={{
              width: 280,
              borderRight: '1px solid #f0f0f0',
              paddingRight: 12,
              overflowY: 'auto',
            }}
          >
            <div style={{ fontWeight: 500, marginBottom: 8, color: '#666' }}>大纲列表</div>
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
                    padding: '8px 12px',
                    background: selectedOutlineIdx === idx ? '#e6f7ff' : 'transparent',
                    borderRadius: 4,
                    marginBottom: 4,
                    border: selectedOutlineIdx === idx ? '1px solid var(--color-primary)' : '1px solid transparent',
                  }}
                >
                  <div style={{ width: '100%' }}>
                    <div style={{ fontWeight: 500, fontSize: 13, marginBottom: 4 }}>
                      {idx + 1}. {result.outline_title}
                    </div>
                    <Space size={4}>
                      <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>{result.expansion_strategy}</Tag>
                      <Tag color="green" style={{ fontSize: 11, margin: 0 }}>{result.actual_chapter_count} ?</Tag>
                    </Space>
                  </div>
                </List.Item>
              )}
            />
          </div>

          <div
            style={{
              width: 320,
              borderRight: '1px solid #f0f0f0',
              paddingRight: 12,
              overflowY: 'auto',
            }}
          >
            <div style={{ fontWeight: 500, marginBottom: 8, color: '#666' }}>
              章节规划（{selectedOutline?.actual_chapter_count || 0} 章）
            </div>
            {selectedOutline ? (
              <List
                size="small"
                dataSource={selectedOutline.chapter_plans}
                renderItem={(plan: ChapterPlanItem, idx: number) => (
                  <List.Item
                    key={idx}
                    onClick={() => setSelectedChapterIdx(idx)}
                    style={{
                      cursor: 'pointer',
                      padding: '8px 12px',
                      background: selectedChapterIdx === idx ? '#e6f7ff' : 'transparent',
                      borderRadius: 4,
                      marginBottom: 4,
                      border: selectedChapterIdx === idx ? '1px solid var(--color-primary)' : '1px solid transparent',
                    }}
                  >
                    <div style={{ width: '100%' }}>
                      <div style={{ fontWeight: 500, fontSize: 13, marginBottom: 4 }}>
                        {idx + 1}. {plan.title}
                      </div>
                      <Space size={4} wrap>
                        <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>{plan.emotional_tone}</Tag>
                        <Tag color="orange" style={{ fontSize: 11, margin: 0 }}>{plan.conflict_type}</Tag>
                        <Tag color="green" style={{ fontSize: 11, margin: 0 }}>?{plan.estimated_words}?</Tag>
                      </Space>
                    </div>
                  </List.Item>
                )}
              />
            ) : null}
          </div>

          <div style={{ flex: 1, overflowY: 'auto', paddingLeft: 12 }}>
            <div style={{ fontWeight: 500, marginBottom: 12, color: '#666' }}>章节详情</div>
            {selectedChapter ? (
              <Space direction="vertical" size="middle" style={{ width: '100%' }}>
                <Card size="small" title="剧情摘要" bordered={false}>
                  {selectedChapter.plot_summary}
                </Card>
                <Card size="small" title="叙事目标" bordered={false}>
                  {selectedChapter.narrative_goal}
                </Card>
                <Card size="small" title="关键事件" bordered={false}>
                  <Space direction="vertical" size="small" style={{ width: '100%' }}>
                    {(selectedChapter.key_events as string[]).map((event: string, eventIdx: number) => (
                      <div key={eventIdx}>? {event}</div>
                    ))}
                  </Space>
                </Card>
                <Card size="small" title="关注角色" bordered={false}>
                  <Space wrap>
                    {(selectedChapter.character_focus as string[]).map((character: string, characterIdx: number) => (
                      <Tag key={characterIdx} color="purple">{character}</Tag>
                    ))}
                  </Space>
                </Card>
                {selectedChapter.scenes && selectedChapter.scenes.length > 0 ? (
                  <Card size="small" title="场景列表" bordered={false}>
                    <Space direction="vertical" size="small" style={{ width: '100%' }}>
                      {selectedChapter.scenes.map((scene, sceneIdx: number) => {
                        const currentScene = scene as SceneInfo;
                        return (
                          <Card key={sceneIdx} size="small" style={{ backgroundColor: '#fafafa' }}>
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
            ) : (
              <Empty description="请选择一个章节查看详情" />
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}
