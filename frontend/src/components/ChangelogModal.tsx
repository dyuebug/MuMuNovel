import { Modal, Timeline, Tag, Avatar, Empty, Button, Space, Typography, theme } from 'antd';
import { useState, useEffect, useRef, useCallback } from 'react';
import {
  BugOutlined,
  StarOutlined,
  FileTextOutlined,
  BgColorsOutlined,
  ThunderboltOutlined,
  ExperimentOutlined,
  ToolOutlined,
  QuestionCircleOutlined,
  GithubOutlined,
  ReloadOutlined,
  ClockCircleOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import {
  fetchChangelog,
  groupChangelogByDate,
  cacheChangelog,
  clearChangelogCache,
  type ChangelogEntry,
} from '../services/changelogService';
import InlineDeferredPanel from './InlineDeferredPanel';

interface ChangelogModalProps {
  visible: boolean;
  onClose: () => void;
}

const { Text, Title } = Typography;
const changelogGuideSteps = [
  '先看顶部焦点卡，确认这次适合快速扫读最近版本，还是处理加载失败后稍后再刷新。',
  '再按日期分组向下阅读，把更新日志当作版本线索，而不是逐条深挖所有提交细节。',
  '最后在需要时再打开具体提交链接，把这次版本变化带回到当前使用中的工作流页面。',
];

// 提交类型图标和颜色配置
const typeConfig: Record<ChangelogEntry['type'], { icon: React.ReactNode; color: string; label: string }> = {
  feature: { icon: <StarOutlined />, color: 'green', label: '新功能' },
  update: { icon: <SyncOutlined />, color: 'geekblue', label: '更新' },
  fix: { icon: <BugOutlined />, color: 'red', label: '修复' },
  docs: { icon: <FileTextOutlined />, color: 'blue', label: '文档' },
  style: { icon: <BgColorsOutlined />, color: 'purple', label: '样式' },
  refactor: { icon: <ThunderboltOutlined />, color: 'orange', label: '重构' },
  perf: { icon: <ThunderboltOutlined />, color: 'gold', label: '性能' },
  test: { icon: <ExperimentOutlined />, color: 'cyan', label: '测试' },
  chore: { icon: <ToolOutlined />, color: 'default', label: '杂项' },
  other: { icon: <QuestionCircleOutlined />, color: 'default', label: '其他' },
};

export default function ChangelogModal({ visible, onClose }: ChangelogModalProps) {
  const [changelog, setChangelog] = useState<ChangelogEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  // 加载更新日志
  // 每次用户打开窗口时才同步获取最新数据，不自动刷新
  const loadChangelog = useCallback(async (pageNum: number = 1, append: boolean = false) => {
    requestIdRef.current += 1;
    const requestId = requestIdRef.current;
    setLoading(true);
    setError(null);

    try {
      // 每次打开都从网络获取最新数据
      const entries = await fetchChangelog(pageNum, 30);
      if (!mountedRef.current || requestIdRef.current !== requestId) {
        return;
      }

      if (entries.length === 0) {
        setHasMore(false);
      } else {
        if (append) {
          setChangelog(prev => [...prev, ...entries]);
        } else {
          setChangelog(entries);
          // 缓存第一页数据（用于分页加载时的数据持久化）
          if (pageNum === 1) {
            cacheChangelog(entries);
          }
        }
      }
    } catch (err) {
      if (mountedRef.current && requestIdRef.current === requestId) {
        setError(err instanceof Error ? err.message : '获取更新日志失败');
      }
    } finally {
      if (mountedRef.current && requestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, []);

  // 初始加载
  useEffect(() => {
    if (visible) {
      setPage(1);
      setHasMore(true);
      void loadChangelog(1, false);
    } else {
      requestIdRef.current += 1;
    }
  }, [loadChangelog, visible]);

  // 加载更多
  const handleLoadMore = () => {
    const nextPage = page + 1;
    setPage(nextPage);
    void loadChangelog(nextPage, true);
  };

  // 刷新（清除缓存并重新加载）
  const handleRefresh = () => {
    clearChangelogCache();
    setPage(1);
    setHasMore(true);
    void loadChangelog(1, false);
  };

  // 按日期分组
  const groupedChangelog = groupChangelogByDate(changelog);
  const sortedDates = Array.from(groupedChangelog.keys()).sort((a, b) => b.localeCompare(a));
  const changelogWorkspaceFocus = error
    ? {
        title: '当前日志拉取失败，先判断是否需要立即刷新重试',
        note: '这时更适合把它当作一次临时网络失败，而不是功能异常；稍后刷新后再回来继续浏览版本变化。',
      }
    : loading && changelog.length === 0
      ? {
          title: '当前正在同步最新日志，适合先等待最近版本摘要加载完成',
          note: '日志窗口每次打开都会主动拉取最新 GitHub 提交，现在优先保持等待，不需要额外操作。',
        }
      : changelog.length === 0
        ? {
            title: '当前还没有可展示的更新日志，先把它当作空状态窗口',
            note: '如果后续有新版本发布，这里会按日期补齐版本记录；当前不用在这里停留过久。',
          }
        : hasMore
          ? {
              title: `当前已加载第 ${page} 页日志，先从最近 ${changelog.length} 条变化里抓主线`,
              note: '更适合优先看最近变更方向与类型分布，只有在需要更长时间线时再继续加载更多。',
            }
          : {
              title: '当前已经到达日志末尾，可以按日期回看完整版本轨迹',
              note: '这时更适合把时间线当作完整产品演进记录，按日期区块回看重点变化与提交说明。',
            };

  // 格式化日期
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));

    if (diffDays === 0) return '今天';
    if (diffDays === 1) return '昨天';
    if (diffDays < 7) return `${diffDays} 天前`;

    return date.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' });
  };

  // 格式化时间
  const formatTime = (dateStr: string) => {
    return new Date(dateStr).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
  };

  return (
    <Modal
      title={
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 16 }}>
          <Space direction="vertical" size={4}>
            <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Release Chronicle
            </Text>
            <Space size={8} align="center">
              <GithubOutlined style={{ color: token.colorPrimary }} />
              <Title level={4} style={{ margin: 0 }}>
                更新日志
              </Title>
            </Space>
            <Text type="secondary" style={{ fontSize: 13, lineHeight: 1.7 }}>
              以工作日志的方式整理产品迭代、修复与体验更新，帮助你快速了解最近的变化。
            </Text>
          </Space>

          <Button
            type="text"
            size="small"
            icon={<ReloadOutlined />}
            onClick={handleRefresh}
            loading={loading}
            title="刷新"
          >
            刷新
          </Button>
        </div>
      }
      open={visible}
      onCancel={onClose}
      footer={null}
      width={800}
      centered
      styles={{
        header: {
          borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          paddingBottom: 12,
        },
        body: {
          maxHeight: '70vh',
          overflowY: 'auto',
          padding: '24px',
        },
      }}
    >
      {error && (
        <div style={{
          padding: '16px',
          marginBottom: '16px',
          background: 'var(--color-error-bg)',
          border: '1px solid var(--color-error-border)',
          borderRadius: '4px',
          color: 'var(--color-error)',
        }}>
          {error}
        </div>
      )}

      <div
        style={{
          marginBottom: 24,
          padding: '18px 20px',
          borderRadius: 20,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          boxShadow: `0 16px 36px ${alphaColor(token.colorText, 0.08)}`,
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Changelog Guide
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px' }}>
              更新日志阅读顺序
            </Title>
            <Text style={{ display: 'block', color: token.colorTextSecondary, lineHeight: 1.75 }}>
              这里现在只增强阅读顺序和当前焦点提示，不改变日志拉取、缓存清理、分页加载或提交跳转逻辑。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
              {changelogGuideSteps.map((item, index) => (
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
              padding: '16px 18px',
              background: token.colorBgContainer,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              当前工作焦点
            </Text>
            <Title level={5} style={{ margin: '8px 0 6px' }}>
              {changelogWorkspaceFocus.title}
            </Title>
            <Text style={{ display: 'block', color: token.colorTextSecondary, lineHeight: 1.75 }}>
              {changelogWorkspaceFocus.note}
            </Text>
            <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
              <Tag color={error ? 'error' : loading ? 'processing' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {error ? '拉取失败' : loading ? '同步中' : `第 ${page} 页`}
              </Tag>
              <Tag color={changelog.length > 0 ? 'green' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                已载入 {changelog.length} 条
              </Tag>
              <Tag color={hasMore ? 'gold' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {hasMore ? '可继续加载更多' : '已到末尾'}
              </Tag>
            </Space>
          </div>
        </div>
      </div>

      {loading && changelog.length === 0 ? (
        <InlineDeferredPanel
          eyebrow="Changelog Review"
          title="正在同步最新更新日志"
          message="系统正在准备最近版本摘要、日期分组与提交入口，原有 GitHub 拉取、缓存与分页逻辑保持不变。"
          minHeight={260}
          tags={[
            { label: '更新日志同步中', color: 'processing' },
            { label: `当前页 ${page}`, color: 'blue' },
            { label: '阅读链路保持原样', color: 'green' },
          ]}
        />
      ) : changelog.length === 0 ? (
        <Empty description="暂无更新日志" />
      ) : (
        <>
          <div
            style={{
              marginBottom: 24,
              padding: '18px 20px',
              borderRadius: 20,
              background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.94)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
              boxShadow: `0 16px 36px ${alphaColor(token.colorText, 0.08)}`,
            }}
          >
            <Space size={[12, 12]} wrap>
              <Tag color="processing">当前页 {changelog.length}</Tag>
              <Tag color="blue">分页 {page}</Tag>
              {hasMore ? <Tag color="gold">可继续加载更多</Tag> : <Tag>已到达末尾</Tag>}
            </Space>
            <Text style={{ display: 'block', marginTop: 12, color: token.colorTextSecondary, lineHeight: 1.7 }}>
              这里优先展示最近更新，适合快速浏览版本走向；需要细看时可按日期区块向下阅读。
            </Text>
          </div>

          {sortedDates.map(date => {
            const entries = groupedChangelog.get(date) || [];

            return (
              <div
                key={date}
                style={{
                  marginBottom: '24px',
                  padding: '20px 20px 8px',
                  borderRadius: 22,
                  border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.82)}`,
                  background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
                  boxShadow: `0 14px 32px ${alphaColor(token.colorText, 0.06)}`,
                }}
              >
                <div style={{
                  fontSize: '16px',
                  fontWeight: 600,
                  color: token.colorTextHeading,
                  marginBottom: '16px',
                  paddingBottom: '8px',
                  borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
                }}>
                  <ClockCircleOutlined style={{ marginRight: '8px', color: token.colorPrimary }} />
                  {formatDate(date)}
                </div>

                <Timeline>
                  {entries.map(entry => {
                    const config = typeConfig[entry.type] || typeConfig.other;

                    return (
                      <Timeline.Item
                        key={entry.id}
                        dot={
                          <div style={{
                            width: '24px',
                            height: '24px',
                            borderRadius: '50%',
                            background: 'var(--color-bg-container)',
                            border: `2px solid ${config.color === 'default' ? 'var(--color-border)' : config.color}`,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            fontSize: '12px',
                          }}>
                            {config.icon}
                          </div>
                        }
                      >
                          <div style={{ marginLeft: '8px' }}>
                            <Space size="small" wrap>
                              <Tag color={config.color} icon={config.icon}>
                                {config.label}
                              </Tag>
                            {entry.scope && (
                              <Tag color="blue">{entry.scope}</Tag>
                            )}
                            <span style={{ color: 'var(--color-text-tertiary)', fontSize: '12px' }}>
                              {formatTime(entry.date)}
                            </span>
                          </Space>

                          <div style={{
                            marginTop: '8px',
                            fontSize: '14px',
                            lineHeight: '1.6',
                            color: token.colorText,
                          }}>
                            {entry.message}
                          </div>

                          <Space size="small" style={{ marginTop: '8px' }}>
                            {entry.author.avatar && (
                              <Avatar size="small" src={entry.author.avatar} />
                            )}
                            <span style={{ color: 'var(--color-text-secondary)', fontSize: '13px' }}>
                              {entry.author.username || entry.author.name}
                            </span>
                            <a
                              href={entry.commitUrl}
                              target="_blank"
                              rel="noopener noreferrer"
                              style={{ fontSize: '12px' }}
                            >
                              查看提交
                            </a>
                          </Space>
                        </div>
                      </Timeline.Item>
                    );
                  })}
                </Timeline>
              </div>
            );
          })}

          {
            hasMore && (
              <div style={{ textAlign: 'center', marginTop: '24px' }}>
                <Button
                  type="default"
                  onClick={handleLoadMore}
                  loading={loading}
                >
                  加载更多
                </Button>
              </div>
            )
          }

          {
            !hasMore && changelog.length > 0 && (
              <div style={{
                textAlign: 'center',
                color: token.colorTextTertiary,
                padding: '16px 0',
                fontSize: '14px',
              }}>
                已显示所有更新日志
              </div>
            )
          }
        </>
      )}

      <div style={{
        marginTop: '24px',
        padding: '14px 16px',
        background: `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.92)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        borderRadius: '16px',
        border: `1px solid ${alphaColor(token.colorInfo, 0.18)}`,
        fontSize: '13px',
        color: token.colorPrimary,
      }}>
        提示：每次打开窗口时都会主动同步最新日志，数据来源于 GitHub 提交历史。
      </div>
    </Modal >
  );
}
