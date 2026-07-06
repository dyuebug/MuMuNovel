import { Modal, Form, Input, InputNumber, Select, Tag, Space, Button, message, Divider, Card, Typography, theme } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useState, useEffect, useCallback, useRef } from 'react';
import type { ExpansionPlanData, Character } from '../types';
import { characterApi } from '../services/modularApi';
import { renderCompactSettingHint } from './storyCreationCommonUi';

const { TextArea } = Input;
const { Text } = Typography;

interface ExpansionPlanEditorProps {
  visible: boolean;
  planData: ExpansionPlanData | null;
  chapterSummary: string | null;
  projectId: string;
  onSave: (data: ExpansionPlanData & { summary?: string }) => Promise<void>;
  onCancel: () => void;
}

export default function ExpansionPlanEditor({
  visible,
  planData,
  chapterSummary,
  projectId,
  onSave,
  onCancel
}: ExpansionPlanEditorProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  
  // 关键事件标签输入
  const [keyEventInput, setKeyEventInput] = useState('');
  const [keyEvents, setKeyEvents] = useState<string[]>([]);
  
  // 角色列表和选择
  const [availableCharacters, setAvailableCharacters] = useState<Character[]>([]);
  const [characters, setCharacters] = useState<string[]>([]);
  const [loadingCharacters, setLoadingCharacters] = useState(false);
  const mountedRef = useRef(true);
  const renderCharacterStatusHint = (
    title: string,
    detail: string,
    tone: 'info' | 'warning' = 'info',
  ) => (
    <div style={{ padding: '10px 12px' }}>
      {renderCompactSettingHint(title, detail, {
        tone,
        style: {
          marginBottom: 0,
          padding: '10px 12px',
          borderRadius: 16,
          boxShadow: 'none',
        },
      })}
    </div>
  );
  const loadCharactersRequestIdRef = useRef(0);
  const submitRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      loadCharactersRequestIdRef.current += 1;
      submitRequestIdRef.current += 1;
    };
  }, []);

  // 加载项目角色列表
  const loadCharacters = useCallback(async () => {
    loadCharactersRequestIdRef.current += 1;
    const requestId = loadCharactersRequestIdRef.current;
    try {
      setLoadingCharacters(true);
      setAvailableCharacters([]); // 重置为空数组
      const response = await characterApi.getCharacters(projectId);
      if (!mountedRef.current || loadCharactersRequestIdRef.current !== requestId) {
        return;
      }
      console.log('加载到的角色数据:', response);
      
      // API返回的是 {total, items} 格式,需要提取items
      let chars: Character[] = [];
      if (Array.isArray(response)) {
        chars = response;
      } else if (response && typeof response === 'object' && 'items' in response) {
        const responseObj = response as { items?: Character[] };
        if (Array.isArray(responseObj.items)) {
          chars = responseObj.items;
        }
      } else {
        console.error('角色API返回格式异常:', response);
        message.warning('角色数据格式异常');
      }
      
      setAvailableCharacters(chars);
      console.log('设置的角色列表:', chars);
    } catch (error: unknown) {
      if (!mountedRef.current || loadCharactersRequestIdRef.current !== requestId) {
        return;
      }
      console.error('加载角色列表失败:', error);
      setAvailableCharacters([]);
      const err = error as Error;
      message.error('加载角色列表失败: ' + (err?.message || '未知错误'));
    } finally {
      if (mountedRef.current && loadCharactersRequestIdRef.current === requestId) {
        setLoadingCharacters(false);
      }
    }
  }, [projectId]);

  useEffect(() => {
    if (visible && projectId) {
      loadCharacters();
    }
  }, [visible, projectId, loadCharacters]);

  // 当planData或chapterSummary变化时更新状态
  useEffect(() => {
    if (visible) {
      if (planData) {
        setKeyEvents(planData.key_events || []);
        setCharacters(planData.character_focus || []);
        form.setFieldsValue({
          summary: chapterSummary || '',
          emotional_tone: planData.emotional_tone,
          narrative_goal: planData.narrative_goal,
          conflict_type: planData.conflict_type,
          estimated_words: planData.estimated_words
        });
      } else {
        // 重置状态
        setKeyEvents([]);
        setCharacters([]);
        form.setFieldsValue({
          summary: chapterSummary || ''
        });
      }
    }
  }, [planData, chapterSummary, form, visible]);

  const handleAddKeyEvent = () => {
    if (keyEventInput.trim()) {
      setKeyEvents([...keyEvents, keyEventInput.trim()]);
      setKeyEventInput('');
    }
  };

  const handleAddCharacter = (characterName: string) => {
    if (characterName && !characters.includes(characterName)) {
      setCharacters([...characters, characterName]);
    }
  };

  const handleSubmit = async () => {
    submitRequestIdRef.current += 1;
    const requestId = submitRequestIdRef.current;
    try {
      setLoading(true);
      const values = await form.validateFields();
      
      // 验证至少有一个关键事件
      if (keyEvents.length === 0) {
        if (!mountedRef.current || submitRequestIdRef.current !== requestId) {
          return;
        }
        message.warning('请至少添加一个关键事件');
        return;
      }
      
      // 验证至少有一个角色
      if (characters.length === 0) {
        if (!mountedRef.current || submitRequestIdRef.current !== requestId) {
          return;
        }
        message.warning('请至少添加一个涉及角色');
        return;
      }
      
      const updatedPlan: ExpansionPlanData & { summary?: string } = {
        summary: values.summary,
        key_events: keyEvents,
        character_focus: characters,
        emotional_tone: values.emotional_tone,
        narrative_goal: values.narrative_goal,
        conflict_type: values.conflict_type,
        estimated_words: values.estimated_words,
        scenes: planData?.scenes || null
      };
      
      await onSave(updatedPlan);
      if (!mountedRef.current || submitRequestIdRef.current !== requestId) {
        return;
      }
      // message.success('规划信息保存成功');
    } catch (error) {
      if (!mountedRef.current || submitRequestIdRef.current !== requestId) {
        return;
      }
      console.error('保存失败:', error);
      message.error('保存失败，请重试');
    } finally {
      if (mountedRef.current && submitRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  };

  const handleCancel = () => {
    form.resetFields();
    setKeyEvents([]);
    setCharacters([]);
    setKeyEventInput('');
    onCancel();
  };
  const expansionPlanGuideSteps = [
    '先用情节概要锁定这一章要讲什么，再继续补细节，不要一开始就把注意力散到所有字段上。',
    '再补齐关键事件和涉及角色，让章节节拍与人物线先站稳，再去调情绪、冲突和字数。',
    '最后才写叙事目标，把这章为什么存在、要推动什么说清楚，再正式保存规划。',
  ];
  const expansionPlanWorkspaceFocus = !chapterSummary
    ? {
        title: '先补齐这一章的情节概要',
        note: '当前还没有清晰的章节概要，更适合先说清主事件和推进方向，再继续细化关键事件与叙事目标。',
      }
    : keyEvents.length === 0
      ? {
          title: '优先补出本章必须发生的关键事件',
          note: '当前概要已经存在，但还没有节拍事件，适合先把这章真正要发生的事拆成可执行的关键节点。',
        }
      : characters.length === 0
        ? {
            title: '确认这一章真正需要出现的角色',
            note: '当前事件已经有了，但角色焦点还是空的，适合先补齐人物参与者，避免后续展开时人物线脱节。',
          }
        : {
            title: '收束本章的控制项与叙事目标',
            note: '当前概要、事件和角色都已具备，更适合继续校准情绪、冲突、字数和叙事目标，让章节规划更完整。',
          };

  return (
    <Modal
      title={(
        <div>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Chapter Planning Studio
          </Text>
          <Text strong style={{ display: 'block', fontSize: 18, marginBottom: 4 }}>
            编辑章节规划
          </Text>
          <Text type="secondary">
            调整本章的节奏、角色关注点与叙事目标，让展开结果更贴近你希望的章节落点。
          </Text>
        </div>
      )}
      open={visible}
      onCancel={handleCancel}
      width={760}
      centered
      footer={[
        <Button key="cancel" onClick={handleCancel} disabled={loading}>
          取消
        </Button>,
        <Button key="submit" type="primary" loading={loading} onClick={handleSubmit}>
          保存
        </Button>
      ]}
    >
      <Card
        size="small"
        style={{
          marginBottom: 18,
          borderRadius: 22,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.82)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Planning Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              本章规划工作台
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里的修改只影响章节规划内容，不会改变现有正文。我们把填写顺序前置，帮助你先锁定事件与角色，再细化节奏和目标。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {expansionPlanGuideSteps.map((item, index) => (
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
              {expansionPlanWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {expansionPlanWorkspaceFocus.note}
            </Text>
            <Space wrap size={[8, 8]}>
              <Tag color="blue">项目 ID：{projectId}</Tag>
              <Tag color="purple">关键事件：{keyEvents.length} 条</Tag>
              <Tag color="cyan">涉及角色：{characters.length} 位</Tag>
              {chapterSummary ? <Tag color="green">已有章节概要</Tag> : <Tag color="default">等待补充概要</Tag>}
            </Space>
          </div>
        </div>
      </Card>

      <Form
        form={form}
        layout="vertical"
        initialValues={{
          emotional_tone: '紧张激烈',
          conflict_type: '人物冲突',
          estimated_words: 3000
        }}
      >
        <Card
          size="small"
          style={{
            marginBottom: 16,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: alphaColor(token.colorBgContainer, 0.98),
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Chapter Summary
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            情节概要
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            用几句话明确这一章的主事件、推进方向和情绪落点，为后续详细规划提供统一锚点。
          </Text>
          <Form.Item
            label="情节概要"
            name="summary"
            tooltip="简要描述本章的主要情节和故事走向"
            style={{ marginBottom: 0 }}
          >
            <TextArea
              rows={3}
              placeholder="简要描述本章的主要情节，例如：主角遇到意外事件，开始了一段新的冒险..."
              maxLength={500}
              showCount
            />
          </Form.Item>
        </Card>

        <Divider orientation="left">详细规划</Divider>

        <Card
          size="small"
          style={{
            marginBottom: 16,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.46)} 100%)`,
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Key Events
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            关键事件
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            至少添加一个关键事件，按节拍把这一章真正要发生的事写清楚。
          </Text>
          <Form.Item
            label="关键事件"
            tooltip="至少添加一个关键事件"
            required
            style={{ marginBottom: 0 }}
          >
            <Space direction="vertical" style={{ width: '100%' }}>
              <Space.Compact style={{ width: '100%' }}>
                <Input
                  placeholder="输入关键事件后按回车或点击添加"
                  value={keyEventInput}
                  onChange={(e) => setKeyEventInput(e.target.value)}
                  onPressEnter={handleAddKeyEvent}
                />
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={handleAddKeyEvent}
                >
                  添加
                </Button>
              </Space.Compact>
              <Space wrap>
                {keyEvents.map((event, idx) => (
                  <Tag
                    key={idx}
                    closable
                    onClose={(e) => {
                      e.preventDefault();
                      setKeyEvents(keyEvents.filter((_, i) => i !== idx));
                    }}
                    color="purple"
                    style={{ marginBottom: 8, padding: '5px 9px', borderRadius: 999 }}
                  >
                    <span style={{ fontWeight: 'bold', marginRight: 4 }}>#{idx + 1}</span>
                    {event}
                  </Tag>
                ))}
              </Space>
            </Space>
          </Form.Item>
        </Card>

        <Card
          size="small"
          style={{
            marginBottom: 16,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: alphaColor(token.colorBgContainer, 0.98),
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Character Focus
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            涉及角色
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            从项目角色池里挑出本章真正需要被看见的人，避免章节目标和人物线脱节。
          </Text>
          <Form.Item
            label="涉及角色"
            tooltip="从项目现有角色中选择"
            required
            style={{ marginBottom: 0 }}
          >
            <Space direction="vertical" style={{ width: '100%' }}>
              <Select
                placeholder="选择角色"
                style={{ width: '100%' }}
                loading={loadingCharacters}
                onChange={handleAddCharacter}
                value={undefined}
                showSearch
                optionFilterProp="children"
                filterOption={(input, option) =>
                  (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
                }
                options={Array.isArray(availableCharacters)
                  ? availableCharacters
                      .filter(char => !characters.includes(char.name))
                      .map(char => ({
                        label: char.name,
                        value: char.name,
                      }))
                  : []}
                notFoundContent={
                  loadingCharacters
                    ? renderCharacterStatusHint(
                        '角色候选正在返回',
                        '角色池正在同步到当前大纲编辑器，稍等片刻后就可以继续给这章补齐关键人物。',
                      )
                    : !Array.isArray(availableCharacters)
                      ? renderCharacterStatusHint(
                          '暂时没能载入角色池',
                          '可以稍后重试，或先回到角色管理确认项目角色是否已整理完成。',
                          'warning',
                        )
                      : availableCharacters.length === 0
                        ? renderCharacterStatusHint(
                            '当前项目还没有角色',
                            '建议先在角色管理里建立基础角色档案，再回到这里为章节绑定关键人物。',
                            'warning',
                          )
                        : renderCharacterStatusHint(
                            '可选角色已经全部加入',
                            '这章当前涉及的人物已经补齐；如果还需要新增角色，可以先到角色管理补档。',
                          )
                }
              />
              <Space wrap>
                {characters.map((char, idx) => (
                  <Tag
                    key={idx}
                    closable
                    onClose={() => setCharacters(characters.filter((_, i) => i !== idx))}
                    color="cyan"
                    style={{ padding: '5px 9px', borderRadius: 999 }}
                  >
                    {char}
                  </Tag>
                ))}
              </Space>
            </Space>
          </Form.Item>
        </Card>

        <Card
          size="small"
          style={{
            marginBottom: 16,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: alphaColor(token.colorBgContainer, 0.98),
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Story Controls
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            节奏与控制项
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            用三个关键字段锁定本章的情绪、冲突和容量，让展开后的内容更稳定。
          </Text>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
              gap: 12,
            }}
          >
            <Form.Item
              label="情感基调"
              name="emotional_tone"
              rules={[{ required: true, message: '请输入情感基调' }]}
              tooltip="例如：紧张激烈、温馨感人、悬疑惊悚等"
              style={{ marginBottom: 0 }}
            >
              <Input
                placeholder="输入情感基调，例如：紧张激烈、温馨感人等"
                maxLength={20}
              />
            </Form.Item>

            <Form.Item
              label="冲突类型"
              name="conflict_type"
              rules={[{ required: true, message: '请输入冲突类型' }]}
              tooltip="例如：人物冲突、内心冲突、环境冲突等"
              style={{ marginBottom: 0 }}
            >
              <Input
                placeholder="输入冲突类型，例如：人物冲突、内心冲突等"
                maxLength={20}
              />
            </Form.Item>

            <Form.Item
              label="预估字数"
              name="estimated_words"
              rules={[{ required: true, message: '请输入预估字数' }]}
              style={{ marginBottom: 0 }}
            >
              <InputNumber
                min={500}
                max={10000}
                step={100}
                style={{ width: '100%' }}
                formatter={(value) => `${value} 字`}
                parser={(value) => Number(value?.replace(' 字', '')) as 500 | 10000}
              />
            </Form.Item>
          </div>
        </Card>

        <Card
          size="small"
          style={{
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: alphaColor(token.colorBgContainer, 0.98),
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Narrative Goal
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            叙事目标
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            最后明确这一章“为什么存在”，它应该推动什么、揭示什么、留下什么余波。
          </Text>
          <Form.Item
            label="叙事目标"
            name="narrative_goal"
            rules={[{ required: true, message: '请输入叙事目标' }]}
            style={{ marginBottom: 0 }}
          >
            <TextArea
              rows={3}
              placeholder="描述本章要达成的叙事目标，例如：推进主线剧情、深化角色关系、揭示重要信息等..."
              maxLength={500}
              showCount
            />
          </Form.Item>
        </Card>
      </Form>
    </Modal>
  );
}
