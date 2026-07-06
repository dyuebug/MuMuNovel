import { Button, Card, Space, Tag, Typography, Upload, theme } from 'antd';
import { InboxOutlined, PlayCircleOutlined } from '@ant-design/icons';
import type { UploadFile } from 'antd/es/upload/interface';
import { designDisplayFont } from '../theme/themeConfig';

const { Dragger } = Upload;
const { Text, Paragraph, Title } = Typography;

type BookImportUploadStepProps = {
  file: File | null;
  creatingTask: boolean;
  taskId: string | null;
  onFileSelect: (file: File) => void;
  onFileRemove: () => void;
  onStartTask: () => void;
};

export default function BookImportUploadStep({
  file,
  creatingTask,
  taskId,
  onFileSelect,
  onFileRemove,
  onStartTask,
}: BookImportUploadStepProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 30%, #1f262e 70%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 34%, ${token.colorBgContainer} 66%) 100%)`;
  const panelBorder = `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`;
  const intakeGuideSteps = [
    '先确认上传的是可直接拆章的 TXT 原文，再进入解析任务创建。',
    '再看文件名和任务状态摘要，避免在错误文本上浪费后续预览修订时间。',
    '最后再启动解析；原有文件选择、移除和任务创建逻辑保持不变。',
  ];

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
          Source Intake
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
          上传 TXT 并启动拆书解析
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这一步是拆书导入的素材入口。原有文件上传、任务创建和后续流转逻辑保持不变，这里只把上传前该确认的焦点整理得更明确。
        </Paragraph>
        <Space wrap size={[8, 8]} style={{ marginTop: 16 }}>
          <Tag color={file ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {file ? `已选择 ${file.name}` : '尚未选择 TXT'}
          </Tag>
          <Tag color={taskId ? 'cyan' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {taskId ? '已创建解析任务' : '等待创建任务'}
          </Tag>
          <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            首轮仅支持 `.txt`
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
          Intake Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先确认上传源，再看是否已经生成任务标识，最后才进入解析。这里只增强阅读顺序，不改变原有上传和启动解析动作。
        </Paragraph>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
          {intakeGuideSteps.map((item, index) => (
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
              Intake Workspace
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
              当前素材导入工作区
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              建议先用干净的章节文本完成首轮解析，后续再在预览阶段修订标题、摘要和正文。这里保留原有拖拽上传与任务创建交互。
            </Paragraph>
          </div>
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            loading={creatingTask}
            onClick={onStartTask}
            size="large"
            style={{ borderRadius: 14 }}
          >
            开始解析
          </Button>
        </div>

        <Space direction="vertical" style={{ width: '100%' }} size={16}>
          <div
            style={{
              padding: '14px 16px',
              borderRadius: 18,
              background: alphaColor(token.colorFillQuaternary, 0.82),
              color: token.colorTextSecondary,
              fontSize: 13,
              lineHeight: 1.7,
            }}
          >
            当前支持 `.txt` 文本导入。建议先用较干净的章节文本做首轮解析，后续再在预览阶段修订标题、摘要和章节内容。
          </div>

          <Dragger
            accept=".txt"
            multiple={false}
            beforeUpload={(selectedFile) => {
              onFileSelect(selectedFile);
              return false;
            }}
            onRemove={() => {
              onFileRemove();
            }}
            fileList={
              file
                ? [
                    {
                      uid: 'selected-txt',
                      name: file.name,
                      status: 'done',
                    } as UploadFile,
                  ]
                : []
            }
            style={{
              padding: '12px 0',
              borderRadius: 18,
              background: alphaColor(token.colorBgElevated, 0.92),
              border: `1px dashed ${alphaColor(token.colorPrimary, 0.28)}`,
            }}
          >
            <p className="ant-upload-drag-icon">
              <InboxOutlined />
            </p>
            <p className="ant-upload-text">点击或拖拽 TXT 文件到此区域</p>
            <p className="ant-upload-hint">首版仅支持 .txt，建议不超过 50MB</p>
          </Dragger>

          <Space wrap>
            {taskId ? <Tag color="blue">任务ID: {taskId}</Tag> : null}
          </Space>
        </Space>
      </Card>
    </div>
  );
}
