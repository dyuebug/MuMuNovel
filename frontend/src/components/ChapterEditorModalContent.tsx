/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy, memo, useCallback, useEffect, useRef, useState } from 'react';
import { Button, Card, Form, Input, Select, Space, Typography, message, theme } from 'antd';
import { FundOutlined, FormOutlined, LockOutlined, ThunderboltOutlined } from '@ant-design/icons';
import PartialRegenerateToolbar from './PartialRegenerateToolbar';
import ChapterEditorAiSection from './ChapterEditorAiSection';
import WorkflowEntryFallback from './WorkflowEntryFallback';
import {
  renderCompactSelectionSummary,
  renderCompactSettingFlow,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';
import { CREATION_PLOT_STAGE_OPTIONS } from '../utils/creationPresetsCore';
import type { TextAreaRef } from 'antd/es/input/TextArea';

const { TextArea } = Input;
const { Text } = Typography;

const LazyPartialRegenerateModal = lazy(() => import('./PartialRegenerateModal'));

type RenderDebugGlobal = typeof globalThis & {
  __NOVEL_RENDER_DEBUG__?: boolean;
  __NOVEL_RENDER_DEBUG_FILTER__?: string[];
};

const noopRenderDiagnostics = (...args: [string, () => Record<string, unknown>]): void => {
  void args;
};

function useActiveRenderDiagnostics(componentName: string, getSnapshot: () => Record<string, unknown>): void {
  const renderCountRef = useRef(0);
  const previousSnapshotRef = useRef<Record<string, unknown> | null>(null);

  useEffect(() => {
    const renderDebugGlobal = globalThis as RenderDebugGlobal;
    if (!renderDebugGlobal.__NOVEL_RENDER_DEBUG__) {
      return;
    }

    const filters = renderDebugGlobal.__NOVEL_RENDER_DEBUG_FILTER__;
    if (Array.isArray(filters) && filters.length > 0 && !filters.includes(componentName)) {
      return;
    }

    renderCountRef.current += 1;
    const nextSnapshot = getSnapshot();
    const previousSnapshot = previousSnapshotRef.current;
    const changedKeys = previousSnapshot
      ? Object.keys(nextSnapshot).filter((key) => !Object.is(previousSnapshot[key], nextSnapshot[key]))
      : Object.keys(nextSnapshot);

    console.debug(`[render-debug] ${componentName} #${renderCountRef.current}`, {
      changedKeys,
      snapshot: nextSnapshot,
    });

    previousSnapshotRef.current = nextSnapshot;
  });
}

const useLocalRenderDiagnostics = import.meta.env.DEV ? useActiveRenderDiagnostics : noopRenderDiagnostics;

type ToolbarPosition = {
  top: number;
  left: number;
};

const calculatePartialRegenerateToolbarPosition = (
  textArea: HTMLTextAreaElement,
  selectionStart: number,
): ToolbarPosition => {
  const rect = textArea.getBoundingClientRect();
  const computedStyle = window.getComputedStyle(textArea);
  const lineHeight = parseFloat(computedStyle.lineHeight) || 24;
  const paddingTop = parseFloat(computedStyle.paddingTop) || 0;
  const textBeforeSelection = textArea.value.substring(0, selectionStart);
  const startLine = textBeforeSelection.split('\n').length - 1;
  const visualTop = (startLine * lineHeight) + paddingTop - textArea.scrollTop;
  const toolbarTop = rect.top + visualTop - 45;
  const toolbarLeft = rect.right - 180;

  let finalTop = toolbarTop;

  if (visualTop < 0) {
    finalTop = rect.top + 10;
  } else if (visualTop > textArea.clientHeight) {
    finalTop = rect.bottom - 50;
  }

  return {
    top: Math.max(rect.top + 10, Math.min(finalTop, rect.bottom - 50)),
    left: Math.min(Math.max(rect.left + 20, toolbarLeft), window.innerWidth - 200),
  };
};

type ChapterEditorModalContentProps = {
  contentProps: any;
};

const getNarrativePerspectiveText = (perspective?: string): string => {
  switch (perspective) {
    case 'first_person':
      return '第一人称';
    case 'third_person':
      return '第三人称';
    case 'omniscient':
      return '全知视角';
    default:
      return '未设定';
  }
};

const areCurrentEditingChaptersEqual = (previousChapter?: any, nextChapter?: any): boolean => {
  if (previousChapter === nextChapter) {
    return true;
  }

  if (!previousChapter || !nextChapter) {
    return previousChapter === nextChapter;
  }

  return previousChapter.id === nextChapter.id
    && previousChapter.chapter_number === nextChapter.chapter_number;
};

const areEditorModalContentPropsEqual = (
  previousProps: ChapterEditorModalContentProps,
  nextProps: ChapterEditorModalContentProps,
): boolean => {
  const previousContentProps = previousProps.contentProps;
  const nextContentProps = nextProps.contentProps;
  const previousKeys = Object.keys(previousContentProps);
  const nextKeys = Object.keys(nextContentProps);

  if (previousKeys.length !== nextKeys.length) {
    return false;
  }

  return previousKeys.every((key) => {
    if (key === 'currentEditingChapter') {
      return areCurrentEditingChaptersEqual(previousContentProps.currentEditingChapter, nextContentProps.currentEditingChapter);
    }

    if (key === 'showGenerateModal' || key === 'handleEditorSubmit') {
      return true;
    }

    return previousContentProps[key] === nextContentProps[key];
  });
};

function ChapterEditorModalContent({ contentProps }: ChapterEditorModalContentProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const {
    editorForm,
    handleEditorSubmit,
    isMobile,
    currentEditingChapter,
    currentEditingCanGenerate,
    currentEditingGenerateDisabledReason,
    showGenerateModal,
    isContinuing,
    canAnalyzeCurrentChapter,
    handleShowAnalysis,
    selectedStyleId,
    setSelectedStyleId,
    writingStyles,
    currentProjectNarrativePerspective,
    temporaryNarrativePerspective,
    setTemporaryNarrativePerspective,
    selectedPlotStage,
    setSelectedPlotStage,
    applyInferredSinglePlotStage,
    aiSectionProps,
    onCloseEditor,
  } = contentProps;

  const contentTextAreaRef = useRef<TextAreaRef>(null);
  const mountedRef = useRef(false);
  const selectionTimerRef = useRef<number | null>(null);
  const currentEditingChapterId = currentEditingChapter?.id ?? null;
  const [partialRegenerateToolbarVisible, setPartialRegenerateToolbarVisible] = useState(false);
  const [partialRegenerateToolbarPosition, setPartialRegenerateToolbarPosition] = useState<ToolbarPosition>({ top: 0, left: 0 });
  const [selectedTextForRegenerate, setSelectedTextForRegenerate] = useState('');
  const [selectionStartPosition, setSelectionStartPosition] = useState(0);
  const [selectionEndPosition, setSelectionEndPosition] = useState(0);
  const [partialRegenerateModalVisible, setPartialRegenerateModalVisible] = useState(false);

  const clearSelectionTimer = useCallback(() => {
    if (selectionTimerRef.current !== null) {
      window.clearTimeout(selectionTimerRef.current);
      selectionTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    return () => {
      mountedRef.current = false;
      clearSelectionTimer();
    };
  }, [clearSelectionTimer]);

  useLocalRenderDiagnostics('ChapterEditorModalContent', () => ({
    chapterId: currentEditingChapterId,
    chapterNumber: currentEditingChapter?.chapter_number,
    canGenerate: currentEditingCanGenerate,
    canAnalyze: canAnalyzeCurrentChapter,
    selectedStyleId,
    selectedPlotStage,
    partialSelectionLength: selectedTextForRegenerate.length,
    partialRegenerateToolbarVisible,
    partialRegenerateModalVisible,
  }));

  const handleOpenPartialRegenerate = useCallback(() => {
    setPartialRegenerateToolbarVisible(false);
    setPartialRegenerateModalVisible(true);
  }, []);

  const handleApplyPartialRegenerate = useCallback((newText: string, startPos: number, endPos: number) => {
    const currentContent = editorForm.getFieldValue('content') || '';
    const newContent = currentContent.substring(0, startPos) + newText + currentContent.substring(endPos);
    editorForm.setFieldsValue({ content: newContent });
    setPartialRegenerateModalVisible(false);
    message.success('已应用重写内容');
  }, [editorForm]);

  const handleTextSelection = useCallback(() => {
    const selection = window.getSelection();

    if (!selection || selection.rangeCount === 0) {
      setPartialRegenerateToolbarVisible(false);
      return;
    }

    const selectedText = selection.toString().trim();
    if (selectedText.length < 10) {
      setPartialRegenerateToolbarVisible(false);
      return;
    }

    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;
    if (!textArea) {
      setPartialRegenerateToolbarVisible(false);
      return;
    }

    if (document.activeElement !== textArea) {
      setPartialRegenerateToolbarVisible(false);
      return;
    }

    const start = textArea.selectionStart;
    const end = textArea.selectionEnd;
    const selectedInTextArea = textArea.value.substring(start, end);

    if (selectedInTextArea.trim().length < 10) {
      setPartialRegenerateToolbarVisible(false);
      return;
    }

    setSelectedTextForRegenerate(selectedInTextArea);
    setSelectionStartPosition(start);
    setSelectionEndPosition(end);
    setPartialRegenerateToolbarPosition(calculatePartialRegenerateToolbarPosition(textArea, start));
    setPartialRegenerateToolbarVisible(true);
  }, []);

  const scheduleHandleTextSelection = useCallback(() => {
    clearSelectionTimer();
    selectionTimerRef.current = window.setTimeout(() => {
      selectionTimerRef.current = null;
      if (!mountedRef.current) {
        return;
      }
      handleTextSelection();
    }, 50);
  }, [clearSelectionTimer, handleTextSelection]);

  const updateToolbarPosition = useCallback(() => {
    if (!mountedRef.current || !partialRegenerateToolbarVisible || !selectedTextForRegenerate) {
      return;
    }

    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;
    if (!textArea) {
      return;
    }

    setPartialRegenerateToolbarPosition(calculatePartialRegenerateToolbarPosition(textArea, selectionStartPosition));
  }, [partialRegenerateToolbarVisible, selectedTextForRegenerate, selectionStartPosition]);

  useEffect(() => {
    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;
    if (!textArea) {
      return undefined;
    }

    const handleMouseUp = () => {
      scheduleHandleTextSelection();
    };
    const handleKeyUp = () => {
      scheduleHandleTextSelection();
    };
    const handleScroll = () => {
      updateToolbarPosition();
    };
    const modalBody = document.querySelector('.ant-modal-body');

    textArea.addEventListener('mouseup', handleMouseUp);
    textArea.addEventListener('keyup', handleKeyUp);
    textArea.addEventListener('scroll', handleScroll);
    if (modalBody) {
      modalBody.addEventListener('scroll', handleScroll);
    }
    window.addEventListener('resize', handleScroll);

    return () => {
      clearSelectionTimer();
      textArea.removeEventListener('mouseup', handleMouseUp);
      textArea.removeEventListener('keyup', handleKeyUp);
      textArea.removeEventListener('scroll', handleScroll);
      if (modalBody) {
        modalBody.removeEventListener('scroll', handleScroll);
      }
      window.removeEventListener('resize', handleScroll);
    };
  }, [clearSelectionTimer, scheduleHandleTextSelection, updateToolbarPosition]);

  useEffect(() => {
    if (!partialRegenerateToolbarVisible) {
      return undefined;
    }

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as HTMLElement;

      if (target.closest('[data-partial-regenerate-toolbar]')) {
        return;
      }

      if (target.tagName === 'TEXTAREA') {
        return;
      }

      if (target.closest('.ant-modal-content')) {
        return;
      }

      setPartialRegenerateToolbarVisible(false);
    };

    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  }, [partialRegenerateToolbarVisible]);

  useEffect(() => {
    clearSelectionTimer();
    setPartialRegenerateToolbarVisible(false);
    setPartialRegenerateModalVisible(false);
    setSelectedTextForRegenerate('');
    setSelectionStartPosition(0);
    setSelectionEndPosition(0);
  }, [clearSelectionTimer, currentEditingChapterId]);

  const selectedRegenerateCount = selectedTextForRegenerate.trim().length;
  const hasPartialSelection = selectedRegenerateCount > 0;
  const chapterDisplayTitle = currentEditingChapter?.title || '未命名章节';
  const chapterDisplayNumber = currentEditingChapter?.chapter_number ? `第 ${currentEditingChapter.chapter_number} 章` : '章节编辑';
  const chapterEditorGuideSteps = [
    '先确认章节标题、风格、视角和剧情阶段，让本章的创作上下文在动笔前保持一致。',
    '再决定这一轮是继续生成、查看分析，还是直接手动编辑正文，避免在多个入口之间来回切换。',
    '最后再进入局部重写或全文修改，把动作建立在已经明确的章节目标之上。',
  ];
  const chapterEditorWorkspaceFocus = hasPartialSelection
    ? {
        title: `优先处理当前选中的 ${selectedRegenerateCount} 字片段`,
        note: '当前已经选中可重写的正文片段，适合先完成局部重写或回到正文修改，再决定是否继续分析或续写整章内容。',
      }
    : !currentEditingCanGenerate
      ? {
          title: '先补齐本章继续生成前的条件',
          note: currentEditingGenerateDisabledReason || '当前还不满足继续生成条件，适合先确认创作设置与正文状态，再推进下一步生成。',
        }
      : canAnalyzeCurrentChapter
        ? {
            title: '围绕本章现有内容安排下一步动作',
            note: '当前既可继续生成，也能查看分析，更适合先判断这一轮是要扩写内容还是先复核质量信号。',
          }
        : {
            title: '先稳定本章的编辑上下文',
            note: '当前更适合先核对标题、创作设置与正文内容，再决定是否进入后续生成或分析链路。',
          };

  return (
    <Form form={editorForm} layout="vertical" onFinish={handleEditorSubmit}>
      <Card
        size="small"
        style={{
          marginBottom: 14,
          borderRadius: 22,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.84)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Chapter Editing Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              {chapterDisplayTitle}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里是章节正文编辑工作台。不会改变原有编辑、续写、分析或局部重写逻辑，只是把工作顺序和判断重点提前说明，让这一章的编辑节奏更清楚。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {chapterEditorGuideSteps.map((item, index) => (
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
              {chapterEditorWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {chapterEditorWorkspaceFocus.note}
            </Text>
            <Space wrap size={[8, 8]}>
              <Text
                style={{
                  padding: '4px 10px',
                  borderRadius: 999,
                  background: alphaColor(token.colorPrimary, 0.08),
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                }}
              >
                {chapterDisplayNumber}
              </Text>
              <Text
                style={{
                  padding: '4px 10px',
                  borderRadius: 999,
                  background: currentEditingCanGenerate ? alphaColor(token.colorSuccess, 0.08) : alphaColor(token.colorWarning, 0.08),
                  border: `1px solid ${currentEditingCanGenerate ? alphaColor(token.colorSuccess, 0.14) : alphaColor(token.colorWarning, 0.16)}`,
                }}
              >
                {currentEditingCanGenerate ? '可继续生成' : '当前不可续写'}
              </Text>
              <Text
                style={{
                  padding: '4px 10px',
                  borderRadius: 999,
                  background: canAnalyzeCurrentChapter ? alphaColor(token.colorInfo, 0.08) : alphaColor(token.colorTextTertiary, 0.08),
                  border: `1px solid ${canAnalyzeCurrentChapter ? alphaColor(token.colorInfo, 0.14) : alphaColor(token.colorBorderSecondary, 0.9)}`,
                }}
              >
                {canAnalyzeCurrentChapter ? '可查看分析' : '暂无分析入口'}
              </Text>
              <Text
                style={{
                  padding: '4px 10px',
                  borderRadius: 999,
                  background: hasPartialSelection ? alphaColor(token.colorInfo, 0.08) : alphaColor(token.colorTextTertiary, 0.08),
                  border: `1px solid ${hasPartialSelection ? alphaColor(token.colorInfo, 0.14) : alphaColor(token.colorBorderSecondary, 0.9)}`,
                }}
              >
                {hasPartialSelection ? `已选 ${selectedRegenerateCount} 字` : '未选择片段'}
              </Text>
            </Space>
          </div>
        </div>
      </Card>

      <Card
        size="small"
        style={{
          marginBottom: isMobile ? 16 : 12,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: alphaColor(token.colorBgContainer, 0.98),
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Chapter Header
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8 }}>
          标题与快捷操作
        </Text>
        <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
          标题当前只读，章节管理动作仍然从这里快速触发，避免来回切换工作区域。
        </Text>
        <Form.Item
          label="章节标题"
          tooltip="当前标题仅供查看，修改请前往章节设置"
          style={{ marginBottom: 0 }}
        >
          <Space.Compact style={{ width: '100%' }}>
            <Form.Item name="title" noStyle>
              <Input disabled style={{ flex: 1 }} />
            </Form.Item>
            {currentEditingChapter ? (
              <>
                <Button
                  type="primary"
                  icon={currentEditingCanGenerate ? <ThunderboltOutlined /> : <LockOutlined />}
                  onClick={() => showGenerateModal(currentEditingChapter)}
                  loading={isContinuing}
                  disabled={!currentEditingCanGenerate}
                  danger={!currentEditingCanGenerate}
                  style={{ fontWeight: 'bold' }}
                  title={!currentEditingCanGenerate ? currentEditingGenerateDisabledReason : '继续生成章节内容'}
                >
                  {isMobile ? '续写' : '继续生成'}
                </Button>
                <Button
                  icon={<FundOutlined />}
                  onClick={() => handleShowAnalysis(currentEditingChapter.id)}
                  disabled={!canAnalyzeCurrentChapter}
                  title={canAnalyzeCurrentChapter ? '查看章节分析' : '暂无内容，无法分析'}
                >
                  {isMobile ? '分析' : '分析章节'}
                </Button>
              </>
            ) : null}
          </Space.Compact>
        </Form.Item>
      </Card>

      {renderCompactSettingFlow(
        '生成前先确认本章创作设置。',
        '写作风格、叙事视角和剧情阶段会直接影响续写结果。',
        ['确认标题', '选择风格', '设置视角', '设置阶段'],
      )}

      <Card
        size="small"
        title="本章创作设置"
        style={{
          marginBottom: 12,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.44)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <div
          style={{
            display: isMobile ? 'block' : 'flex',
            gap: isMobile ? 0 : 16,
            marginBottom: 0,
          }}
        >
          <Form.Item
            label="写作风格"
            tooltip="选择用于本章续写的写作风格"
            required
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder="请选择写作风格"
              value={selectedStyleId}
              onChange={setSelectedStyleId}
              status={!selectedStyleId ? 'error' : undefined}
            >
              {writingStyles.map((style: any) => (
                <Select.Option key={style.id} value={style.id}>
                  {style.name}
                  {style.is_default ? '（默认）' : ''}
                </Select.Option>
              ))}
            </Select>
            {!selectedStyleId ? (
              <div style={{ color: '#ff4d4f', fontSize: 12, marginTop: 4 }}>请选择写作风格</div>
            ) : null}
          </Form.Item>

          <Form.Item
            label="叙事视角"
            tooltip="留空则沿用项目默认视角，也可临时覆盖本章视角"
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder={`留空则沿用项目视角：${getNarrativePerspectiveText(currentProjectNarrativePerspective)}`}
              value={temporaryNarrativePerspective}
              onChange={setTemporaryNarrativePerspective}
              allowClear
            >
              <Select.Option value="first_person">第一人称</Select.Option>
              <Select.Option value="third_person">第三人称</Select.Option>
              <Select.Option value="omniscient">全知视角</Select.Option>
            </Select>
            {temporaryNarrativePerspective ? (
              <div style={{ color: 'var(--color-success)', fontSize: 12, marginTop: 4 }}>
                当前选择：{getNarrativePerspectiveText(temporaryNarrativePerspective)}
              </div>
            ) : null}
          </Form.Item>

          <Form.Item
            label="剧情阶段"
            tooltip="帮助系统判断当前章节更像铺陈、高潮还是收束回收"
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder="请选择剧情阶段"
              value={selectedPlotStage}
              onChange={setSelectedPlotStage}
              allowClear
              optionLabelProp="label"
            >
              {CREATION_PLOT_STAGE_OPTIONS.map((option: any) => (
                <Select.Option key={option.value} value={option.value} label={option.label}>
                  <div>{option.label}</div>
                  <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                </Select.Option>
              ))}
            </Select>
            <Space size={8} style={{ marginTop: 8 }}>
              <Button size="small" onClick={applyInferredSinglePlotStage}>应用推断阶段</Button>
              {selectedPlotStage ? (
                <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                  已应用：{CREATION_PLOT_STAGE_OPTIONS.find((item: any) => item.value === selectedPlotStage)?.label || selectedPlotStage}
                </span>
              ) : null}
            </Space>
          </Form.Item>
        </div>
      </Card>

      <ChapterEditorAiSection sectionProps={aiSectionProps} />

      {renderCompactStoryControlHeader(
        '局部智能重写',
        hasPartialSelection
          ? `已选 ${selectedRegenerateCount} 字，可直接发起局部重写`
          : '先选中一段正文，再使用局部重写',
        {
          tagText: hasPartialSelection ? `已选 ${selectedRegenerateCount} 字` : '未选择文本',
          tagColor: hasPartialSelection ? 'blue' : 'default',
          style: { marginBottom: 8 },
          action: (
            <Button
              size="small"
              icon={<FormOutlined />}
              onClick={handleOpenPartialRegenerate}
              disabled={!hasPartialSelection}
              title={hasPartialSelection ? '对选中文本执行局部重写' : '请先选中文本后再重写'}
            >
              {isMobile ? '重写' : '局部智能重写'}
            </Button>
          ),
        },
      )}
      <Card
        size="small"
        style={{
          marginBottom: 10,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: alphaColor(token.colorBgContainer, 0.98),
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Chapter Body
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8 }}>
          正文编辑区
        </Text>
        <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
          直接在这里调整正文。选中文本后，会在附近弹出局部智能重写工具条，方便就地改写片段。
        </Text>
        <Form.Item name="content" style={{ marginBottom: 0 }}>
          <TextArea
            ref={contentTextAreaRef}
            rows={isMobile ? 12 : 20}
            placeholder="请在这里编辑章节正文..."
            style={{
              fontFamily: 'monospace',
              fontSize: isMobile ? 12 : 14,
              lineHeight: 1.8,
              background: alphaColor(token.colorFillAlter, 0.72),
            }}
          />
        </Form.Item>
      </Card>

      <div data-partial-regenerate-toolbar>
        {partialRegenerateToolbarVisible && selectedTextForRegenerate ? (
          <PartialRegenerateToolbar
            visible={partialRegenerateToolbarVisible}
            position={partialRegenerateToolbarPosition}
            selectedText={selectedTextForRegenerate}
            onRegenerate={handleOpenPartialRegenerate}
          />
        ) : null}
      </div>

      {partialRegenerateModalVisible && currentEditingChapterId ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Partial Regenerate"
              title="正在整理局部重写工作区"
              message="系统正在恢复选中文本、改写范围与应用入口，原有局部重生成与写回逻辑保持不变。"
              tags={[
                { label: '局部重写', color: 'orange' },
                { label: '选区上下文恢复中', color: 'processing' },
                { label: '应用逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyPartialRegenerateModal
            visible={partialRegenerateModalVisible}
            chapterId={currentEditingChapterId}
            selectedText={selectedTextForRegenerate}
            startPosition={selectionStartPosition}
            endPosition={selectionEndPosition}
            styleId={selectedStyleId}
            onClose={() => setPartialRegenerateModalVisible(false)}
            onApply={handleApplyPartialRegenerate}
          />
        </Suspense>
      ) : null}

      <Form.Item style={{ marginBottom: 0 }}>
        <div
          style={{
            display: 'flex',
            flexDirection: isMobile ? 'column' : 'row',
            justifyContent: 'space-between',
            alignItems: isMobile ? 'stretch' : 'center',
            gap: 12,
            padding: isMobile ? '12px 14px' : '14px 16px',
            borderRadius: 18,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: alphaColor(token.colorBgContainer, 0.98),
          }}
        >
          {renderCompactSelectionSummary(
            [
              {
                label: '选中',
                value: hasPartialSelection ? `已选 ${selectedRegenerateCount} 字用于重写` : '未选择任何文本',
                color: hasPartialSelection ? 'blue' : 'default',
              },
              {
                label: '模式',
                value: '局部智能重写',
                color: 'green',
              },
            ],
            { style: { marginBottom: 0, flex: 1, minWidth: 0 } },
          )}
          <Space.Compact style={{ width: isMobile ? '100%' : 'auto' }} block={isMobile}>
            <Button onClick={onCloseEditor}>取消</Button>
            <Button type="primary" htmlType="submit">保存更改</Button>
          </Space.Compact>
        </div>
      </Form.Item>
    </Form>
  );
}

export default memo(ChapterEditorModalContent, areEditorModalContentPropsEqual);
