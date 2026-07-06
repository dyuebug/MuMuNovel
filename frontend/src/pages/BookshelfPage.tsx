import { Card, Button, Space, Tag, Typography, Alert, Tooltip, theme } from 'antd';
import { BookOutlined, RocketOutlined, BulbOutlined, UploadOutlined, DownloadOutlined, LoadingOutlined, CalendarOutlined, DeleteOutlined, CheckCircleOutlined, EditOutlined, PauseCircleOutlined } from '@ant-design/icons';
import type { ReactNode } from 'react';
import type { Project } from '../types';
import { bookshelfCardStyles, bookshelfCardHoverHandlers } from '../components/CardStyles';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { useThemeMode } from '../theme/useThemeMode';
import { designDisplayFont } from '../theme/themeConfig';
import { VERSION_INFO } from '../config/version';
import { isProjectWizardIncomplete } from '../utils/projectWizardState';

const { Paragraph } = Typography;

interface BookshelfPageProps {
  isMobile: boolean;
  loading: boolean;
  projects: Project[];
  showApiTip: boolean;
  setShowApiTip: (show: boolean) => void;
  exportableProjectsCount: number;
  onOpenImportModal: () => void;
  onOpenExportModal: () => void;
  onGoSettings: () => void;
  onStartWizard: () => void;
  onOpenInspiration: () => void;
  onEnterProject: (project: Project) => void;
  onDeleteProject: (projectId: string) => void;
  formatWordCount: (count: number) => string;
  getProgress: (current: number, target: number) => number;
  getProgressColor: (progress: number) => string;
  getDisplayStatus: (status: string, progress: number) => string;
  getStatusTag: (status: string) => ReactNode;
  formatDate: (dateString: string) => string;
}

export default function BookshelfPage({
  isMobile,
  loading,
  projects,
  showApiTip,
  setShowApiTip,
  exportableProjectsCount,
  onOpenImportModal,
  onOpenExportModal,
  onGoSettings,
  onStartWizard,
  onOpenInspiration,
  onEnterProject,
  onDeleteProject,
  formatWordCount,
  getProgress,
  getProgressColor,
  getDisplayStatus,
  formatDate,
}: BookshelfPageProps) {
  const { token } = theme.useToken();
  const { resolvedMode } = useThemeMode();
  const isDark = resolvedMode === 'dark';
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const mobileBookHeight = 460;
  const desktopBookHeight = 430;
  const mobileSpineWidth = 32;
  const heroBackground = isDark
    ? 'linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, #cc785c 32%) 100%)'
    : 'linear-gradient(135deg, #1a1714 0%, color-mix(in srgb, #1a1714 70%, #cc785c 30%) 100%)';
  const heroMutedInk = alphaColor('#f7f1e8', 0.76);
  const wizardPendingCount = projects.filter((project) => isProjectWizardIncomplete(project)).length;
  const latestUpdatedProject = [...projects]
    .sort((left, right) => new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime())[0];
  const shelfGuideItems = [
    {
      label: 'Step 1',
      title: '先看入口书架',
      description: '把这里当成项目总览入口，先判断是继续旧项目、创建新项目，还是导入已有档案。',
    },
    {
      label: 'Step 2',
      title: '再挑创作路径',
      description: '核心项目适合直接进入书本，模糊想法则先去灵感模式再回到书架继续推进。',
    },
    {
      label: 'Step 3',
      title: '最后做归档操作',
      description: '导入导出更适合在项目结构稳定后再执行，避免创作中途频繁打断。',
    },
  ];
  const shelfFocusItems = [
    {
      label: '项目总数',
      value: `${projects.length} 本`,
      detail: '当前书架已收纳的项目数量',
    },
    {
      label: '待补向导',
      value: `${wizardPendingCount} 本`,
      detail: wizardPendingCount > 0 ? '这些项目可能还需要继续完成创建向导' : '当前没有未完成的创建向导',
    },
    {
      label: '最近更新',
      value: latestUpdatedProject?.title || '暂无',
      detail: latestUpdatedProject ? `更新于 ${formatDate(latestUpdatedProject.updated_at)}` : '书架里还没有项目记录',
    },
  ];
  const shelfWorkspaceFocus = loading
    ? {
        title: '恢复项目书架与阅读入口',
        note: '当前正在刷新项目目录、最近更新信息与书架陈列。先等待书本入口、导入导出和灵感跳转恢复，再决定本轮继续哪条创作路径。',
      }
    : projects.length === 0
      ? {
          title: '建立第一批书架项目',
          note: '当前书架还是空的，适合先创建第一个项目，或者导入已有档案后再回到书架统一整理。',
        }
      : {
          title: '从书架中挑选下一本要推进的作品',
          note: '当前书架已经有可继续推进的项目，适合先按更新时间和状态筛出本轮重点，再进入正文创作或继续补全向导。',
        };
  const renderBookshelfWorkspaceFallback = () => (
    <InlineDeferredPanel
      eyebrow="Bookshelf Workspace"
      title={shelfWorkspaceFocus.title}
      message={`${shelfWorkspaceFocus.note} 原有项目进入、删除、导入导出与灵感模式跳转逻辑保持不变。`}
      minHeight={isMobile ? mobileBookHeight + 60 : desktopBookHeight + 90}
      tags={[
        { label: `${projects.length} 本项目`, color: 'blue' },
        { label: '书架目录刷新中', color: 'processing' },
        { label: exportableProjectsCount > 0 ? `可导出 ${exportableProjectsCount} 本` : '等待导出清单', color: exportableProjectsCount > 0 ? 'success' : 'default' },
      ]}
    />
  );

  const serialBookPalettes = [
    {
      spine: `linear-gradient(180deg, color-mix(in srgb, ${token.colorSuccess} ${isDark ? 58 : 42}%, ${token.colorBgContainer} ${isDark ? 42 : 58}%) 0%, color-mix(in srgb, ${token.colorSuccess} ${isDark ? 74 : 56}%, ${token.colorText} ${isDark ? 26 : 44}%) 100%)`,
      spineBorder: `color-mix(in srgb, ${token.colorSuccess} ${isDark ? 66 : 52}%, ${token.colorBorder} ${isDark ? 34 : 48}%)`,
      ribbon: `color-mix(in srgb, ${token.colorSuccess} ${isDark ? 70 : 82}%, ${token.colorPrimary} ${isDark ? 30 : 18}%)`,
    },
    {
      spine: `linear-gradient(180deg, color-mix(in srgb, ${token.colorWarning} ${isDark ? 64 : 52}%, ${token.colorBgContainer} ${isDark ? 36 : 48}%) 0%, color-mix(in srgb, ${token.colorWarning} ${isDark ? 80 : 66}%, ${token.colorText} ${isDark ? 20 : 34}%) 100%)`,
      spineBorder: `color-mix(in srgb, ${token.colorWarning} ${isDark ? 70 : 56}%, ${token.colorBorder} ${isDark ? 30 : 44}%)`,
      ribbon: `color-mix(in srgb, ${token.colorWarning} ${isDark ? 72 : 82}%, ${token.colorPrimary} ${isDark ? 28 : 18}%)`,
    },
    {
      spine: `linear-gradient(180deg, color-mix(in srgb, ${token.colorInfo} ${isDark ? 46 : 30}%, ${token.colorText} ${isDark ? 54 : 70}%) 0%, color-mix(in srgb, ${token.colorText} ${isDark ? 66 : 52}%, ${token.colorBgContainer} ${isDark ? 34 : 48}%) 100%)`,
      spineBorder: `color-mix(in srgb, ${token.colorText} ${isDark ? 74 : 64}%, ${token.colorBorder} ${isDark ? 26 : 36}%)`,
      ribbon: `color-mix(in srgb, ${token.colorInfo} ${isDark ? 68 : 76}%, ${token.colorPrimary} ${isDark ? 32 : 24}%)`,
    },
    {
      spine: `linear-gradient(180deg, color-mix(in srgb, ${token.colorPrimary} ${isDark ? 62 : 50}%, ${token.colorBgContainer} ${isDark ? 38 : 50}%) 0%, color-mix(in srgb, ${token.colorPrimary} ${isDark ? 78 : 62}%, ${token.colorText} ${isDark ? 22 : 38}%) 100%)`,
      spineBorder: `color-mix(in srgb, ${token.colorPrimary} ${isDark ? 70 : 58}%, ${token.colorBorder} ${isDark ? 30 : 42}%)`,
      ribbon: `color-mix(in srgb, ${token.colorPrimary} ${isDark ? 74 : 86}%, ${token.colorInfo} ${isDark ? 26 : 14}%)`,
    },
  ];

  const completedBookPalette = {
    spine: `linear-gradient(180deg, color-mix(in srgb, ${token.colorPrimary} ${isDark ? 68 : 52}%, ${token.colorSuccess} ${isDark ? 32 : 48}%) 0%, color-mix(in srgb, ${token.colorPrimary} ${isDark ? 82 : 66}%, ${token.colorText} ${isDark ? 18 : 34}%) 100%)`,
    spineBorder: `color-mix(in srgb, ${token.colorPrimary} ${isDark ? 76 : 62}%, ${token.colorBorder} ${isDark ? 24 : 38}%)`,
    ribbon: `color-mix(in srgb, ${token.colorPrimary} ${isDark ? 62 : 48}%, ${token.colorError} ${isDark ? 38 : 52}%)`,
  };

  const getRibbonStatusIcon = (displayStatus: string, isWizardIncomplete: boolean, isCompleted: boolean) => {
    const commonStyle = { color: token.colorWhite, fontSize: isMobile ? 12 : 14 };

    if (isWizardIncomplete) {
      return <LoadingOutlined spin style={commonStyle} />;
    }
    if (isCompleted) {
      return <CheckCircleOutlined style={commonStyle} />;
    }
    if (displayStatus.includes('暂停') || displayStatus.includes('搁置')) {
      return <PauseCircleOutlined style={commonStyle} />;
    }
    if (displayStatus.includes('筹备') || displayStatus.includes('准备') || displayStatus.includes('大纲')) {
      return <BulbOutlined style={commonStyle} />;
    }

    return <EditOutlined style={commonStyle} />;
  };

  return (
    <div>
      <Card
        variant="borderless"
        style={{
          marginBottom: isMobile ? 12 : 16,
          borderRadius: isMobile ? 18 : 24,
          background: heroBackground,
          boxShadow: `0 24px 42px ${alphaColor(token.colorText, isDark ? 0.24 : 0.16)}`,
          border: `1px solid ${alphaColor('#f7f1e8', 0.1)}`,
          position: 'relative',
          overflow: 'hidden',
        }}
        styles={{ body: { padding: isMobile ? '16px 16px' : '20px 22px' } }}
      >
        <div style={{ position: 'absolute', top: -44, right: -44, width: 150, height: 150, borderRadius: '50%', background: '#f7f1e8', opacity: 0.08, pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -36, left: '26%', width: 100, height: 100, borderRadius: '50%', background: '#f7f1e8', opacity: 0.05, pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(90deg, rgba(255,255,255,0.04) 0%, transparent 22%, transparent 78%, rgba(255,255,255,0.03) 100%)', pointerEvents: 'none' }} />

        <div
          style={{
            position: 'relative',
            zIndex: 1,
            display: 'flex',
            flexDirection: isMobile ? 'column' : 'row',
            alignItems: isMobile ? 'stretch' : 'center',
            justifyContent: 'space-between',
            gap: isMobile ? 12 : 16,
          }}
        >
          <div>
            <div style={{
              fontSize: 11,
              letterSpacing: '0.18em',
              textTransform: 'uppercase',
              color: heroMutedInk,
              marginBottom: 8,
            }}>
              Library
            </div>
            <div
              style={{
                fontSize: isMobile ? 22 : 30,
                fontWeight: 600,
                color: '#f7f1e8',
                lineHeight: 1.3,
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.03em',
              }}
            >
              <BookOutlined style={{ opacity: 0.92 }} />
              我的书架
            </div>
            <div
              style={{
                marginTop: 6,
                fontSize: isMobile ? 12 : 14,
                color: heroMutedInk,
                maxWidth: 520,
                lineHeight: 1.7,
              }}
            >
              点击任意书本即可继续创作，也可以从这里统一查看进度、字数和最近更新时间。
            </div>
          </div>

          <Space size={8} wrap>
            <Button
              icon={<UploadOutlined />}
              onClick={onOpenImportModal}
              style={{
                borderRadius: 999,
                height: 40,
                paddingInline: 18,
                background: alphaColor('#ffffff', 0.08),
                borderColor: alphaColor('#f7f1e8', 0.12),
                color: '#f7f1e8',
              }}
            >
              导入项目
            </Button>
            <Button
              icon={<DownloadOutlined />}
              onClick={onOpenExportModal}
              disabled={exportableProjectsCount === 0}
              style={{
                borderRadius: 999,
                height: 40,
                paddingInline: 18,
              }}
            >
              导出项目
            </Button>
          </Space>
        </div>
      </Card>

      {showApiTip && projects.length === 0 && (
        <Alert
          message={`欢迎使用 ${VERSION_INFO.projectName}`}
          description={
            <div style={{
              display: 'flex',
              flexDirection: isMobile ? 'column' : 'row',
              alignItems: isMobile ? 'flex-start' : 'center',
              gap: isMobile ? 12 : 16,
              justifyContent: 'space-between'
            }}>
              <span style={{ fontSize: isMobile ? 12 : 14 }}>
                在开始创作之前，请先配置您的AI接口（支持 OpenAI / Anthropic）。
              </span>
              <Button
                size="small"
                type="primary"
                onClick={onGoSettings}
                style={{ flexShrink: 0 }}
              >
                去配置
              </Button>
            </div>
          }
          type="info"
          showIcon
          closable
          onClose={() => setShowApiTip(false)}
          style={{
            marginBottom: isMobile ? 16 : 24,
            borderRadius: 18,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            boxShadow: `0 18px 32px ${alphaColor(token.colorText, isDark ? 0.14 : 0.06)}`,
          }}
        />
      )}

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.15fr) minmax(300px, 0.9fr)',
          gap: isMobile ? 12 : 16,
          marginBottom: isMobile ? 14 : 18,
        }}
      >
        <Card
          variant="borderless"
          style={{
            borderRadius: isMobile ? 18 : 22,
            border: `1px solid ${alphaColor(token.colorText, isDark ? 0.1 : 0.06)}`,
            background: isDark
              ? `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.7)} 0%, ${alphaColor(token.colorBgLayout, 0.86)} 100%)`
              : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.94)} 0%, ${alphaColor(token.colorBgLayout, 0.98)} 100%)`,
            boxShadow: `0 18px 36px ${alphaColor(token.colorText, isDark ? 0.1 : 0.05)}`,
          }}
          styles={{ body: { padding: isMobile ? 14 : 18 } }}
        >
          <div style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Shelf Guide
          </div>
          <div style={{ margin: '8px 0 10px', fontSize: isMobile ? 22 : 24, fontWeight: 600, fontFamily: designDisplayFont, letterSpacing: '-0.03em', color: token.colorText }}>
            书架页阅读顺序
          </div>
          <Paragraph type="secondary" style={{ marginBottom: 14, lineHeight: 1.8 }}>
            这里更像创作项目的入口大厅。先确认你要继续哪本书，再决定是新建、导入，还是回到灵感工作流里补前置想法。
          </Paragraph>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: 10 }}>
            {shelfGuideItems.map((item) => (
              <div
                key={item.label}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  border: `1px solid ${token.colorBorderSecondary}`,
                  background: token.colorBgContainer,
                }}
              >
                <div style={{ fontSize: 11, color: token.colorTextTertiary, textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                  {item.label}
                </div>
                <div style={{ margin: '6px 0 4px', fontWeight: 600, color: token.colorText }}>
                  {item.title}
                </div>
                <div style={{ fontSize: 12, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                  {item.description}
                </div>
              </div>
            ))}
          </div>
        </Card>

        <Card
          variant="borderless"
          style={{
            borderRadius: isMobile ? 18 : 22,
            border: `1px solid ${alphaColor(token.colorText, isDark ? 0.1 : 0.06)}`,
            background: isDark
              ? `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.7)} 0%, ${alphaColor(token.colorBgLayout, 0.86)} 100%)`
              : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.94)} 0%, ${alphaColor(token.colorBgLayout, 0.98)} 100%)`,
            boxShadow: `0 18px 36px ${alphaColor(token.colorText, isDark ? 0.1 : 0.05)}`,
          }}
          styles={{ body: { padding: isMobile ? 14 : 18 } }}
        >
          <div style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Current Shelf
          </div>
          <div style={{ margin: '8px 0 10px', fontSize: isMobile ? 22 : 24, fontWeight: 600, fontFamily: designDisplayFont, letterSpacing: '-0.03em', color: token.colorText }}>
            当前书架焦点
          </div>
          <Space direction="vertical" size={10} style={{ width: '100%' }}>
            {shelfFocusItems.map((item) => (
              <div
                key={item.label}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <div style={{ marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                  {item.label}
                </div>
                <div style={{ fontWeight: 600, color: token.colorText, lineHeight: 1.7 }}>
                  {item.value}
                </div>
                <div style={{ fontSize: 12, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                  {item.detail}
                </div>
              </div>
            ))}
          </Space>
        </Card>
      </div>

      {loading ? (
        renderBookshelfWorkspaceFallback()
      ) : (
        <div style={{
          ...bookshelfCardStyles.container,
          borderRadius: isMobile ? 18 : 28,
          border: `1px solid ${isDark ? alphaColor(token.colorBorder, 0.42) : alphaColor(token.colorText, 0.08)}`,
          background: isDark
            ? `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.64)} 0%, ${alphaColor(token.colorBgLayout, 0.82)} 100%)`
            : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.84)} 0%, ${alphaColor(token.colorBgLayout, 0.92)} 100%)`,
          backgroundImage: `radial-gradient(${alphaColor(token.colorText, isDark ? 0.16 : 0.08)} 1px, transparent 0)`,
          backgroundSize: '18px 18px',
          boxShadow: `inset 0 1px 0 ${alphaColor(token.colorWhite, isDark ? 0.08 : 0.45)}, 0 24px 48px ${alphaColor(token.colorText, isDark ? 0.12 : 0.06)}`,
          padding: isMobile ? '14px' : '22px',
          ...(isMobile && {
            gridTemplateColumns: '1fr',
            gap: '14px',
          })
        }}>
          <div style={{ position: 'relative', width: '100%', minWidth: 0, minHeight: isMobile ? mobileBookHeight : desktopBookHeight }}>
            <Card
              hoverable
              style={{ ...bookshelfCardStyles.newProjectCard, minHeight: isMobile ? mobileBookHeight : desktopBookHeight }}
              styles={{ body: { padding: 0, flex: 1, display: 'flex', flexDirection: 'column' } }}
              {...bookshelfCardHoverHandlers}
              data-type="new-project"
              data-card-style="bookshelf-book"
              data-book-kind="new"
            >
              <div style={{
                position: 'absolute',
                inset: 0,
                background: `linear-gradient(180deg, ${alphaColor(isDark ? token.colorBgContainer : token.colorWhite, isDark ? 0.14 : 0.42)} 0%, transparent 22%, transparent 78%, ${alphaColor(token.colorWarning, isDark ? 0.1 : 0.05)} 100%)`,
                pointerEvents: 'none'
              }} />
              <div style={{
                width: '100%',
                minHeight: isMobile ? mobileBookHeight : desktopBookHeight,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                gap: isMobile ? 18 : 22,
                padding: isMobile ? '24px 18px 64px' : '34px 28px 72px',
                position: 'relative',
                zIndex: 1,
              }}>
                <div style={{
                  position: 'absolute',
                  top: isMobile ? 18 : 22,
                  left: isMobile ? 18 : 22,
                  fontSize: 11,
                  letterSpacing: '0.18em',
                  textTransform: 'uppercase',
                  color: token.colorTextTertiary,
                }}>
                  Entry Point
                </div>
                <div style={{
                  width: isMobile ? 56 : 68,
                  height: isMobile ? 56 : 68,
                  borderRadius: '50%',
                  display: 'grid',
                  placeItems: 'center',
                  background: `linear-gradient(180deg, ${alphaColor(token.colorPrimary, isDark ? 0.18 : 0.12)} 0%, ${alphaColor(token.colorPrimary, isDark ? 0.28 : 0.2)} 100%)`,
                  color: token.colorPrimary,
                  boxShadow: `inset 0 1px 0 ${alphaColor(token.colorWhite, isDark ? 0.18 : 0.75)}`,
                }}>
                  <BookOutlined style={{ fontSize: isMobile ? 24 : 28 }} />
                </div>

                <div style={{ width: '100%', maxWidth: 240, display: 'flex', flexDirection: 'column', gap: isMobile ? 10 : 14 }}>
                  <Button
                    type="primary"
                    size={isMobile ? 'middle' : 'large'}
                    icon={<RocketOutlined />}
                    onClick={onStartWizard}
                    style={{
                      height: isMobile ? 42 : 52,
                      fontSize: isMobile ? 14 : 16,
                      borderRadius: 10,
                      boxShadow: `0 10px 18px ${alphaColor(token.colorPrimary, isDark ? 0.14 : 0.22)}`,
                    }}
                    block
                  >
                    快速开始
                  </Button>
                  <Button
                    size={isMobile ? 'middle' : 'large'}
                    icon={<BulbOutlined />}
                    onClick={onOpenInspiration}
                    style={{
                      height: isMobile ? 42 : 52,
                      fontSize: isMobile ? '14px' : '16px',
                      borderRadius: 10,
                      borderColor: alphaColor(token.colorWarning, isDark ? 0.34 : 0.5),
                      color: `color-mix(in srgb, ${token.colorWarning} ${isDark ? 78 : 72}%, ${token.colorText} ${isDark ? 22 : 28}%)`,
                      background: `linear-gradient(180deg, ${alphaColor(token.colorWarning, isDark ? 0.12 : 0.12)} 0%, ${alphaColor(token.colorWarning, isDark ? 0.2 : 0.2)} 100%)`,
                    }}
                    block
                  >
                    灵感模式
                  </Button>
                </div>

                <div style={{
                  position: 'absolute',
                  bottom: isMobile ? 24 : 28,
                  left: 24,
                  right: 24,
                  textAlign: 'center',
                  fontSize: isMobile ? 11 : 12,
                  color: token.colorTextTertiary,
                  letterSpacing: 0.5,
                  textTransform: 'uppercase',
                }}>
                  开始一个新的创作旅程
                </div>
              </div>
            </Card>
          </div>

          {Array.isArray(projects) && projects.map((project, index) => {
            const progress = getProgress(project.current_words || 0, project.target_words || 0);
            const progressColor = getProgressColor(progress);
            const isWizardIncomplete = isProjectWizardIncomplete(project);
            const displayStatus = getDisplayStatus(project.status, progress);
            const isCompleted = progress >= 100 || displayStatus.includes('完结');
            const palette = isCompleted ? completedBookPalette : serialBookPalettes[index % serialBookPalettes.length];
            const tags = project.genre ? project.genre.split(/[,、，]/).map((t: string) => t.trim()).filter((t: string) => t) : [];

            const ribbonStatusIcon = getRibbonStatusIcon(displayStatus, isWizardIncomplete, isCompleted);

            return (
              <div key={project.id} style={{ position: 'relative', width: '100%', minWidth: 0, minHeight: isMobile ? mobileBookHeight : desktopBookHeight }}>
                <Card
                  hoverable
                  style={{ ...bookshelfCardStyles.projectCard, minHeight: isMobile ? mobileBookHeight : desktopBookHeight }}
                  styles={{ body: { padding: 0, flex: 1, display: 'flex', flexDirection: 'column' } }}
                  {...bookshelfCardHoverHandlers}
                  onClick={() => onEnterProject(project)}
                  data-card-style="bookshelf-book"
                  data-book-kind="project"
                >
                  <div style={{
                    position: 'absolute',
                    left: 0,
                    top: 0,
                    bottom: 0,
                    width: isMobile ? mobileSpineWidth : 22,
                    borderRadius: '3px 0 0 3px',
                    background: palette.spine,
                    borderRight: `1px solid ${palette.spineBorder}`,
                    boxShadow: `inset -3px 0 6px ${alphaColor(token.colorText, isDark ? 0.42 : 0.25)}, inset 2px 0 4px ${alphaColor(token.colorWhite, isDark ? 0.08 : 0.14)}`,
                    zIndex: 0,
                    pointerEvents: 'none',
                  }} />
                  <div style={{
                    position: 'absolute',
                    top: 0,
                    right: isMobile ? 16 : 24,
                    width: isMobile ? 24 : 30,
                    height: isMobile ? 42 : 52,
                    clipPath: 'polygon(0 0, 100% 0, 100% 100%, 50% calc(100% - 7px), 0 100%)',
                    background: palette.ribbon,
                    boxShadow: `0 8px 16px ${alphaColor(token.colorText, isDark ? 0.26 : 0.14)}`,
                    display: 'flex',
                    alignItems: 'flex-start',
                    justifyContent: 'center',
                    paddingTop: isMobile ? 8 : 10,
                    zIndex: 3,
                    pointerEvents: 'none',
                  }}>
                    {ribbonStatusIcon}
                  </div>

                  <div style={{
                    position: 'relative',
                    zIndex: 2,
                    display: 'flex',
                    flexDirection: 'column',
                    minHeight: isMobile ? mobileBookHeight : desktopBookHeight,
                    padding: isMobile ? '18px 16px 14px 38px' : '26px 24px 18px 42px',
                  }}>
                    <div style={{ marginBottom: isMobile ? 10 : 12, paddingRight: isMobile ? 18 : 30 }}>
                      <div style={{
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: 8,
                        marginBottom: isMobile ? 8 : 10,
                        minHeight: isMobile ? 50 : 58,
                      }}>
                        <BookOutlined style={{
                          fontSize: isMobile ? 14 : 16,
                          color: alphaColor(token.colorText, 0.4),
                          marginTop: 2,
                          flexShrink: 0,
                        }} />
                        <Tooltip title={project.title}>
                          <div style={{
                            fontSize: isMobile ? 18 : 22,
                            fontWeight: 600,
                            color: token.colorText,
                            lineHeight: 1.3,
                            display: '-webkit-box',
                            WebkitLineClamp: 2,
                            WebkitBoxOrient: 'vertical',
                            overflow: 'hidden',
                            wordBreak: 'break-word',
                            fontFamily: designDisplayFont,
                            letterSpacing: '-0.02em',
                          }}>
                            {project.title}
                          </div>
                        </Tooltip>
                      </div>

                      <div style={{
                        display: 'flex',
                        flexWrap: 'wrap',
                        gap: 6,
                        minHeight: isMobile ? 20 : 22,
                        alignItems: 'flex-start',
                      }}>
                        {tags.length > 0 ? tags.slice(0, 3).map((tag: string, idx: number) => (
                          <Tag key={idx} style={{
                            margin: 0,
                            padding: isMobile ? '0 7px' : '0 8px',
                            borderRadius: 4,
                            border: `1px solid ${alphaColor(token.colorSuccess, 0.18)}`,
                            background: alphaColor(token.colorSuccess, 0.08),
                            color: token.colorSuccess,
                            fontSize: isMobile ? 10 : 11,
                            lineHeight: isMobile ? '18px' : '20px',
                            fontWeight: 500,
                          }}>
                            {tag}
                          </Tag>
                        )) : (
                          <Tag style={{
                            margin: 0,
                            padding: isMobile ? '0 7px' : '0 8px',
                            borderRadius: 4,
                            border: `1px solid ${alphaColor(token.colorSuccess, 0.18)}`,
                            background: alphaColor(token.colorSuccess, 0.08),
                            color: token.colorSuccess,
                            fontSize: isMobile ? 10 : 11,
                            lineHeight: isMobile ? '18px' : '20px',
                            fontWeight: 500,
                          }}>
                            未分类
                          </Tag>
                        )}
                      </div>
                    </div>

                    <Paragraph
                      ellipsis={{ rows: isMobile ? 3 : 3 }}
                      style={{
                        fontSize: isMobile ? 12 : 13,
                        color: token.colorTextSecondary,
                        marginBottom: isMobile ? 12 : 16,
                        lineHeight: 1.7,
                        flexGrow: 1,
                      }}
                    >
                      {project.description || '暂无描述...'}
                    </Paragraph>

                    <div style={{ marginBottom: isMobile ? 14 : 18 }}>
                      <div style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        marginBottom: 6,
                        fontSize: isMobile ? 11 : 12,
                      }}>
                        <span style={{ color: token.colorTextTertiary }}>完成进度</span>
                        <span style={{ color: progressColor, fontWeight: 700 }}>{progress}%</span>
                      </div>
                      <div style={{
                        height: 6,
                        width: '100%',
                        borderRadius: 999,
                        overflow: 'hidden',
                        background: alphaColor(token.colorText, 0.06),
                      }}>
                        <div style={{
                          width: `${progress}%`,
                          height: '100%',
                          borderRadius: 999,
                          background: progressColor,
                          transition: 'width 0.3s ease',
                        }} />
                      </div>
                    </div>

                    <div style={{
                      marginBottom: isMobile ? 12 : 16,
                      padding: isMobile ? '12px 10px' : '14px 12px',
                      background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.94)} 0%, ${alphaColor(token.colorFillSecondary, 0.78)} 100%)`,
                      borderRadius: 10,
                      border: `1px solid ${alphaColor(token.colorText, 0.06)}`,
                      boxShadow: `inset 0 1px 2px ${alphaColor(token.colorText, 0.08)}`,
                    }}>
                      <div style={{ display: 'flex', alignItems: 'stretch', textAlign: 'center' }}>
                        <div style={{ flex: 1 }}>
                          <div style={{
                            fontSize: isMobile ? 22 : 26,
                            fontWeight: 600,
                            color: token.colorText,
                            lineHeight: 1.1,
                            fontFamily: designDisplayFont,
                            letterSpacing: '-0.03em',
                          }}>
                            {formatWordCount(project.current_words || 0)}
                          </div>
                          <div style={{
                            fontSize: isMobile ? 10 : 11,
                            color: token.colorTextTertiary,
                            marginTop: 4,
                          }}>
                            已写字数
                          </div>
                        </div>
                        <div style={{
                          width: 1,
                          margin: '0 12px',
                          background: alphaColor(token.colorText, 0.1),
                        }} />
                        <div style={{ flex: 1 }}>
                          <div style={{
                            fontSize: isMobile ? 22 : 26,
                            fontWeight: 600,
                            color: progress >= 100 ? token.colorSuccess : progressColor,
                            lineHeight: 1.1,
                            fontFamily: designDisplayFont,
                            letterSpacing: '-0.03em',
                          }}>
                            {formatWordCount(project.target_words || 0)}
                          </div>
                          <div style={{
                            fontSize: isMobile ? 10 : 11,
                            color: token.colorTextTertiary,
                            marginTop: 4,
                          }}>
                            目标字数
                          </div>
                        </div>
                      </div>
                    </div>

                    <div style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      paddingTop: isMobile ? 10 : 12,
                      borderTop: `1px solid ${alphaColor(token.colorText, 0.06)}`,
                      color: token.colorTextTertiary,
                      marginTop: 'auto',
                    }}>
                      <Space size={4} style={{ fontSize: isMobile ? 11 : 12, color: token.colorTextTertiary }}>
                        <CalendarOutlined style={{ fontSize: isMobile ? 10 : 12 }} />
                        {formatDate(project.updated_at)}
                      </Space>

                      <Button
                        type="text"
                        size="small"
                        danger
                        icon={<DeleteOutlined style={{ fontSize: isMobile ? 12 : 14 }} />}
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteProject(project.id);
                        }}
                        style={{
                          padding: isMobile ? '2px 4px' : '4px 8px',
                          borderRadius: 8,
                        }}
                      />
                    </div>
                  </div>
                </Card>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
