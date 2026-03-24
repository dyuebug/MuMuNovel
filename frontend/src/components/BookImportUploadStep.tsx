import { Button, Card, Space, Tag, Upload } from 'antd';
import { InboxOutlined, PlayCircleOutlined } from '@ant-design/icons';
import type { UploadFile } from 'antd/es/upload/interface';

const { Dragger } = Upload;

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
  return (
    <Card title="上传 TXT 并开始解析" style={{ marginBottom: 16 }}>
      <Space direction="vertical" style={{ width: '100%' }} size={16}>
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
          style={{ padding: '8px 0' }}
        >
          <p className="ant-upload-drag-icon">
            <InboxOutlined />
          </p>
          <p className="ant-upload-text">点击或拖拽 TXT 文件到此区域</p>
          <p className="ant-upload-hint">首版仅支持 .txt，建议不超过 50MB</p>
        </Dragger>

        <Space>
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            loading={creatingTask}
            onClick={onStartTask}
          >
            开始解析
          </Button>
          {taskId ? <Tag color="blue">任务ID: {taskId}</Tag> : null}
        </Space>
      </Space>
    </Card>
  );
}
