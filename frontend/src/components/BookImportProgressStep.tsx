import { Alert, Button, Card, List, Progress, Space, Spin, Tag, Typography } from 'antd';
import { RedoOutlined, WarningOutlined } from '@ant-design/icons';
import type { BookImportStepFailure } from '../types';

type BookImportProgressStepProps = {
  applyProgress: number;
  applyMessage: string;
  applyError: string | null;
  failedSteps: BookImportStepFailure[];
  isApplyComplete: boolean;
  retryProgress: number;
  retrying: boolean;
  retryMessage: string;
  onRetryFailedSteps: () => void;
  onSkipFailedSteps: () => void;
};

export default function BookImportProgressStep({
  applyProgress,
  applyMessage,
  applyError,
  failedSteps,
  isApplyComplete,
  retryProgress,
  retrying,
  retryMessage,
  onRetryFailedSteps,
  onSkipFailedSteps,
}: BookImportProgressStepProps) {
  return (
    <Card title="导入进度" style={{ textAlign: "center" }}>
      <Progress
        percent={Math.min(100, Math.max(0, Math.round(retrying ? retryProgress : applyProgress)))}
        status={
          applyError
            ? 'exception'
            : (failedSteps.length > 0 && isApplyComplete && !retrying)
              ? 'exception'
              : isApplyComplete && failedSteps.length === 0
                ? 'success'
                : 'active'
        }
        strokeColor={{
          '0%': 'var(--color-primary)',
          '100%': failedSteps.length > 0 ? '#faad14' : 'var(--color-primary-active)',
        }}
        style={{ marginBottom: 24 }}
      />

      <Typography.Paragraph
        style={{
          fontSize: 16,
          marginBottom: 32,
          color: applyError
            ? 'var(--color-error)'
            : (failedSteps.length > 0 && isApplyComplete && !retrying)
              ? '#faad14'
              : 'var(--color-text-secondary)',
        }}
      >
        {retrying ? retryMessage : (applyError || applyMessage)}
      </Typography.Paragraph>

      {applyError ? (
        <Alert
          type="error"
          message="导入失败"
          description={applyError}
          showIcon
          style={{ textAlign: 'left', marginBottom: 24 }}
        />
      ) : null}

      {failedSteps.length > 0 && isApplyComplete && !retrying ? (
        <div style={{ textAlign: 'left', marginBottom: 24 }}>
          <Alert
            type="warning"
            icon={<WarningOutlined />}
            showIcon
            message={`${failedSteps.length} 个步骤失败`}
            description={
              <div>
                <Typography.Paragraph style={{ marginBottom: 12, color: 'rgba(0,0,0,0.65)' }}>
                  以下步骤执行失败。你可以重试失败步骤，或跳过失败步骤继续完成导入。
                </Typography.Paragraph>
                <List
                  size="small"
                  bordered
                  dataSource={failedSteps}
                  renderItem={(item) => (
                    <List.Item style={{ padding: '8px 12px' }}>
                      <List.Item.Meta
                        title={
                          <Space>
                            <Tag color="error">{item.step_label}</Tag>
                            {(item.retry_count ?? 0) > 0 ? <Tag color="orange">已重试 {item.retry_count} 次</Tag> : null}
                          </Space>
                        }
                        description={
                          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                            {item.error.length > 120 ? `${item.error.slice(0, 120)}...` : item.error}
                          </Typography.Text>
                        }
                      />
                    </List.Item>
                  )}
                />
                <Space style={{ marginTop: 16, display: 'flex', justifyContent: 'center' }}>
                  <Button type="primary" icon={<RedoOutlined />} onClick={onRetryFailedSteps} loading={retrying}>
                    重试失败步骤
                  </Button>
                  <Button onClick={onSkipFailedSteps}>跳过失败步骤</Button>
                </Space>
              </div>
            }
            style={{ marginBottom: 16 }}
          />
        </div>
      ) : null}

      {retrying ? (
        <div style={{ marginBottom: 24 }}>
          <Spin spinning={retrying}>
            <Alert
              type="info"
              showIcon
              message="正在重试..."
              description={retryMessage}
              style={{ textAlign: 'left' }}
            />
          </Spin>
        </div>
      ) : null}

      {!failedSteps.length && !retrying ? (
        <div
          style={{
            background: 'var(--color-bg-layout)',
            padding: 16,
            borderRadius: 8,
            textAlign: 'left',
            marginTop: 32,
          }}
        >
          <Typography.Text type="secondary" style={{ fontSize: 13 }}>
            导入过程中系统会自动：<br />
            - 创建或更新项目基础信息<br />
            - 写入章节标题、摘要与正文<br />
            - 保留解析得到的结构与顺序<br />
            - 支持失败步骤单独重试<br />
            {isApplyComplete ? "导入已完成，可前往章节页继续校对与润色。" : "导入完成后，可前往章节页继续校对与润色。"}
          </Typography.Text>
        </div>
      ) : null}
    </Card>
  );
}
