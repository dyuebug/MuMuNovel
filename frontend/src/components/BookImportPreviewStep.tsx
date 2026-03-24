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
      title="????"
      extra={
        <Button type="primary" loading={applying} disabled={!preview} onClick={onApplyImport}>
          ????
        </Button>
      }
      style={{ marginBottom: 16 }}
    >
      <Spin spinning={loadingPreview}>
        {!preview ? (
          <Empty description="????????????" />
        ) : (
          <div style={{ maxHeight: '60vh', overflowY: 'auto', paddingRight: 8 }}>
            <Space direction="vertical" style={{ width: '100%' }} size={16}>
              {preview.warnings.length > 0 ? (
                <Alert
                  type="warning"
                  showIcon
                  message="?????"
                  description={
                    <ul style={{ margin: 0, paddingLeft: 20 }}>
                      {preview.warnings.map((warning, idx) => (
                        <li key={`${warning.code}-${idx}`}>[{warning.level}] {warning.message}</li>
                      ))}
                    </ul>
                  }
                />
              ) : null}

              <Card size="small" title="????">
                <Row gutter={12}>
                  <Col xs={24} md={12}>
                    <Text>??</Text>
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
                    <Text>??</Text>
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
                    <Text>??</Text>
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
                    <Text>??</Text>
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
                    <Text>????</Text>
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
                        { value: '????', label: '????' },
                        { value: '????', label: '????' },
                        { value: '????', label: '????' },
                      ]}
                    />
                  </Col>
                  <Col xs={24} md={12}>
                    <Text>????</Text>
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

              <Card size="small" title={`???${preview.chapters.length}?`}>
                <Collapse
                  items={preview.chapters.map((chapter, idx) => ({
                    key: String(idx),
                    label: `? ${chapter.chapter_number} ? ? ${chapter.title}`,
                    children: (
                      <Space direction="vertical" style={{ width: '100%' }}>
                        <Input
                          value={chapter.title}
                          addonBefore="??"
                          onChange={(event) => updateChapter(idx, { title: event.target.value })}
                        />
                        <TextArea
                          rows={2}
                          value={chapter.summary}
                          placeholder="????"
                          onChange={(event) => updateChapter(idx, { summary: event.target.value })}
                        />
                        <TextArea
                          rows={8}
                          value={chapter.content}
                          placeholder="????"
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
