/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy, memo, useCallback, useEffect, useRef, useState } from 'react';
import { Button, Card, Form, Input, Select, Space, message } from 'antd';
import { FundOutlined, FormOutlined, LockOutlined, ThunderboltOutlined } from '@ant-design/icons';
import PartialRegenerateToolbar from './PartialRegenerateToolbar';
import ChapterEditorAiSection from './ChapterEditorAiSection';
import {
  renderCompactSelectionSummary,
  renderCompactSettingFlow,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';
import { CREATION_PLOT_STAGE_OPTIONS } from '../utils/creationPresetsCore';
import type { TextAreaRef } from 'antd/es/input/TextArea';

const { TextArea } = Input;

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
      return '????';
    case 'third_person':
      return '????';
    case 'omniscient':
      return '????';
    default:
      return '????';
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
  const currentEditingChapterId = currentEditingChapter?.id ?? null;
  const [partialRegenerateToolbarVisible, setPartialRegenerateToolbarVisible] = useState(false);
  const [partialRegenerateToolbarPosition, setPartialRegenerateToolbarPosition] = useState<ToolbarPosition>({ top: 0, left: 0 });
  const [selectedTextForRegenerate, setSelectedTextForRegenerate] = useState('');
  const [selectionStartPosition, setSelectionStartPosition] = useState(0);
  const [selectionEndPosition, setSelectionEndPosition] = useState(0);
  const [partialRegenerateModalVisible, setPartialRegenerateModalVisible] = useState(false);

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
    message.success('???????????');
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

  const updateToolbarPosition = useCallback(() => {
    if (!partialRegenerateToolbarVisible || !selectedTextForRegenerate) {
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
      window.setTimeout(handleTextSelection, 50);
    };
    const handleKeyUp = () => {
      window.setTimeout(handleTextSelection, 50);
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
      textArea.removeEventListener('mouseup', handleMouseUp);
      textArea.removeEventListener('keyup', handleKeyUp);
      textArea.removeEventListener('scroll', handleScroll);
      if (modalBody) {
        modalBody.removeEventListener('scroll', handleScroll);
      }
      window.removeEventListener('resize', handleScroll);
    };
  }, [handleTextSelection, updateToolbarPosition]);

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
    setPartialRegenerateToolbarVisible(false);
    setPartialRegenerateModalVisible(false);
    setSelectedTextForRegenerate('');
    setSelectionStartPosition(0);
    setSelectionEndPosition(0);
  }, [currentEditingChapterId]);

  const selectedRegenerateCount = selectedTextForRegenerate.trim().length;
  const hasPartialSelection = selectedRegenerateCount > 0;

  return (
    <Form form={editorForm} layout="vertical" onFinish={handleEditorSubmit}>
      <Form.Item
        label="????"
        tooltip="????????????"
        style={{ marginBottom: isMobile ? 16 : 12 }}
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
                title={!currentEditingCanGenerate ? currentEditingGenerateDisabledReason : '????????'}
              >
                {isMobile ? '??' : '????'}
              </Button>
              <Button
                icon={<FundOutlined />}
                onClick={() => handleShowAnalysis(currentEditingChapter.id)}
                disabled={!canAnalyzeCurrentChapter}
                title={canAnalyzeCurrentChapter ? '????????????' : '??????????'}
              >
                {isMobile ? '??' : '????'}
              </Button>
            </>
          ) : null}
        </Space.Compact>
      </Form.Item>

      {renderCompactSettingFlow(
        '???????????????????????????',
        '?????????????????????????????????',
        ['????', '????', '????', '????'],
      )}

      <Card size="small" title="????" style={{ marginBottom: 12 }}>
        <div
          style={{
            display: isMobile ? 'block' : 'flex',
            gap: isMobile ? 0 : 16,
            marginBottom: 0,
          }}
        >
          <Form.Item
            label="????"
            tooltip="????????????????????"
            required
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder="???????"
              value={selectedStyleId}
              onChange={setSelectedStyleId}
              status={!selectedStyleId ? 'error' : undefined}
            >
              {writingStyles.map((style: any) => (
                <Select.Option key={style.id} value={style.id}>
                  {style.name}
                  {style.is_default ? ' (??)' : ''}
                </Select.Option>
              ))}
            </Select>
            {!selectedStyleId ? (
              <div style={{ color: '#ff4d4f', fontSize: 12, marginTop: 4 }}>???????</div>
            ) : null}
          </Form.Item>

          <Form.Item
            label="????"
            tooltip="????????????????/?????????"
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder={`???????${getNarrativePerspectiveText(currentProjectNarrativePerspective)}`}
              value={temporaryNarrativePerspective}
              onChange={setTemporaryNarrativePerspective}
              allowClear
            >
              <Select.Option value="first_person">????</Select.Option>
              <Select.Option value="third_person">????</Select.Option>
              <Select.Option value="omniscient">????</Select.Option>
            </Select>
            {temporaryNarrativePerspective ? (
              <div style={{ color: 'var(--color-success)', fontSize: 12, marginTop: 4 }}>
                ?????{getNarrativePerspectiveText(temporaryNarrativePerspective)}
              </div>
            ) : null}
          </Form.Item>

          <Form.Item
            label="????"
            tooltip="????????????????????"
            style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
          >
            <Select
              placeholder="???????"
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
              <Button size="small" onClick={applyInferredSinglePlotStage}>??????</Button>
              {selectedPlotStage ? (
                <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                  ????{CREATION_PLOT_STAGE_OPTIONS.find((item: any) => item.value === selectedPlotStage)?.label || selectedPlotStage}
                </span>
              ) : null}
            </Space>
          </Form.Item>
        </div>
      </Card>

      <ChapterEditorAiSection sectionProps={aiSectionProps} />

      {renderCompactStoryControlHeader(
        '????',
        hasPartialSelection
          ? `?? ${selectedRegenerateCount} ?????????????`
          : '????????????????????',
        {
          tagText: hasPartialSelection ? `?? ${selectedRegenerateCount} ?` : '??????',
          tagColor: hasPartialSelection ? 'blue' : 'default',
          style: { marginBottom: 8 },
          action: (
            <Button
              size="small"
              icon={<FormOutlined />}
              onClick={handleOpenPartialRegenerate}
              disabled={!hasPartialSelection}
              title={hasPartialSelection ? '?????????????' : '?????????????'}
            >
              {isMobile ? '????' : '???????'}
            </Button>
          ),
        },
      )}
      <Form.Item name="content" style={{ marginBottom: 10 }}>
        <TextArea
          ref={contentTextAreaRef}
          rows={isMobile ? 12 : 20}
          placeholder="???????..."
          style={{ fontFamily: 'monospace', fontSize: isMobile ? 12 : 14 }}
        />
      </Form.Item>

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
        <Suspense fallback={null}>
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
          }}
        >
          {renderCompactSelectionSummary(
            [
              {
                label: '??',
                value: hasPartialSelection ? `?? ${selectedRegenerateCount} ???????` : '??????????',
                color: hasPartialSelection ? 'blue' : 'default',
              },
              {
                label: '??',
                value: '???????',
                color: 'green',
              },
            ],
            { style: { marginBottom: 0, flex: 1, minWidth: 0 } },
          )}
          <Space.Compact style={{ width: isMobile ? '100%' : 'auto' }} block={isMobile}>
            <Button onClick={onCloseEditor}>??</Button>
            <Button type="primary" htmlType="submit">????</Button>
          </Space.Compact>
        </div>
      </Form.Item>
    </Form>
  );
}

export default memo(ChapterEditorModalContent, areEditorModalContentPropsEqual);
