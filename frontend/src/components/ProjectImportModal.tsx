import { Button, Card, Modal, Space, Spin, Tag, Typography, Upload } from 'antd';
import { UploadOutlined } from '@ant-design/icons';

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
  return (
    <Modal
      title="导入项目"
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={importing}
      okText="导入"
      cancelText="取消"
      width={isMobile ? '90%' : 500}
      centered
      okButtonProps={{ disabled: !validationResult?.valid }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <div>
          <p style={{ marginBottom: '12px', color: token.colorTextSecondary }}>
            {"选择之前导出的 JSON 格式项目文件"}
          </p>
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
            <Button icon={<UploadOutlined />} block>
              {"选择文件"}
            </Button>
          </Upload>
        </div>

        {validating ? (
          <div style={{ textAlign: 'center', padding: '20px' }}>
            <Spin tip="验证文件中..." />
          </div>
        ) : null}

        {validationResult ? (
          <Card size="small" style={{ background: validationResult.valid ? token.colorSuccessBg : token.colorErrorBg }}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <div>
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
