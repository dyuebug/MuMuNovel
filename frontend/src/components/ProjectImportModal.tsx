import { Button, Card, Modal, Space, Tag, Typography, Upload } from 'antd';
import { UploadOutlined } from '@ant-design/icons';
import InlineDeferredPanel from './InlineDeferredPanel';

type ProjectImportStatistics = {
  chapters?: number;
  characters?: number;
  outlines?: number;
  relationships?: number;
  organizations?: number;
  careers?: number;
  character_careers?: number;
  writing_styles?: number;
  story_memories?: number;
  plot_analysis?: number;
  generation_history?: number;
  has_default_style?: boolean;
};

type ProjectImportValidationResult = {
  valid: boolean;
  project_name?: string;
  statistics?: ProjectImportStatistics;
  warnings?: string[];
  errors?: string[];
};

type ProjectImportModalToken = {
  colorTextSecondary: string;
  colorSuccessBg: string;
  colorErrorBg: string;
  colorSuccess: string;
  colorError: string;
  colorWarning: string;
};

type ProjectImportModalProps = {
  open: boolean;
  isMobile: boolean;
  importing: boolean;
  validating: boolean;
  selectedFile: File | null;
  validationResult: ProjectImportValidationResult | null;
  token: ProjectImportModalToken;
  onOk: () => void;
  onCancel: () => void;
  onFileSelect: (file: File) => boolean | Promise<boolean>;
  onRemoveFile: () => void;
};

const { Text } = Typography;

export default function ProjectImportModal({
  open,
  isMobile,
  importing,
  validating,
  selectedFile,
  validationResult,
  token,
  onOk,
  onCancel,
  onFileSelect,
  onRemoveFile,
}: ProjectImportModalProps) {
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const importGuideSteps = [
    '先选择项目归档文件，确认这次导入的是哪一个历史项目或迁移包。',
    '再看验证结果、项目名称和统计标签，先判断归档是否完整、是否值得进入当前工作区。',
    '最后在确认无误后再点击导入，把真正的写入动作放在阅读与校验之后。',
  ];
  const importWorkspaceFocus = importing
    ? {
        title: '等待项目写入当前工作区',
        note: '导入动作已经开始，适合先等待当前归档写入完成，不要重复关闭模态或再次触发导入请求。',
      }
    : validating
      ? {
          title: '检查归档结构与统计信息',
          note: '系统正在验证文件内容，适合先等待项目名称、统计标签和错误提示全部回流，再决定是否继续导入。',
        }
      : validationResult?.valid
        ? {
            title: `准备导入：${validationResult.project_name || '当前项目归档'}`,
            note: '当前归档已经通过验证，适合最后确认章节、角色与附属数据的统计范围，然后再把它写入当前工作区。',
          }
        : validationResult
          ? {
              title: '先修正当前归档问题',
              note: '验证结果已经指出结构或内容错误，适合先处理这些阻塞，再重新选择文件或回到导出源重新生成归档。',
            }
          : selectedFile
            ? {
                title: `等待“${selectedFile.name}”的验证结果`,
                note: '文件已经进入当前模态，下一步重点是看验证反馈，而不是立刻假设它可以安全导入。',
              }
            : {
                title: '选择本轮要恢复的项目归档',
                note: '当前还没有导入源，适合先挑选 JSON 归档文件，再逐步确认结构、统计和导入范围。',
              };

  return (
    <Modal
      title={(
        <Space direction="vertical" size={2}>
          <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextSecondary }}>
            Archive Intake
          </Text>
          <Text strong style={{ fontSize: 20 }}>
            导入项目
          </Text>
          <Text type="secondary" style={{ fontSize: 13, lineHeight: 1.7 }}>
            读取之前导出的项目归档文件，先验证结构与内容，再决定是否导入当前工作区。
          </Text>
        </Space>
      )}
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={importing}
      okText="导入"
      cancelText="取消"
      width={isMobile ? '90%' : 500}
      centered
      okButtonProps={{ disabled: !validationResult?.valid }}
      styles={{
        header: {
          borderBottom: `1px solid ${alphaColor(token.colorTextSecondary, 0.12)}`,
          paddingBottom: 10,
        },
        body: {
          paddingTop: 20,
        },
      }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <Card
          size="small"
          style={{
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorSuccess, 0.16)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.84)} 0%, white 100%)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.55fr) minmax(220px, 0.95fr)',
              gap: 14,
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <Text style={{ fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextSecondary }}>
                Import Guide
              </Text>
              <Text style={{ lineHeight: 1.7 }}>
                这个模态更像项目归档的入库检查台。现有的上传、校验、错误提示和导入确认流程都保持不变，这里只把导入顺序与当前判断重点提前说明。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {importGuideSteps.map((item, index) => (
                  <span
                    key={item}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 8,
                      padding: '6px 12px',
                      borderRadius: 999,
                      background: '#ffffff',
                      border: `1px solid ${alphaColor(token.colorTextSecondary, 0.12)}`,
                      fontSize: 12,
                    }}
                  >
                    <span style={{ color: token.colorSuccess, fontWeight: 700 }}>{index + 1}</span>
                    {item}
                  </span>
                ))}
              </div>
            </div>
            <div
              style={{
                borderRadius: 18,
                padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                background: '#ffffff',
                border: `1px solid ${alphaColor(token.colorTextSecondary, 0.12)}`,
              }}
            >
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextSecondary }}>
                当前工作焦点
              </Text>
              <Text strong style={{ display: 'block', margin: '8px 0 6px', fontSize: 16 }}>
                {importWorkspaceFocus.title}
              </Text>
              <Text type="secondary" style={{ lineHeight: 1.7 }}>
                {importWorkspaceFocus.note}
              </Text>
            </div>
          </div>
        </Card>

        <Card
          size="small"
          style={{
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorTextSecondary, 0.12)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.82)} 0%, white 100%)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <div>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextSecondary }}>
                Import Source
              </Text>
              <Text strong style={{ fontSize: 16 }}>
                选择 JSON 项目归档
              </Text>
            </div>
            <Text type="secondary" style={{ lineHeight: 1.7 }}>
              这里会先检查文件结构、项目名称和主要数据统计。只有验证通过后，才会放行真正的导入动作。
            </Text>
            <Upload
              accept=".json"
              beforeUpload={onFileSelect}
              maxCount={1}
              onRemove={() => {
                onRemoveFile();
              }}
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              fileList={selectedFile ? ([{ uid: '-1', name: selectedFile.name, status: 'done' }] as any) : []}
            >
              <Button icon={<UploadOutlined />} block size="large" style={{ borderRadius: 14 }}>
                {"选择文件"}
              </Button>
            </Upload>
          </Space>
        </Card>

        {validating ? (
          <InlineDeferredPanel
            eyebrow="Archive Validation"
            title="正在验证当前项目归档"
            message="系统正在检查文件结构、项目名称与主要统计信息，原有校验、报错与导入确认逻辑保持不变。"
            minHeight={220}
            tags={[
              { label: '归档验证中', color: 'processing' },
              { label: selectedFile ? selectedFile.name : '等待验证结果', color: 'blue' },
              { label: '导入逻辑保持原样', color: 'green' },
            ]}
          />
        ) : null}

        {validationResult ? (
          <Card
            size="small"
            style={{
              borderRadius: 22,
              border: `1px solid ${validationResult.valid ? alphaColor(token.colorSuccess, 0.18) : alphaColor(token.colorError, 0.2)}`,
              background: validationResult.valid
                ? `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.92)} 0%, white 100%)`
                : `linear-gradient(135deg, ${alphaColor(token.colorErrorBg, 0.92)} 0%, white 100%)`,
            }}
            styles={{ body: { padding: 18 } }}
          >
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <div>
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextSecondary, marginBottom: 4 }}>
                  Validation Report
                </Text>
                <Text strong style={{ color: validationResult.valid ? token.colorSuccess : token.colorError }}>
                  {validationResult.valid ? '✓ 文件验证通过' : '✗ 文件验证失败'}
                </Text>
              </div>
              {validationResult.project_name ? (
                <div>
                  <Text type="secondary">{"项目名称："}</Text>
                  <Text strong>{validationResult.project_name}</Text>
                </div>
              ) : null}
              {validationResult.statistics ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 6 }}>
                    {"数据统计："}
                  </Text>
                  <Space size={[6, 6]} wrap>
                    {validationResult.statistics.chapters ? <Tag color="blue">{"章节: "}{validationResult.statistics.chapters}</Tag> : null}
                    {validationResult.statistics.characters ? <Tag color="green">{"角色: "}{validationResult.statistics.characters}</Tag> : null}
                    {validationResult.statistics.outlines ? <Tag color="cyan">{"大纲: "}{validationResult.statistics.outlines}</Tag> : null}
                    {validationResult.statistics.relationships ? <Tag color="purple">{"关系: "}{validationResult.statistics.relationships}</Tag> : null}
                    {validationResult.statistics.organizations ? <Tag color="orange">{"组织: "}{validationResult.statistics.organizations}</Tag> : null}
                    {validationResult.statistics.careers ? <Tag color="magenta">{"职业: "}{validationResult.statistics.careers}</Tag> : null}
                    {validationResult.statistics.character_careers ? <Tag color="geekblue">{"职业关联: "}{validationResult.statistics.character_careers}</Tag> : null}
                    {validationResult.statistics.writing_styles ? <Tag color="lime">{"写作风格: "}{validationResult.statistics.writing_styles}</Tag> : null}
                    {validationResult.statistics.story_memories ? <Tag color="gold">{"故事记忆: "}{validationResult.statistics.story_memories}</Tag> : null}
                    {validationResult.statistics.plot_analysis ? <Tag color="volcano">{"剧情分析: "}{validationResult.statistics.plot_analysis}</Tag> : null}
                    {validationResult.statistics.generation_history ? <Tag>{"生成历史: "}{validationResult.statistics.generation_history}</Tag> : null}
                    {validationResult.statistics.has_default_style ? <Tag color="success">{"含默认风格"}</Tag> : null}
                  </Space>
                </div>
              ) : null}
              {validationResult.warnings?.length ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="warning" strong style={{ fontSize: 12 }}>
                    {"提示："}
                  </Text>
                  <ul style={{ margin: '4px 0 0 0', paddingLeft: 20, color: token.colorWarning, fontSize: 12 }}>
                    {validationResult.warnings.map((warning, index) => (
                      <li key={index}>{warning}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
              {validationResult.errors?.length ? (
                <div>
                  <Text type="danger" strong>
                    {"错误："}
                  </Text>
                  <ul style={{ margin: '4px 0 0 0', paddingLeft: 20, color: token.colorError, fontSize: 13 }}>
                    {validationResult.errors.map((error, index) => (
                      <li key={index}>{error}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
            </Space>
          </Card>
        ) : null}
      </Space>
    </Modal>
  );
}
