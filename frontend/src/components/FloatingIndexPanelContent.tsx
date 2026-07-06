import { memo } from 'react';
import { Space, Tag, Typography, theme } from 'antd';
import FloatingIndexPanelResults from './FloatingIndexPanelResults';
import FloatingIndexPanelSearchHeader from './FloatingIndexPanelSearchHeader';
import type { FloatingIndexPanelViewModel } from '../utils/floatingIndexPanelContracts';

const { Text } = Typography;

type FloatingIndexPanelContentProps = {
  viewModel: FloatingIndexPanelViewModel;
};

function FloatingIndexPanelContent({ viewModel }: FloatingIndexPanelContentProps) {
  const { resultsModel, searchModel } = viewModel;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const groupCount = resultsModel.filteredGroups.length;
  const chapterCount = resultsModel.filteredGroups.reduce((sum, group) => sum + group.chapters.length, 0);
  const hasSearch = searchModel.searchTerm.trim().length > 0;
  const floatingIndexGuideSteps = [
    '先按大纲或搜索词缩小范围，不要一开始就在整本书里逐条翻找章节。',
    '再根据目录分组快速定位目标章节，把这个面板当作导航入口，而不是正文阅读区。',
    '最后在确认章节后直接跳转，把查找动作和写作动作清晰分开。',
  ];
  const floatingIndexWorkspaceFocus = hasSearch
    ? {
        title: '先确认当前搜索词是否已经把范围缩到可直接跳转的章节集合',
        note: '当前已经进入检索模式，更适合优先看筛出的目录组和章节数量，再决定是否继续细化关键词。',
      }
    : groupCount > 1
      ? {
          title: `先在这 ${groupCount} 组目录里锁定目标章节所在的大纲分区`,
          note: '当前仍是全量浏览模式，建议优先按分组快速缩小范围，再进入具体章节跳转。',
        }
      : {
          title: '先确认当前目录分组是否已经足够聚焦到目标章节',
          note: '当前可见范围已经较窄，更适合直接浏览章节条目并完成跳转，不必额外切换工作区。',
        };

  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        minHeight: 0,
      }}
    >
      <div
        style={{
          margin: '16px 16px 0',
          padding: 16,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.82)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Navigation Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              章节目录导览
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里负责在长篇项目里快速定位章节。当前只补充导航顺序和焦点说明，不改变搜索、分组和章节跳转逻辑。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {floatingIndexGuideSteps.map((item, index) => (
                <span
                  key={item}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    borderRadius: 999,
                    background: token.colorBgContainer,
                    border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                    color: token.colorText,
                    fontSize: 12,
                  }}
                >
                  <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                  {item}
                </span>
              ))}
            </div>
          </div>
          <div
            style={{
              borderRadius: 18,
              padding: '16px 18px 14px',
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              当前工作焦点
            </Text>
            <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
              {floatingIndexWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {floatingIndexWorkspaceFocus.note}
            </Text>
            <Space wrap>
              <Tag color="blue">目录组: {groupCount}</Tag>
              <Tag color="cyan">章节数: {chapterCount}</Tag>
              <Tag color={hasSearch ? 'green' : 'default'}>
                {hasSearch ? `搜索中: ${searchModel.searchTerm}` : '全量浏览'}
              </Tag>
            </Space>
          </div>
        </div>
      </div>
      <FloatingIndexPanelSearchHeader searchModel={searchModel} />
      <FloatingIndexPanelResults resultsModel={resultsModel} />
    </div>
  );
}

export default memo(FloatingIndexPanelContent);
