/**
 * 章节阅读器组件
 * 提供沉浸式阅读体验，支持主题切换、字体调节、翻页导航等功能
 */
import { useState, useEffect, useCallback } from 'react';
import { Modal, Button, Slider, Radio, Space, Typography, message, theme, Tag } from 'antd';
import {
  LeftOutlined,
  RightOutlined,
  SettingOutlined,
  FontSizeOutlined,
  BgColorsOutlined,
  CloseOutlined,
  ColumnHeightOutlined
} from '@ant-design/icons';
import type { Chapter } from '../types';
import InlineDeferredPanel from './InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';

const { Text, Paragraph, Title } = Typography;

// 阅读器设置接口
interface ReaderSettings {
  fontSize: number;       // 字体大小
  theme: 'light' | 'sepia' | 'dark';  // 主题模式
  lineHeight: number;     // 行高
}

// 组件属性接口
interface ChapterReaderProps {
  visible: boolean;                           // 是否显示
  chapter: Chapter;                           // 当前章节
  onClose: () => void;                        // 关闭回调
  onChapterChange: (chapterId: string) => void;  // 章节切换回调
}

// 导航信息接口
interface NavigationInfo {
  previous: { id: string; chapter_number: number; title: string } | null;
  next: { id: string; chapter_number: number; title: string } | null;
  current: { id: string; chapter_number: number; title: string };
}

interface ReaderThemeStyle {
  bg: string;
  text: string;
  headerBg: string;
  border: string;
}

// 本地存储key
const SETTINGS_STORAGE_KEY = 'chapter-reader-settings';

// 从本地存储加载设置
const loadSettings = (): ReaderSettings => {
  try {
    const saved = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch (e) {
    console.warn('加载阅读器设置失败:', e);
  }
  return {
    fontSize: 18,
    theme: 'light',
    lineHeight: 1.8
  };
};

// 保存设置到本地存储
const saveSettings = (settings: ReaderSettings) => {
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch (e) {
    console.warn('保存阅读器设置失败:', e);
  }
};

export default function ChapterReader({ 
  visible, 
  chapter, 
  onClose, 
  onChapterChange 
}: ChapterReaderProps) {
  const { token } = theme.useToken();

  // 阅读器设置
  const [settings, setSettings] = useState<ReaderSettings>(loadSettings);
  
  // 导航信息
  const [navigation, setNavigation] = useState<NavigationInfo | null>(null);
  
  // 加载状态
  const [loading, setLoading] = useState(false);
  
  // 设置面板显示状态
  const [showSettings, setShowSettings] = useState(false);
  
  // 移动端检测
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);

  // 响应式检测
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // 获取章节导航信息
  useEffect(() => {
    if (visible && chapter?.id) {
      setLoading(true);
      fetch(`/api/chapters/${chapter.id}/navigation`)
        .then(res => {
          if (!res.ok) throw new Error('获取导航失败');
          return res.json();
        })
        .then(data => {
          setNavigation(data);
          setLoading(false);
        })
        .catch(err => {
          console.error('获取导航信息失败:', err);
          message.error('获取章节导航信息失败');
          setLoading(false);
        });
    }
  }, [visible, chapter?.id]);

  // 保存设置变更
  useEffect(() => {
    saveSettings(settings);
  }, [settings]);

  // 上一章
  const handlePrevious = useCallback(() => {
    if (navigation?.previous) {
      setLoading(true);
      onChapterChange(navigation.previous.id);
    }
  }, [navigation?.previous, onChapterChange]);

  // 下一章
  const handleNext = useCallback(() => {
    if (navigation?.next) {
      setLoading(true);
      onChapterChange(navigation.next.id);
    }
  }, [navigation?.next, onChapterChange]);

  // 键盘快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!visible) return;
      
      // 忽略输入框中的按键
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }
      
      switch (e.key) {
        case 'ArrowLeft':
          handlePrevious();
          break;
        case 'ArrowRight':
          handleNext();
          break;
        case 'Escape':
          onClose();
          break;
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [visible, handlePrevious, handleNext, onClose]);

  // 章节变化后自动回到顶部
  useEffect(() => {
    if (chapter?.id) {
      setLoading(false);
      // 找到滚动容器并滚动到顶部
      const scrollContainer = document.querySelector('.reader-scroll-container');
      if (scrollContainer) {
        scrollContainer.scrollTop = 0;
      }
    }
  }, [chapter?.id]);

  // 当前主题样式
  const themeStyles: Record<ReaderSettings['theme'], ReaderThemeStyle> = {
    light: {
      bg: token.colorBgContainer,
      text: token.colorText,
      headerBg: token.colorBgElevated,
      border: token.colorBorderSecondary,
    },
    sepia: {
      bg: `color-mix(in srgb, ${token.colorWarningBg} 72%, ${token.colorBgContainer} 28%)`,
      text: `color-mix(in srgb, ${token.colorText} 85%, ${token.colorTextSecondary} 15%)`,
      headerBg: `color-mix(in srgb, ${token.colorWarningBg} 58%, ${token.colorBgElevated} 42%)`,
      border: `color-mix(in srgb, ${token.colorWarningBorder} 65%, ${token.colorBorder} 35%)`,
    },
    dark: {
      bg: `color-mix(in srgb, ${token.colorTextBase} 92%, ${token.colorBgContainer} 8%)`,
      text: `color-mix(in srgb, ${token.colorTextLightSolid} 82%, ${token.colorTextSecondary} 18%)`,
      headerBg: `color-mix(in srgb, ${token.colorTextBase} 84%, ${token.colorBgElevated} 16%)`,
      border: `color-mix(in srgb, ${token.colorTextBase} 60%, ${token.colorBorder} 40%)`,
    },
  };
  const currentTheme = themeStyles[settings.theme];
  const readerGuideSteps = [
    '先确认当前章节与导航状态，再进入沉浸阅读，不把阅读器当成编辑入口。',
    '需要调整观感时再打开设置面板，按主题、字体、行高顺序微调阅读体验。',
    '读完后再通过底部导航切换章节，原有翻页、快捷键和本地设置持久化逻辑保持不变。',
  ];
  const readerFocus = loading
    ? {
      title: '等待章节导航与正文同步完成',
      note: '当前更适合稍等片刻，阅读器会沿现有逻辑载入导航信息并恢复沉浸式阅读状态。',
      tags: [
        { label: '加载中', color: 'processing' },
        { label: settings.theme === 'light' ? '日间主题' : settings.theme === 'sepia' ? '护眼主题' : '夜间主题', color: 'blue' },
      ],
    }
    : {
      title: navigation?.next || navigation?.previous ? '当前适合沿章节链路连续阅读' : '当前更适合专注阅读这一章正文',
      note: showSettings
        ? '设置面板已经展开，可以先微调阅读观感，再继续沉浸阅读正文。'
        : '先通读正文，再按需要打开设置面板调整字体、行高与主题，避免频繁打断阅读节奏。',
      tags: [
        { label: settings.theme === 'light' ? '日间主题' : settings.theme === 'sepia' ? '护眼主题' : '夜间主题', color: 'blue' },
        { label: `${settings.fontSize}px 字体`, color: 'purple' },
        { label: `${settings.lineHeight} 行高`, color: 'green' },
      ],
    };
  const guideStripBackground = `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 10%, ${currentTheme.headerBg} 90%) 0%, color-mix(in srgb, ${token.colorInfo} 8%, ${currentTheme.headerBg} 92%) 100%)`;

  // 更新设置的便捷函数
  const updateSettings = (key: keyof ReaderSettings, value: number | string) => {
    setSettings(prev => ({ ...prev, [key]: value }));
  };


  const navigationSummary = navigation
    ? `${navigation.previous ? `← ${navigation.previous.title}` : '已是第一章'} | ${navigation.next ? `${navigation.next.title} →` : '已是最后一章'}`
    : '';

  return (
    <Modal
      open={visible}
      onCancel={onClose}
      footer={null}
      width="100%"
      style={{
        maxWidth: '100vw',
        top: 0,
        margin: 0,
        padding: 0,
        height: '100vh',
        overflow: 'hidden'
      }}
      styles={{
        content: {
          height: '100vh',
          borderRadius: 0,
          boxShadow: 'none',
          padding: 0,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden'
        },
        body: {
          flex: 1,
          padding: 0,
          background: currentTheme.bg,
          overflow: 'hidden',
          height: '100%',
          scrollbarWidth: 'thin',
          display: 'flex',
          flexDirection: 'column'
        }
      }}
      closable={false}
      maskClosable={false}
    >
      {/* 顶部工具栏 */}
      <div style={{
        flex: 'none',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: isMobile ? '10px 12px' : '12px 20px',
        borderBottom: `1px solid ${currentTheme.border}`,
        background: currentTheme.headerBg,
        zIndex: 10
      }}>
        <Button 
          type="text" 
          icon={<CloseOutlined />} 
          onClick={onClose}
          style={{ color: currentTheme.text }}
        >
          {!isMobile && '关闭'}
        </Button>

        <div
          style={{
            maxWidth: isMobile ? '60%' : '70%',
            overflow: 'hidden',
            textAlign: 'center',
          }}
          title={`第${chapter.chapter_number}章：${chapter.title}`}
        >
          {!isMobile && (
            <Text
              style={{
                display: 'block',
                color: currentTheme.text,
                opacity: 0.58,
                fontSize: 11,
                letterSpacing: '0.14em',
                textTransform: 'uppercase',
                marginBottom: 2,
              }}
            >
              Chapter Reader
            </Text>
          )}
          <Title
            level={5}
            style={{
              margin: 0,
              color: currentTheme.text,
              maxWidth: '100%',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              fontSize: isMobile ? 14 : 16,
              fontFamily: designDisplayFont,
              letterSpacing: '-0.02em',
            }}
          >
            第{chapter.chapter_number}章：{chapter.title}
          </Title>
        </div>

        <Button
          type={showSettings ? 'primary' : 'text'}
          icon={<SettingOutlined />}
          onClick={() => setShowSettings(!showSettings)}
          style={{ color: showSettings ? undefined : currentTheme.text }}
          title="阅读设置"
        />
      </div>

      <div
        style={{
          flex: 'none',
          padding: isMobile ? '10px 12px' : '12px 20px',
          borderBottom: `1px solid ${currentTheme.border}`,
          background: guideStripBackground,
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.4fr) minmax(220px, 0.9fr)',
            gap: 12,
            alignItems: 'start',
          }}
        >
          <div>
            <Text
              style={{
                display: 'block',
                color: currentTheme.text,
                opacity: 0.62,
                fontSize: 11,
                letterSpacing: '0.14em',
                textTransform: 'uppercase',
              }}
            >
              Reading Guide
            </Text>
            <Paragraph
              style={{
                margin: '6px 0 0',
                color: currentTheme.text,
                lineHeight: 1.7,
                opacity: 0.86,
                fontSize: isMobile ? 12 : 13,
              }}
            >
              这里保留沉浸阅读、章节导航、快捷键和本地阅读设置逻辑不变，只补一层轻导览，让阅读顺序和设置入口更清晰。
            </Paragraph>
          </div>
          <div
            style={{
              borderRadius: 16,
              padding: isMobile ? '12px 12px 10px' : '12px 14px',
              background: 'rgba(255,255,255,0.08)',
              border: '1px solid rgba(255,255,255,0.08)',
              backdropFilter: 'blur(8px)',
            }}
          >
            <Text
              style={{
                display: 'block',
                color: currentTheme.text,
                opacity: 0.62,
                fontSize: 11,
                letterSpacing: '0.14em',
                textTransform: 'uppercase',
              }}
            >
              当前阅读焦点
            </Text>
            <Text
              strong
              style={{
                display: 'block',
                color: currentTheme.text,
                marginTop: 6,
                fontSize: 14,
              }}
            >
              {readerFocus.title}
            </Text>
            <Paragraph
              style={{
                margin: '6px 0 0',
                color: currentTheme.text,
                opacity: 0.8,
                lineHeight: 1.65,
                fontSize: 12,
              }}
            >
              {readerFocus.note}
            </Paragraph>
            <Space wrap size={[8, 8]} style={{ marginTop: 10 }}>
              {readerFocus.tags.map((tag) => (
                <Tag key={`${tag.color}-${tag.label}`} color={tag.color} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  {tag.label}
                </Tag>
              ))}
            </Space>
          </div>
        </div>
      </div>

      {/* 设置面板 */}
      {showSettings && (
        <div style={{
          padding: isMobile ? '12px 16px' : '16px 24px',
          borderBottom: `1px solid ${currentTheme.border}`,
          background: currentTheme.headerBg
        }}>
          <div style={{ marginBottom: 16 }}>
            <Text
              style={{
                display: 'block',
                color: currentTheme.text,
                opacity: 0.62,
                fontSize: 11,
                letterSpacing: '0.14em',
                textTransform: 'uppercase',
              }}
            >
              Reader Controls
            </Text>
            <Paragraph
              style={{
                margin: '6px 0 0',
                color: currentTheme.text,
                opacity: 0.82,
                lineHeight: 1.7,
                fontSize: 13,
              }}
            >
              先按下方顺序微调观感，再回到正文继续沉浸阅读；这里只重排信息层级，不改变设置持久化和交互行为。
            </Paragraph>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
              {readerGuideSteps.map((item, index) => (
                <span
                  key={item}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    borderRadius: 999,
                    background: 'rgba(255,255,255,0.08)',
                    border: '1px solid rgba(255,255,255,0.1)',
                    color: currentTheme.text,
                    fontSize: 12,
                  }}
                >
                  <span style={{ fontWeight: 700 }}>{index + 1}</span>
                  {item}
                </span>
              ))}
            </div>
          </div>
          <Space 
            direction={isMobile ? 'vertical' : 'horizontal'} 
            size="large"
            style={{ width: '100%' }}
            wrap
          >
            {/* 字体大小 */}
            <div style={{ minWidth: isMobile ? '100%' : 200 }}>
              <Space style={{ marginBottom: 8, color: currentTheme.text }}>
                <FontSizeOutlined />
                <span>字体大小: {settings.fontSize}px</span>
              </Space>
              <Slider
                min={14}
                max={28}
                value={settings.fontSize}
                onChange={v => updateSettings('fontSize', v)}
                style={{ margin: '8px 0' }}
              />
            </div>

            {/* 行高 */}
            <div style={{ minWidth: isMobile ? '100%' : 200 }}>
              <Space style={{ marginBottom: 8, color: currentTheme.text }}>
                <ColumnHeightOutlined />
                <span>行高: {settings.lineHeight}</span>
              </Space>
              <Slider
                min={1.4}
                max={2.5}
                step={0.1}
                value={settings.lineHeight}
                onChange={v => updateSettings('lineHeight', v)}
                style={{ margin: '8px 0' }}
              />
            </div>

            {/* 主题 */}
            <div>
              <Space style={{ marginBottom: 8, color: currentTheme.text }}>
                <BgColorsOutlined />
                <span>主题</span>
              </Space>
              <div>
                <Radio.Group
                  value={settings.theme}
                  onChange={e => updateSettings('theme', e.target.value)}
                  buttonStyle="solid"
                  size={isMobile ? 'small' : 'middle'}
                >
                  <Radio.Button value="light">日间</Radio.Button>
                  <Radio.Button value="sepia">护眼</Radio.Button>
                  <Radio.Button value="dark">夜间</Radio.Button>
                </Radio.Group>
              </div>
            </div>
          </Space>
        </div>
      )}

      {/* 章节内容区域 */}
      <div
        className="reader-scroll-container"
        style={{
          flex: 1,
          overflowY: 'auto',
          position: 'relative',
          scrollBehavior: 'smooth'
        }}
      >
        {loading ? (
          <div
            style={{
              maxWidth: 1000,
              margin: '0 auto',
              padding: isMobile ? '24px 16px 40px' : '40px 60px 40px',
              minHeight: '100%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <InlineDeferredPanel
              eyebrow="Reader Workspace"
              title="恢复沉浸阅读与章节导航"
              message="当前正在同步章节导航、阅读设置与正文内容。原有翻页、快捷键、本地设置持久化与沉浸阅读逻辑保持不变。"
              minHeight={280}
              tags={[
                { label: '章节导航恢复中', color: 'processing' },
                { label: settings.theme === 'light' ? '日间主题' : settings.theme === 'sepia' ? '护眼主题' : '夜间主题', color: 'blue' },
                { label: `${settings.fontSize}px 字体`, color: 'purple' },
              ]}
            />
          </div>
        ) : (
          <div
            style={{
              maxWidth: 1000,
              margin: '0 auto',
              padding: isMobile ? '24px 16px 40px' : '40px 60px 40px',
              minHeight: '100%',
              fontSize: settings.fontSize,
            lineHeight: settings.lineHeight,
            color: currentTheme.text,
            whiteSpace: 'pre-wrap',
            textAlign: 'justify',
            wordBreak: 'break-word',
            overflowWrap: 'break-word'
          }}
        >
          {chapter.content ? (
            // 按段落渲染内容，优化阅读体验
            chapter.content.split('\n').map((paragraph, index) => (
              paragraph.trim() ? (
                <p
                  key={index}
                  style={{
                    textIndent: '2em',
                    margin: 0,
                    marginBottom: '0.8em'
                  }}
                >
                  {paragraph}
                </p>
              ) : (
                <br key={index} />
              )
            ))
          ) : (
            <div style={{ 
              textAlign: 'center', 
              padding: '60px 20px',
              color: currentTheme.text,
              opacity: 0.6
            }}>
              暂无正文内容，请返回编辑器补充后再阅读
            </div>
          )}
          </div>
        )}
      </div>

      {/* 底部导航栏 */}
      <div style={{
        flex: 'none',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: isMobile ? '12px 16px' : '16px 24px',
        borderTop: `1px solid ${currentTheme.border}`,
        background: currentTheme.headerBg,
        zIndex: 100
      }}>
        <Button
          type="primary"
          icon={<LeftOutlined />}
          disabled={!navigation?.previous || loading}
          onClick={handlePrevious}
          size={isMobile ? 'middle' : 'large'}
        >
          {!isMobile && '上一章'}
        </Button>
        
        <div style={{ 
          textAlign: 'center',
          color: currentTheme.text,
          fontSize: isMobile ? 12 : 14,
          flex: 1,
          minWidth: 0,
          padding: isMobile ? '0 8px' : '0 16px'
        }}>
          <div>{chapter.word_count || 0} 字</div>
          {navigation && (
            <div
              title={navigationSummary}
              style={{ fontSize: isMobile ? 10 : 12, opacity: 0.7, maxWidth: '100%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              {navigationSummary}
            </div>
          )}
        </div>
        
        <Button
          type="primary"
          disabled={!navigation?.next || loading}
          onClick={handleNext}
          size={isMobile ? 'middle' : 'large'}
        >
          {!isMobile && '下一章'}
          <RightOutlined />
        </Button>
      </div>
    </Modal>
  );
}
