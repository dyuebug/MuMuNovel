import { Empty, Space, Tag } from 'antd';
import OutlineChapterPlanTabs, { type OutlinePlanItem } from './OutlineChapterPlanTabs';

type ExistingExpansionData = {
  chapter_count: number;
  expansion_plans: OutlinePlanItem[] | null;
};

type OutlineExistingExpansionContentProps = {
  data: ExistingExpansionData;
  isMobile: boolean;
  outlineTitle: string;
};

export default function OutlineExistingExpansionContent({
  data,
  isMobile,
  outlineTitle,
}: OutlineExistingExpansionContentProps) {
  const plans = data.expansion_plans ?? [];

  return (
    <div>
      <div style={{ marginBottom: 16 }}>
        <Space wrap style={{ maxWidth: '100%' }}>
          <Tag
            color="blue"
            style={{
              whiteSpace: 'normal',
              wordBreak: 'break-word',
              height: 'auto',
              lineHeight: '1.5',
              padding: '4px 8px',
            }}
          >
            大纲: {outlineTitle}
          </Tag>
          <Tag color="green">章节数: {data.chapter_count}</Tag>
          <Tag color="orange">已创建章节</Tag>
        </Space>
      </div>

      {plans.length > 0 ? (
        <OutlineChapterPlanTabs plans={plans} isMobile={isMobile} usePlanSubIndex />
      ) : (
        <Empty description="未找到展开规划数据" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      )}
    </div>
  );
}
