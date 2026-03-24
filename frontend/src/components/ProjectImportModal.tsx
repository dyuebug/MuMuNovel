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
      title="\u5bfc\u5165\u9879\u76ee"
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={importing}
      okText="\u5bfc\u5165"
      cancelText="\u53d6\u6d88"
      width={isMobile ? '90%' : 500}
      centered
      okButtonProps={{ disabled: !validationResult?.valid }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <div>
          <p style={{ marginBottom: '12px', color: token.colorTextSecondary }}>
            {"\u9009\u62e9\u4e4b\u524d\u5bfc\u51fa\u7684 JSON \u683c\u5f0f\u9879\u76ee\u6587\u4ef6"}
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
              {"\u9009\u62e9\u6587\u4ef6"}
            </Button>
          </Upload>
        </div>

        {validating ? (
          <div style={{ textAlign: 'center', padding: '20px' }}>
            <Spin tip="\u9a8c\u8bc1\u6587\u4ef6\u4e2d..." />
          </div>
        ) : null}

        {validationResult ? (
          <Card size="small" style={{ background: validationResult.valid ? token.colorSuccessBg : token.colorErrorBg }}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <div>
                <Text strong style={{ color: validationResult.valid ? token.colorSuccess : token.colorError }}>
                  {validationResult.valid ? '\u2713 \u6587\u4ef6\u9a8c\u8bc1\u901a\u8fc7' : '\u2717 \u6587\u4ef6\u9a8c\u8bc1\u5931\u8d25'}
                </Text>
              </div>
              {validationResult.project_name ? (
                <div>
                  <Text type="secondary">{"\u9879\u76ee\u540d\u79f0\uff1a"}</Text>
                  <Text strong>{validationResult.project_name}</Text>
                </div>
              ) : null}
              {validationResult.statistics ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 6 }}>
                    {"\u6570\u636e\u7edf\u8ba1\uff1a"}
                  </Text>
                  <Space size={[6, 6]} wrap>
                    {validationResult.statistics.chapters ? <Tag color="blue">{"\u7ae0\u8282: "}{validationResult.statistics.chapters}</Tag> : null}
                    {validationResult.statistics.characters ? <Tag color="green">{"\u89d2\u8272: "}{validationResult.statistics.characters}</Tag> : null}
                    {validationResult.statistics.outlines ? <Tag color="cyan">{"\u5927\u7eb2: "}{validationResult.statistics.outlines}</Tag> : null}
                    {validationResult.statistics.relationships ? <Tag color="purple">{"\u5173\u7cfb: "}{validationResult.statistics.relationships}</Tag> : null}
                    {validationResult.statistics.organizations ? <Tag color="orange">{"\u7ec4\u7ec7: "}{validationResult.statistics.organizations}</Tag> : null}
                    {validationResult.statistics.careers ? <Tag color="magenta">{"\u804c\u4e1a: "}{validationResult.statistics.careers}</Tag> : null}
                    {validationResult.statistics.character_careers ? <Tag color="geekblue">{"\u804c\u4e1a\u5173\u8054: "}{validationResult.statistics.character_careers}</Tag> : null}
                    {validationResult.statistics.writing_styles ? <Tag color="lime">{"\u5199\u4f5c\u98ce\u683c: "}{validationResult.statistics.writing_styles}</Tag> : null}
                    {validationResult.statistics.story_memories ? <Tag color="gold">{"\u6545\u4e8b\u8bb0\u5fc6: "}{validationResult.statistics.story_memories}</Tag> : null}
                    {validationResult.statistics.plot_analysis ? <Tag color="volcano">{"\u5267\u60c5\u5206\u6790: "}{validationResult.statistics.plot_analysis}</Tag> : null}
                    {validationResult.statistics.generation_history ? <Tag>{"\u751f\u6210\u5386\u53f2: "}{validationResult.statistics.generation_history}</Tag> : null}
                    {validationResult.statistics.has_default_style ? <Tag color="success">{"\u542b\u9ed8\u8ba4\u98ce\u683c"}</Tag> : null}
                  </Space>
                </div>
              ) : null}
              {validationResult.warnings?.length ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="warning" strong style={{ fontSize: 12 }}>
                    {"\u63d0\u793a\uff1a"}
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
                    {"\u9519\u8bef\uff1a"}
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
