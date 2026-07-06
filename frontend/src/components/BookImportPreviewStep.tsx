import { Alert, Button, Card, Col, Collapse, Empty, Input, InputNumber, Row, Select, Space, Tag, Typography, theme } from 'antd';
import type { Dispatch, SetStateAction } from 'react';
import type { BookImportPreview } from '../types';
import InlineDeferredPanel from './InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';

type BookImportPreviewStepProps = {
  applying: boolean;
  loadingPreview: boolean;
  preview: BookImportPreview | null;
  setPreview: Dispatch<SetStateAction<BookImportPreview | null>>;
  updateChapter: (index: number, patch: Partial<BookImportPreview['chapters'][number]>) => void;
  onApplyImport: () => void;
};

const { Text, Paragraph, Title } = Typography;
const { TextArea } = Input;

export default function BookImportPreviewStep({
  applying,
  loadingPreview,
  preview,
  setPreview,
  updateChapter,
  onApplyImport,
}: BookImportPreviewStepProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 30%, #1f262e 70%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 34%, ${token.colorBgContainer} 66%) 100%)`;
  const panelBorder = `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`;
  const guideSteps = [
    '先看导入预警和项目建议，确认这次拆书结果是否适合进入当前工作区。',
    '再修订章节标题、摘要和正文，保证真正写入前就把明显问题消化掉。',
    '最后再启动导入；原有 apply 导入逻辑、章节更新和状态流保持不变。',
  ];
  const warningCount = preview?.warnings.length ?? 0;
  const chapterCount = preview?.chapters.length ?? 0;
  const renderPreviewWorkspaceFallback = () => (
    <InlineDeferredPanel
      eyebrow="Preview Workspace"
      title={preview ? '恢复导入预览与章节校对工作区' : '生成拆书导入预览工作区'}
      message={preview
        ? '当前正在刷新项目建议、解析预警与章节预览内容。原有预览修订、章节内容更新和正式导入逻辑保持不变。'
        : '当前正在准备第一次导入预览，系统会按既有逻辑恢复项目建议、章节内容和写入前校对入口。'}
      minHeight={320}
      tags={[
        { label: preview ? `已识别 ${chapterCount} 章` : '等待预览生成', color: 'blue' },
        { label: warningCount > 0 ? `${warningCount} 条解析预警` : '解析状态同步中', color: warningCount > 0 ? 'gold' : 'processing' },
        { label: '写入前校对保持原样', color: 'default' },
      ]}
    />
  );

  return (
    <div style={{ marginBottom: 16 }}>
      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <Text style={{ color: 'rgba(255,255,255,0.68)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Preview Deck
        </Text>
        <Title
          level={5}
          style={{
            margin: '8px 0 10px',
            color: '#f7f1e8',
            fontFamily: designDisplayFont,
            letterSpacing: '-0.03em',
          }}
        >
          导入预览与写入前校对工作台
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这里像拆书导入的最终编辑台。原有预览修订、章节内容更新和正式导入逻辑保持不变，这里只把阅读顺序和当前焦点整理清楚。
        </Paragraph>
        <Space wrap size={[8, 8]} style={{ marginTop: 16 }}>
          <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {preview ? `已生成 ${chapterCount} 章预览` : '等待生成预览'}
          </Tag>
          <Tag color={warningCount > 0 ? 'gold' : 'green'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {warningCount > 0 ? `${warningCount} 条解析预警` : '暂无解析预警'}
          </Tag>
          <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            写入前可直接修订内容
          </Tag>
        </Space>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 8%, white 92%) 0%, color-mix(in srgb, ${token.colorWarning} 8%, white 92%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 14%, white 86%)`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Preview Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先判断这次解析结果是否可用，再微调项目建议和章节内容，最后再正式写入。这里只重排预览顺序，不改变原有导入动作。
        </Paragraph>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
          {guideSteps.map((item, index) => (
            <span
              key={item}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 8,
                padding: '6px 12px',
                borderRadius: 999,
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
                color: token.colorTextSecondary,
                fontSize: 12,
                lineHeight: 1.5,
              }}
            >
              <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
              {item}
            </span>
          ))}
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 24,
          border: panelBorder,
          background: quietPanelBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            gap: 12,
            flexWrap: 'wrap',
            alignItems: 'flex-start',
            marginBottom: 16,
          }}
        >
          <div>
            <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
              Preview Workspace
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
              当前导入预览工作区
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              先修正项目基础信息，再展开章节内容逐章校对。确认无误后再开始导入，原有 `apply` 提交流程保持不变。
            </Paragraph>
          </div>
          <Button type="primary" size="large" style={{ borderRadius: 14 }} loading={applying} disabled={!preview} onClick={onApplyImport}>
            开始导入
          </Button>
        </div>

        {loadingPreview ? (
          renderPreviewWorkspaceFallback()
        ) : !preview ? (
            <Card
              variant="borderless"
              style={{
                borderRadius: 22,
                minHeight: 280,
                background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.84)} 100%)`,
                border: `1px dashed ${alphaColor(token.colorBorder, 0.9)}`,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
              styles={{ body: { width: '100%' } }}
            >
              <Empty description="请先生成导入预览" />
            </Card>
          ) : (
            <div style={{ maxHeight: '60vh', overflowY: 'auto', paddingRight: 8 }}>
              <Space direction="vertical" style={{ width: '100%' }} size={16}>
                {preview.warnings.length > 0 ? (
                  <Alert
                    type="warning"
                    showIcon
                    message="解析警告"
                    description={
                      <ul style={{ margin: 0, paddingLeft: 20 }}>
                        {preview.warnings.map((warning, idx) => (
                          <li key={`${warning.code}-${idx}`}>[{warning.level}] {warning.message}</li>
                        ))}
                      </ul>
                    }
                  />
                ) : null}

                <Card
                  size="small"
                  title="项目建议"
                  style={{
                    borderRadius: 20,
                    border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                    background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
                  }}
                >
                  <Row gutter={12}>
                    <Col xs={24} md={12}>
                      <Text>标题</Text>
                      <Input
                        value={preview.project_suggestion.title}
                        onChange={(event) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: { ...prev.project_suggestion, title: event.target.value },
                          }) : prev)
                        }
                      />
                    </Col>
                    <Col xs={24} md={12}>
                      <Text>题材</Text>
                      <Input
                        value={preview.project_suggestion.genre}
                        onChange={(event) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: { ...prev.project_suggestion, genre: event.target.value },
                          }) : prev)
                        }
                      />
                    </Col>
                    <Col xs={24}>
                      <Text>主题</Text>
                      <TextArea
                        rows={3}
                        value={preview.project_suggestion.theme}
                        onChange={(event) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: { ...prev.project_suggestion, theme: event.target.value },
                          }) : prev)
                        }
                      />
                    </Col>
                    <Col xs={24}>
                      <Text>简介</Text>
                      <TextArea
                        rows={3}
                        value={preview.project_suggestion.description}
                        onChange={(event) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: { ...prev.project_suggestion, description: event.target.value },
                          }) : prev)
                        }
                      />
                    </Col>
                    <Col xs={24} md={12}>
                      <Text>叙事视角</Text>
                      <Select
                        style={{ width: '100%' }}
                        value={preview.project_suggestion.narrative_perspective}
                        onChange={(value) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: { ...prev.project_suggestion, narrative_perspective: value },
                          }) : prev)
                        }
                        options={[
                          { value: '第一人称', label: '第一人称' },
                          { value: '第三人称', label: '第三人称' },
                          { value: '全知视角', label: '全知视角' },
                        ]}
                      />
                    </Col>
                    <Col xs={24} md={12}>
                      <Text>目标字数</Text>
                      <InputNumber
                        style={{ width: '100%' }}
                        min={1000}
                        step={1000}
                        value={preview.project_suggestion.target_words}
                        onChange={(value) =>
                          setPreview((prev) => prev ? ({
                            ...prev,
                            project_suggestion: {
                              ...prev.project_suggestion,
                              target_words: Number(value || 100000),
                            },
                          }) : prev)
                        }
                      />
                    </Col>
                  </Row>
                </Card>

                <Card
                  size="small"
                  title={`章节预览（${preview.chapters.length}章）`}
                  style={{
                    borderRadius: 20,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                    background: alphaColor(token.colorBgElevated, 0.98),
                  }}
                >
                  <Collapse
                    items={preview.chapters.map((chapter, idx) => ({
                      key: String(idx),
                      label: `第${chapter.chapter_number}章 · ${chapter.title}`,
                      children: (
                        <Space direction="vertical" style={{ width: '100%' }}>
                          <Input
                            value={chapter.title}
                            addonBefore="标题"
                            onChange={(event) => updateChapter(idx, { title: event.target.value })}
                          />
                          <TextArea
                            rows={2}
                            value={chapter.summary}
                            placeholder="章节摘要"
                            onChange={(event) => updateChapter(idx, { summary: event.target.value })}
                          />
                          <TextArea
                            rows={8}
                            value={chapter.content}
                            placeholder="章节正文"
                            onChange={(event) => updateChapter(idx, { content: event.target.value })}
                          />
                        </Space>
                      ),
                    }))}
                  />
                </Card>
              </Space>
            </div>
          )}
        
      </Card>
    </div>
  );
}
