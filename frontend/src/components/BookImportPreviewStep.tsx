import { Alert, Button, Card, Col, Collapse, Empty, Input, InputNumber, Row, Select, Space, Spin, Typography } from 'antd';
import type { Dispatch, SetStateAction } from 'react';
import type { BookImportPreview } from '../types';

type BookImportPreviewStepProps = {
  applying: boolean;
  loadingPreview: boolean;
  preview: BookImportPreview | null;
  setPreview: Dispatch<SetStateAction<BookImportPreview | null>>;
  updateChapter: (index: number, patch: Partial<BookImportPreview['chapters'][number]>) => void;
  onApplyImport: () => void;
};

const { Text } = Typography;
const { TextArea } = Input;

export default function BookImportPreviewStep({
  applying,
  loadingPreview,
  preview,
  setPreview,
  updateChapter,
  onApplyImport,
}: BookImportPreviewStepProps) {
  return (
    <Card
      title="导入预览"
      extra={
        <Button type="primary" loading={applying} disabled={!preview} onClick={onApplyImport}>
          开始导入
        </Button>
      }
      style={{ marginBottom: 16 }}
    >
      <Spin spinning={loadingPreview}>
        {!preview ? (
          <Empty description="请先生成导入预览" />
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

              <Card size="small" title="项目建议">
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
                        { value: "第一人称", label: "第一人称" },
                        { value: "第三人称", label: "第三人称" },
                        { value: "全知视角", label: "全知视角" },
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

              <Card size="small" title={`章节预览（${preview.chapters.length}章）`}>
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
      </Spin>
    </Card>
  );
}
