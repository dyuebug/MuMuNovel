import React, { useEffect, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Card,
  Divider,
  Input,
  InputNumber,
  Modal,
  Radio,
  Space,
  Spin,
  Switch,
  Typography,
  message,
  theme,
} from 'antd';
import {
  CheckOutlined,
  EditOutlined,
  LoadingOutlined,
  ReloadOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { chapterApi } from '../services/modularApi';

const { TextArea } = Input;
const { Paragraph, Text } = Typography;

const PARTIAL_REGENERATE_STREAM_INACTIVITY_TIMEOUT_MS = 90000;
const PARTIAL_REGENERATE_HEARTBEAT_SUFFIX = '（仍在生成中）';

type LengthMode = 'similar' | 'expand' | 'condense' | 'custom';

type RenderDebugGlobal = typeof globalThis & {
  __NOVEL_RENDER_DEBUG__?: boolean;
  __NOVEL_RENDER_DEBUG_FILTER__?: string[];
};

interface PartialRegenerateModalProps {
  visible: boolean;
  chapterId: string;
  selectedText: string;
  startPosition: number;
  endPosition: number;
  styleId?: number;
  onClose: () => void;
  onApply: (newText: string, startPosition: number, endPosition: number) => void;
}

const noopRenderDiagnostics = (...args: [string, () => Record<string, unknown>]): void => {
  void args;
};

const appendPartialRegenerateHeartbeatHint = (nextMessage: string) => {
  const normalized = nextMessage.trim();
  if (!normalized) {
    return `正在持续生成...${PARTIAL_REGENERATE_HEARTBEAT_SUFFIX}`;
  }

  if (normalized.endsWith(PARTIAL_REGENERATE_HEARTBEAT_SUFFIX)) {
    return normalized;
  }

  return `${normalized}${PARTIAL_REGENERATE_HEARTBEAT_SUFFIX}`;
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

const getLengthModeDescription = (mode: LengthMode): string => {
  const descriptions: Record<LengthMode, string> = {
    similar: '保持与原文相近的篇幅与节奏。',
    expand: '扩展细节与铺陈，适合加强氛围、动作或心理描写。',
    condense: '压缩冗余表达，保留核心信息与情绪。',
    custom: '按你指定的目标字数来重写。',
  };

  return descriptions[mode];
};

export const PartialRegenerateModal: React.FC<PartialRegenerateModalProps> = ({
  visible,
  chapterId,
  selectedText,
  startPosition,
  endPosition,
  styleId,
  onClose,
  onApply,
}) => {
  const { token } = theme.useToken();
  const [userInstructions, setUserInstructions] = useState('');
  const [lengthMode, setLengthMode] = useState<LengthMode>('similar');
  const [customWordCount, setCustomWordCount] = useState<number>(Math.max(selectedText.length, 10));
  const [enableWebResearch, setEnableWebResearch] = useState(false);
  const [webResearchQuery, setWebResearchQuery] = useState('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [generatedText, setGeneratedText] = useState('');
  const [hasGenerated, setHasGenerated] = useState(false);
  const [progress, setProgress] = useState(0);
  const [progressMessage, setProgressMessage] = useState('');
  const abortControllerRef = useRef<AbortController | null>(null);
  const generatedTextScrollRef = useRef<HTMLDivElement>(null);
  const generatedTextValueRef = useRef('');
  const mountedRef = useRef(true);
  const generateRequestIdRef = useRef(0);
  const applyRequestIdRef = useRef(0);

  useLocalRenderDiagnostics('PartialRegenerateModal', () => ({
    visible,
    chapterId,
    selectedTextLength: selectedText.length,
    generatedTextLength: generatedText.length,
    enableWebResearch,
    webResearchQueryLength: webResearchQuery.length,
    isGenerating,
    hasGenerated,
    progress,
    progressMessage,
  }));

  useEffect(() => {
    mountedRef.current = true;
    if (!visible) {
      return;
    }

    setUserInstructions('');
    setLengthMode('similar');
    setCustomWordCount(Math.max(selectedText.length, 10));
    setEnableWebResearch(false);
    setWebResearchQuery('');
    setIsGenerating(false);
    setGeneratedText('');
    generatedTextValueRef.current = '';
    setHasGenerated(false);
    setProgress(0);
    setProgressMessage('');
    abortControllerRef.current = null;
  }, [visible, selectedText.length]);

  useEffect(() => {
    if (generatedTextScrollRef.current && isGenerating) {
      generatedTextScrollRef.current.scrollTop = generatedTextScrollRef.current.scrollHeight;
    }
  }, [generatedText, isGenerating]);

  useEffect(() => () => {
    mountedRef.current = false;
    generateRequestIdRef.current += 1;
    applyRequestIdRef.current += 1;
    abortControllerRef.current?.abort();
  }, []);

  const handleGenerate = async () => {
    if (!userInstructions.trim()) {
      message.warning('请输入重写要求');
      return;
    }

    generateRequestIdRef.current += 1;
    const requestId = generateRequestIdRef.current;
    setIsGenerating(true);
    setGeneratedText('');
    generatedTextValueRef.current = '';
    setHasGenerated(false);
    setProgress(0);
    setProgressMessage('准备生成...');
    abortControllerRef.current = new AbortController();

    try {
      const result = await chapterApi.partialRegenerateInBackground(
        chapterId,
        {
          selected_text: selectedText,
          start_position: startPosition,
          end_position: endPosition,
          user_instructions: userInstructions.trim(),
          context_chars: 500,
          style_id: styleId,
          length_mode: lengthMode,
          target_word_count: lengthMode === 'custom' ? customWordCount : undefined,
          enable_web_research: enableWebResearch,
          web_research_query: enableWebResearch && webResearchQuery.trim()
            ? webResearchQuery.trim()
            : undefined,
        },
        {
          inactivityTimeoutMs: PARTIAL_REGENERATE_STREAM_INACTIVITY_TIMEOUT_MS,
          signal: abortControllerRef.current.signal,
          onProgress: (nextMessage, nextProgress) => {
            if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
              return;
            }
            setProgress(nextProgress);
            setProgressMessage(nextMessage);
          },
          onHeartbeat: () => {
            if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
              return;
            }
            setProgressMessage((previous) => appendPartialRegenerateHeartbeatHint(previous || '正在持续生成...'));
          },
          onResult: (data) => {
            if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
              return;
            }
            const nextText = data.new_text || '';
            generatedTextValueRef.current = nextText;
            setGeneratedText(nextText);
            setProgress(100);
            setProgressMessage('生成完成');
            setHasGenerated(true);
          },
          onError: (error) => {
            if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
              return;
            }
            console.error('局部重写 SSE 错误:', error);
            message.error(error || '生成过程中发生错误');
            setIsGenerating(false);
            setHasGenerated(generatedTextValueRef.current.trim().length > 0);
          },
          onComplete: () => {
            if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
              return;
            }
            setIsGenerating(false);
            setHasGenerated((current) => current || generatedTextValueRef.current.trim().length > 0);
            abortControllerRef.current = null;
          },
        }
      );
      if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
        return;
      }
      const nextText = result.new_text || '';
      generatedTextValueRef.current = nextText;
      setGeneratedText(nextText);
      setProgress(100);
      setProgressMessage('生成完成');
      setHasGenerated(nextText.trim().length > 0);
      setIsGenerating(false);
      abortControllerRef.current = null;
    } catch (error) {
      if (!mountedRef.current || generateRequestIdRef.current !== requestId) {
        return;
      }
      console.error('局部重写生成失败:', error);
      if ((error as Error).name !== 'AbortError') {
        message.error('生成失败，请重试');
      }
      setIsGenerating(false);
      abortControllerRef.current = null;
    }
  };

  const handleCancel = () => {
    if (isGenerating && abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
      setIsGenerating(false);
      message.info('已取消生成');
    }

    onClose();
  };

  const handleAccept = async () => {
    if (!generatedText.trim()) {
      message.warning('没有可应用的内容');
      return;
    }

    applyRequestIdRef.current += 1;
    const requestId = applyRequestIdRef.current;
    try {
      await chapterApi.applyPartialRegenerate(chapterId, {
        new_text: generatedText,
        start_position: startPosition,
        end_position: endPosition,
      });
      if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
        return;
      }

      message.success('已应用重写内容');
      onApply(generatedText, startPosition, endPosition);
      onClose();
    } catch (error) {
      if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
        return;
      }
      console.error('应用局部重写失败:', error);
      message.error('应用失败，请重试');
    }
  };

  const handleRegenerate = () => {
    setGeneratedText('');
    generatedTextValueRef.current = '';
    setHasGenerated(false);
    setProgress(0);
    setProgressMessage('');
    void handleGenerate();
  };

  const canStartGenerate = Boolean(userInstructions.trim())
    && (lengthMode !== 'custom' || customWordCount >= 10);
  const partialRewriteGuideSteps = [
    '先读清这段原文，再决定这一轮是保留原节奏、扩展细节，还是做更紧凑的局部改写。',
    '再补充重写要求、长度策略和联网检索，把“局部重写边界”在提交前说清楚。',
    '最后再开始生成；拿到结果后先比较字数与表达方向，再决定是否接受或继续重写。',
  ];
  const partialRewriteWorkspaceFocus = isGenerating
    ? {
        title: '跟进当前局部重写进度',
        note: '当前后台生成已经启动，适合先观察进度、生成提示和结果文本，不要同时继续改动这一轮输入条件。',
      }
    : hasGenerated && generatedText.trim()
      ? {
          title: '复核这一版局部重写结果',
          note: '当前已经得到新的片段文本，适合先比较长度变化、语气和信息密度，再决定是否接受并应用到正文。',
        }
      : enableWebResearch
        ? {
            title: '确认是否需要外部资料支撑这段改写',
            note: '当前已开启联网检索，更适合用在职业、时代、规则或场景细节明确的片段，不必给普通润色片段增加多余上下文。',
          }
        : !userInstructions.trim()
          ? {
              title: '先定义这段文字的改写目标',
              note: '当前还没有输入重写要求，适合先说清楚你想强化的是氛围、动作、情绪还是节奏，再提交这一轮局部重写。',
            }
          : {
              title: `按“${lengthMode === 'similar' ? '保持长度' : lengthMode === 'expand' ? '扩展内容' : lengthMode === 'condense' ? '精简内容' : '自定义字数'}”推进本段重写`,
              note: '当前已经具备可提交的重写条件，适合先确认要求与长度策略是否一致，再把这一段交给现有局部重写链路。',
            };

  return (
    <Modal
      title={(
        <Space>
          <EditOutlined style={{ color: token.colorPrimary }} />
          <span>局部智能重写</span>
        </Space>
      )}
      open={visible}
      onCancel={handleCancel}
      width="min(880px, calc(100vw - 32px))"
      centered
      maskClosable={!isGenerating}
      closable={!isGenerating}
      keyboard={!isGenerating}
      footer={(
        <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
          <Button onClick={handleCancel} disabled={isGenerating}>
            取消
          </Button>
          {!hasGenerated ? (
            <Button
              type="primary"
              icon={isGenerating ? <LoadingOutlined /> : <ThunderboltOutlined />}
              onClick={() => void handleGenerate()}
              loading={isGenerating}
              disabled={!canStartGenerate}
              style={{
                background: `linear-gradient(135deg, ${token.colorPrimary} 0%, ${token.colorPrimaryHover} 100%)`,
                border: 'none',
                boxShadow: token.boxShadowSecondary,
              }}
            >
              {isGenerating ? '生成中...' : '开始重写'}
            </Button>
          ) : (
            <>
              <Button icon={<ReloadOutlined />} onClick={handleRegenerate}>
                重新生成
              </Button>
              <Button
                type="primary"
                icon={<CheckOutlined />}
                onClick={() => void handleAccept()}
                style={{ background: token.colorSuccess, borderColor: token.colorSuccess }}
              >
                接受并应用
              </Button>
            </>
          )}
        </Space>
      )}
      styles={{
        body: {
          maxHeight: 'calc(100dvh - 220px)',
          overflowY: 'auto',
          overflowX: 'hidden',
          paddingTop: 12,
        },
      }}
    >
      <Card
        size="small"
        title={(
          <Space>
            <Text strong>原文内容</Text>
            <Text type="secondary">{selectedText.length} 字</Text>
          </Space>
        )}
        style={{ marginBottom: 16 }}
        styles={{
          body: {
            maxHeight: 120,
            overflowY: 'auto',
            background: token.colorFillAlter,
          },
        }}
      >
        <Paragraph
          style={{
            margin: 0,
            whiteSpace: 'pre-wrap',
            color: token.colorText,
            lineHeight: 1.8,
          }}
        >
          {selectedText}
        </Paragraph>
      </Card>

      <Card
        size="small"
        style={{
          marginBottom: 16,
          borderRadius: 22,
          border: `1px solid ${token.colorBorderSecondary}`,
          background: `linear-gradient(135deg, ${token.colorPrimaryBg} 0%, ${token.colorBgContainer} 100%)`,
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
              Partial Rewrite Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              局部重写工作台
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里不改变原有的局部重写请求、进度回流和应用逻辑，只把输入顺序与结果判断重点提前说明，帮助你更稳定地处理这一小段正文。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {partialRewriteGuideSteps.map((item, index) => (
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
              background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              当前工作焦点
            </Text>
            <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
              {partialRewriteWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {partialRewriteWorkspaceFocus.note}
            </Text>
            <Space wrap size={[8, 8]}>
              <Text type="secondary">{selectedText.length} 字原文</Text>
              <Text type="secondary">{userInstructions.length} 字要求</Text>
              <Text type="secondary">{enableWebResearch ? '联网检索已开启' : '联网检索已关闭'}</Text>
            </Space>
          </div>
        </div>
      </Card>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
          gap: 16,
        }}
      >
        <Card size="small" title="重写要求" styles={{ body: { padding: 16 } }}>
          <Space direction="vertical" size={8} style={{ width: '100%' }}>
            <Text strong>
              描述希望如何改写 <Text type="danger">*</Text>
            </Text>
            <TextArea
              value={userInstructions}
              onChange={(event) => setUserInstructions(event.target.value)}
              placeholder={[
                '例如：',
                '- 让描写更生动细腻',
                '- 增加环境氛围描写',
                '- 强化角色心理活动',
                '- 调整节奏，让冲突更紧凑',
              ].join('\n')}
              rows={6}
              disabled={isGenerating}
              style={{ resize: 'none' }}
              showCount
              maxLength={600}
            />
          </Space>
        </Card>

        <Card size="small" title="生成设置" styles={{ body: { padding: 16 } }}>
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <div>
              <Text strong style={{ display: 'block', marginBottom: 8 }}>
                长度控制
              </Text>
              <Radio.Group
                value={lengthMode}
                onChange={(event) => setLengthMode(event.target.value)}
                disabled={isGenerating}
                buttonStyle="solid"
                style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}
              >
                <Radio.Button value="similar">保持长度</Radio.Button>
                <Radio.Button value="expand">扩展内容</Radio.Button>
                <Radio.Button value="condense">精简内容</Radio.Button>
                <Radio.Button value="custom">自定义</Radio.Button>
              </Radio.Group>
              <div style={{ marginTop: 8 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {getLengthModeDescription(lengthMode)}
                </Text>
              </div>
              {lengthMode === 'custom' && (
                <Space style={{ marginTop: 12 }} align="center">
                  <Text>目标字数</Text>
                  <InputNumber
                    value={customWordCount}
                    onChange={(value) => setCustomWordCount(Number(value) || Math.max(selectedText.length, 10))}
                    min={10}
                    max={10000}
                    step={50}
                    disabled={isGenerating}
                    addonAfter="字"
                    style={{ width: 160 }}
                  />
                </Space>
              )}
            </div>

            <Divider style={{ margin: '0' }} />

            <div>
              <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
                <Text strong>联网检索</Text>
                <Switch
                  checked={enableWebResearch}
                  onChange={setEnableWebResearch}
                  checkedChildren="开启"
                  unCheckedChildren="关闭"
                  disabled={isGenerating}
                />
              </Space>
              <div style={{ marginTop: 8 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  开启后会先补充外部参考资料，再将结果注入局部重写提示词，更适合职业、时代、场景等细节型片段。
                </Text>
              </div>
              {enableWebResearch && (
                <div style={{ marginTop: 12 }}>
                  <Text strong style={{ display: 'block', marginBottom: 8 }}>
                    联网检索查询词
                  </Text>
                  <TextArea
                    value={webResearchQuery}
                    onChange={(event) => setWebResearchQuery(event.target.value)}
                    rows={3}
                    disabled={isGenerating}
                    placeholder="可留空，系统会根据当前选中文本与上下文自动生成查询词。也可以手动指定，例如：民国刑警办案流程、夜市摊贩吆喝细节。"
                    maxLength={300}
                    showCount
                    style={{ resize: 'none' }}
                  />
                </div>
              )}
            </div>
          </Space>
        </Card>
      </div>

      <Divider style={{ margin: '16px 0' }} />

      {(isGenerating || hasGenerated) && (
        <div>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 8,
            }}
          >
            <Space>
              <Text strong>重写结果</Text>
              {generatedText && <Text type="secondary">{generatedText.length} 字</Text>}
            </Space>
            {isGenerating && (
              <Space>
                <Spin indicator={<LoadingOutlined style={{ fontSize: 14 }} spin />} />
                <Text type="secondary">{progressMessage || '生成中...'}</Text>
              </Space>
            )}
          </div>

          {isGenerating && (
            <div style={{ marginBottom: 12 }}>
              <div
                style={{
                  height: 4,
                  background: token.colorFillTertiary,
                  borderRadius: 2,
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    height: '100%',
                    background: `linear-gradient(90deg, ${token.colorPrimary} 0%, ${token.colorPrimaryHover} 100%)`,
                    width: `${progress}%`,
                    transition: 'width 0.3s ease',
                    borderRadius: 2,
                  }}
                />
              </div>
            </div>
          )}

          <Card
            size="small"
            ref={generatedTextScrollRef}
            style={{
              background: generatedText ? token.colorSuccessBg : token.colorFillAlter,
              border: generatedText ? `1px solid ${token.colorSuccessBorder}` : `1px solid ${token.colorBorder}`,
            }}
            styles={{
              body: {
                maxHeight: 260,
                overflowY: 'auto',
                minHeight: 120,
              },
            }}
          >
            {generatedText ? (
              <Paragraph
                style={{
                  margin: 0,
                  whiteSpace: 'pre-wrap',
                  lineHeight: 1.8,
                }}
              >
                {generatedText}
                {isGenerating && (
                  <span
                    style={{
                      display: 'inline-block',
                      width: 8,
                      height: 16,
                      background: token.colorPrimary,
                      marginLeft: 2,
                      animation: 'blink 1s infinite',
                    }}
                  />
                )}
              </Paragraph>
            ) : (
              <div style={{ textAlign: 'center', padding: 20, color: token.colorTextTertiary }}>
                {isGenerating ? '正在生成内容...' : '等待生成...'}
              </div>
            )}
          </Card>

          {hasGenerated && generatedText && (
            <Alert
              message="生成完成"
              description={(
                <span>
                  原文 {selectedText.length} 字 → 新文 {generatedText.length} 字
                  {generatedText.length > selectedText.length && (
                    <Text type="success">（+{generatedText.length - selectedText.length} 字）</Text>
                  )}
                  {generatedText.length < selectedText.length && (
                    <Text type="warning">（{generatedText.length - selectedText.length} 字）</Text>
                  )}
                </span>
              )}
              type="success"
              showIcon
              style={{ marginTop: 12 }}
            />
          )}
        </div>
      )}

      <style>{`
        @keyframes blink {
          0%, 50% { opacity: 1; }
          51%, 100% { opacity: 0; }
        }
      `}</style>
    </Modal>
  );
};

export default PartialRegenerateModal;
