import React, { useMemo, useEffect, useRef } from 'react';
import { Card, Tag, Badge, Empty, Collapse, theme } from 'antd';
import {
  FireOutlined,
  StarOutlined,
  ThunderboltOutlined,
  UserOutlined,
} from '@ant-design/icons';
import type { MemoryAnnotation } from './AnnotatedText';

const { Panel } = Collapse;

interface MemorySidebarProps {
  annotations: MemoryAnnotation[];
  activeAnnotationId?: string;
  onAnnotationClick?: (annotation: MemoryAnnotation) => void;
  scrollToAnnotation?: string;
}

// 类型配置
const TYPE_CONFIG = {
  hook: {
    label: '钩子',
    icon: <FireOutlined />,
  },
  foreshadow: {
    label: '伏笔',
    icon: <StarOutlined />,
  },
  plot_point: {
    label: '情节点',
    icon: <ThunderboltOutlined />,
  },
  character_event: {
    label: '角色事件',
    icon: <UserOutlined />,
  },
};

/**
 * 记忆侧边栏组件
 * 展示章节的所有记忆标注
 */
const MemorySidebar: React.FC<MemorySidebarProps> = ({
  annotations,
  activeAnnotationId,
  onAnnotationClick,
  scrollToAnnotation,
}) => {
  const { token } = theme.useToken();
  const cardRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const typeColors: Record<keyof typeof TYPE_CONFIG, string> = {
    hook: token.colorError,
    foreshadow: token.colorInfo,
    plot_point: token.colorSuccess,
    character_event: token.colorWarning,
  };

  // 当需要滚动到特定标注卡片时
  useEffect(() => {
    if (scrollToAnnotation && cardRefs.current[scrollToAnnotation]) {
      const element = cardRefs.current[scrollToAnnotation];
      element?.scrollIntoView({
        behavior: 'smooth',
        block: 'center',
      });
    }
  }, [scrollToAnnotation]);
  // 按类型分组
  const groupedAnnotations = useMemo(() => {
    const groups: Record<string, MemoryAnnotation[]> = {
      hook: [],
      foreshadow: [],
      plot_point: [],
      character_event: [],
    };

    annotations.forEach((annotation) => {
      if (groups[annotation.type]) {
        groups[annotation.type].push(annotation);
      }
    });

    // 每组按重要性排序
    Object.keys(groups).forEach((type) => {
      groups[type].sort((a, b) => b.importance - a.importance);
    });

    return groups;
  }, [annotations]);

  // 统计信息
  const stats = useMemo(() => {
    return {
      total: annotations.length,
      hooks: groupedAnnotations.hook.length,
      foreshadows: groupedAnnotations.foreshadow.length,
      plotPoints: groupedAnnotations.plot_point.length,
      characterEvents: groupedAnnotations.character_event.length,
    };
  }, [annotations, groupedAnnotations]);
  const activeAnnotation = useMemo(
    () => annotations.find((annotation) => annotation.id === activeAnnotationId) ?? null,
    [activeAnnotationId, annotations],
  );
  const memoryGuideSteps = [
    '先看钩子、伏笔、情节点和角色事件的分布，判断当前章节最值得优先回看的线索类型。',
    '再按重要性浏览卡片列表，把侧栏当作线索导航，而不是正文的替代阅读区。',
    '最后再点击具体记忆跳回正文，把分析回看和正文修订连接起来。',
  ];
  const memoryWorkspaceFocus = activeAnnotation
    ? {
        title: `优先回看当前选中的${TYPE_CONFIG[activeAnnotation.type].label}线索`,
        note: `当前焦点是“${activeAnnotation.title}”，更适合顺着这一条记忆回到正文，确认它和章节推进是否仍然一致。`,
      }
    : stats.foreshadows > stats.hooks
      ? {
          title: '先检查伏笔与情节点是否已经形成清晰的推进链路',
          note: '当前伏笔数量相对更高，适合优先回看铺垫与回收压力，再决定正文里哪些段落需要补强或收束。',
        }
      : {
          title: '先从高重要性的钩子和角色事件切入章节回看',
          note: '当前更适合优先浏览排序靠前的卡片，快速定位真正影响章节阅读张力的关键记忆。',
        };

  // 渲染单个记忆卡片
  const renderMemoryCard = (annotation: MemoryAnnotation) => {
    const config = TYPE_CONFIG[annotation.type];
    const color = typeColors[annotation.type];
    const isActive = activeAnnotationId === annotation.id;

    return (
      <div
        key={annotation.id}
        ref={(el) => {
          cardRefs.current[annotation.id] = el;
        }}
      >
        <Card
          size="small"
          hoverable
          onClick={() => onAnnotationClick?.(annotation)}
          style={{
            marginBottom: 14,
            borderLeft: `4px solid ${color}`,
            borderRadius: 18,
            border: `1px solid ${alphaColor(color, 0.18)}`,
            background: isActive
              ? `linear-gradient(135deg, ${alphaColor(color, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`
              : `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
            cursor: 'pointer',
            transition: 'all 0.2s',
            boxShadow: `0 14px 30px ${alphaColor(token.colorText, 0.05)}`,
          }}
          bodyStyle={{ padding: 14 }}
        >
        <div style={{ marginBottom: 8 }}>
          <Badge
            count={`${(annotation.importance * 10).toFixed(1)}`}
            style={{
              backgroundColor: color,
              float: 'right',
            }}
          />
          <div style={{ fontWeight: 600, fontSize: 14, paddingRight: 50 }}>
            {config.icon} {annotation.title}
          </div>
        </div>

        <div
          style={{
            fontSize: 13,
            color: token.colorTextSecondary,
            lineHeight: 1.6,
            marginBottom: 8,
          }}
        >
          {annotation.content.length > 100
            ? `${annotation.content.slice(0, 100)}...`
            : annotation.content}
        </div>

        {annotation.tags && annotation.tags.length > 0 && (
          <div>
            {annotation.tags.map((tag, index) => (
              <Tag key={index} style={{ fontSize: 11, margin: '2px 4px 2px 0' }}>
                {tag}
              </Tag>
            ))}
          </div>
        )}

        {/* 特殊元数据 */}
        {annotation.metadata.strength && (
          <div style={{ marginTop: 4, fontSize: 11, color: token.colorTextTertiary }}>
            强度: {annotation.metadata.strength}/10
          </div>
        )}
        {annotation.metadata.foreshadowType && (
          <Tag
            color={annotation.metadata.foreshadowType === 'planted' ? 'blue' : 'green'}
            style={{ marginTop: 4 }}
          >
            {annotation.metadata.foreshadowType === 'planted' ? '已埋下' : '已回收'}
          </Tag>
        )}
        </Card>
      </div>
    );
  };

  if (annotations.length === 0) {
    return (
      <div style={{ padding: 24 }}>
        <Empty description="暂无分析数据" />
      </div>
    );
  }

  return (
    <div style={{ height: '100%', overflowY: 'auto', padding: '16px' }}>
      <Card
        size="small"
        style={{
          marginBottom: 16,
          borderRadius: 24,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.94)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
          boxShadow: `0 18px 38px ${alphaColor(token.colorText, 0.08)}`,
        }}
        bodyStyle={{ padding: 18 }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <div style={{ fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Memory Atlas Guide
            </div>
            <div style={{ fontWeight: 700, marginBottom: 6, fontSize: 18, color: token.colorTextHeading }}>
              章节记忆侧栏
            </div>
            <div style={{ fontSize: 13, lineHeight: 1.7, color: token.colorTextSecondary, marginBottom: 12 }}>
              汇总当前章节里的钩子、伏笔、情节点与角色事件，帮助你从阅读与写作两侧快速回看关键线索。
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {memoryGuideSteps.map((item, index) => (
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
            <div style={{ fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              当前工作焦点
            </div>
            <div style={{ fontWeight: 700, marginBottom: 8, fontSize: 16, color: token.colorTextHeading }}>
              {memoryWorkspaceFocus.title}
            </div>
            <div style={{ fontSize: 13, lineHeight: 1.7, color: token.colorTextSecondary, marginBottom: 14 }}>
              {memoryWorkspaceFocus.note}
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <div>
                <div style={{ fontSize: 12, color: token.colorTextTertiary }}>钩子</div>
                <div style={{ fontSize: 20, fontWeight: 600, color: typeColors.hook }}>
                  {stats.hooks}
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: token.colorTextTertiary }}>伏笔</div>
                <div style={{ fontSize: 20, fontWeight: 600, color: typeColors.foreshadow }}>
                  {stats.foreshadows}
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: token.colorTextTertiary }}>情节点</div>
                <div style={{ fontSize: 20, fontWeight: 600, color: typeColors.plot_point }}>
                  {stats.plotPoints}
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: token.colorTextTertiary }}>角色事件</div>
                <div
                  style={{ fontSize: 20, fontWeight: 600, color: typeColors.character_event }}
                >
                  {stats.characterEvents}
                </div>
              </div>
            </div>
          </div>
        </div>
      </Card>

      <div
        style={{
          marginBottom: 16,
          padding: '12px 14px',
          borderRadius: 18,
          background: alphaColor(token.colorFillQuaternary, 0.88),
          color: token.colorTextSecondary,
          fontSize: 12,
          lineHeight: 1.7,
        }}
      >
        当前共收录 <strong style={{ color: token.colorTextHeading }}>{stats.total}</strong> 条分析记忆，按重要性排序；点击任一卡片会同步跳转到正文位置。
      </div>

      <Collapse defaultActiveKey={['hook', 'foreshadow', 'plot_point']} ghost>
        {Object.entries(groupedAnnotations).map(([type, items]) => {
          if (items.length === 0) return null;

          const config = TYPE_CONFIG[type as keyof typeof TYPE_CONFIG];

          return (
            <Panel
              key={type}
              header={
                <span style={{ fontWeight: 600, fontSize: 14 }}>
                  {config.icon} {config.label} ({items.length})
                </span>
              }
            >
              {items.map((annotation) => renderMemoryCard(annotation))}
            </Panel>
          );
        })}
      </Collapse>
    </div>
  );
};

export default MemorySidebar;
