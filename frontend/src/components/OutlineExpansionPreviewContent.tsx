import { Space, Tag } from 'antd';
import type { OutlineExpansionResponse } from '../types';
import OutlineChapterPlanTabs from './OutlineChapterPlanTabs';

type OutlineExpansionPreviewContentProps = {
  isMobile: boolean;
  response: OutlineExpansionResponse;
};

export default function OutlineExpansionPreviewContent({
  isMobile,
  response,
}: OutlineExpansionPreviewContentProps) {
  return (
    <div>
      <div style={{ marginBottom: 16 }}>
        <Space wrap>
          <Tag color="blue">策略: {response.expansion_strategy}</Tag>
          <Tag color="green">章节数: {response.actual_chapter_count}</Tag>
          <Tag color="orange">预览模式（未创建章节）</Tag>
        </Space>
      </div>
      <OutlineChapterPlanTabs plans={response.chapter_plans} isMobile={isMobile} />
    </div>
  );
}
