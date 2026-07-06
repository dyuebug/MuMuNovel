import { memo } from 'react';
import { Empty, List, Space, Typography, theme } from 'antd';
import FloatingIndexGroupSection from './FloatingIndexGroupSection';
import type { FloatingIndexPanelResultsModel } from '../utils/floatingIndexPanelContracts';
import { FLOATING_INDEX_PANEL_EMPTY_DESCRIPTION } from '../utils/floatingIndexPanelViewHelpers';
import { designDisplayFont } from '../theme/themeConfig';

const { Text } = Typography;

type FloatingIndexPanelResultsProps = {
  resultsModel: FloatingIndexPanelResultsModel;
};

function FloatingIndexPanelResults({ resultsModel }: FloatingIndexPanelResultsProps) {
  const { filteredGroups, onChapterClick } = resultsModel;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const chapterCount = filteredGroups.reduce((sum, group) => sum + group.chapters.length, 0);

  return filteredGroups.length > 0 ? (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: 'flex',
        flexDirection: 'column',
        margin: '14px 16px 16px',
        padding: '14px 16px 16px',
        borderRadius: 20,
        border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
      }}
    >
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1fr) auto',
          gap: 12,
          alignItems: 'start',
          marginBottom: 12,
        }}
      >
        <div style={{ minWidth: 0 }}>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Result Dossier
          </Text>
          <Text
            strong
            style={{
              display: 'block',
              fontSize: 16,
              marginBottom: 6,
              fontFamily: designDisplayFont,
              letterSpacing: '-0.02em',
            }}
          >
            当前目录结果与可跳转章节
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
            先按分组判断是否已经足够聚焦，再进入具体章节。这里只整理结果阅读顺序，不改变筛选和跳转逻辑。
          </Text>
        </div>
        <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              borderRadius: 999,
              padding: '6px 12px',
              background: alphaColor(token.colorPrimary, 0.08),
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
              color: token.colorText,
              fontSize: 12,
            }}
          >
            分组 {filteredGroups.length}
          </span>
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              borderRadius: 999,
              padding: '6px 12px',
              background: alphaColor(token.colorInfo, 0.08),
              border: `1px solid ${alphaColor(token.colorInfo, 0.12)}`,
              color: token.colorText,
              fontSize: 12,
            }}
          >
            章节 {chapterCount}
          </span>
        </Space>
      </div>
      <List
        rowKey={(group) => group.key}
        dataSource={filteredGroups}
        renderItem={(group) => (
          <FloatingIndexGroupSection
            chapters={group.chapters}
            onChapterClick={onChapterClick}
            outlineLabel={group.outlineLabel}
            outlineTagColor={group.outlineTagColor}
          />
        )}
        style={{
          flex: 1,
          minHeight: 0,
          height: '100%',
          maxHeight: '100%',
          overflowY: 'auto',
          overflowX: 'hidden',
        }}
      />
    </div>
  ) : (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        margin: '14px 16px 16px',
        padding: '24px 20px',
        borderRadius: 20,
        border: `1px dashed ${alphaColor(token.colorBorder, 0.9)}`,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.38)} 100%)`,
      }}
    >
      <Empty description={FLOATING_INDEX_PANEL_EMPTY_DESCRIPTION} style={{ marginTop: 0 }} />
    </div>
  );
}

export default memo(FloatingIndexPanelResults);
