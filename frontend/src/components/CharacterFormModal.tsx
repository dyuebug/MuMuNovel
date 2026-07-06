import { Button, Card, Col, Divider, Form, Input, InputNumber, Modal, Row, Select, Space, Tag, Typography, theme } from 'antd';
import type { FormInstance } from 'antd';
import type { Character } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

type CareerOption = {
  id: string;
  name: string;
  max_stage: number;
};

type CharacterFormValues = {
  name: string;
  age?: string;
  gender?: string;
  role_type?: string;
  personality?: string;
  appearance?: string;
  background?: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_career_data?: Array<{
    career_id: string;
    stage: number;
  }>;
  organization_type?: string;
  organization_purpose?: string;
  organization_members?: string;
  power_level?: number;
  location?: string;
  motto?: string;
  color?: string;
};

type CharacterFormModalProps = {
  open: boolean;
  title: string;
  mode: 'create' | 'edit';
  entityType: 'character' | 'organization';
  form: FormInstance<CharacterFormValues>;
  isMobile: boolean;
  record?: Character | null;
  mainCareers: CareerOption[];
  subCareers: CareerOption[];
  submitText: string;
  onCancel: () => void;
  onFinish: (values: CharacterFormValues) => void | Promise<void>;
};

const { TextArea } = Input;
const characterFormGuideSteps = [
  '先确认当前是在创建还是编辑角色/组织，再决定这次优先补基础身份还是世界观细节。',
  '再填写上方核心字段，把名称、定位、职业或组织性质作为这次表单的主线信息。',
  '最后再补背景、外貌、宗旨等扩展内容，提交前回看右侧焦点卡确认这次工作重点是否完整。',
];

function renderCharacterFields(
  mode: 'create' | 'edit',
  form: FormInstance<CharacterFormValues>,
  record: Character | null | undefined,
  token: ReturnType<typeof theme.useToken>['token'],
  mainCareers: CareerOption[],
  subCareers: CareerOption[],
) {
  return (
    <>
      <Row gutter={12}>
        <Col span={8}>
          <Form.Item
            label="名称"
            name="name"
            rules={[{ required: true, message: '请输入角色名称' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="例如：林渊 / 苏璃 / 阿迟" />
          </Form.Item>
        </Col>
        <Col span={6}>
          <Form.Item
            label="角色类型"
            name="role_type"
            initialValue={mode === 'create' ? 'supporting' : undefined}
            style={{ marginBottom: 12 }}
          >
            <Select>
              <Select.Option value="protagonist">主角</Select.Option>
              <Select.Option value="supporting">配角</Select.Option>
              <Select.Option value="antagonist">反派</Select.Option>
            </Select>
          </Form.Item>
        </Col>
        <Col span={5}>
          <Form.Item label="年龄" name="age" style={{ marginBottom: 12 }}>
            <Input placeholder="例如：25岁" />
          </Form.Item>
        </Col>
        <Col span={5}>
          <Form.Item label="性别" name="gender" style={{ marginBottom: 12 }}>
            <Select placeholder="请选择">
              <Select.Option value="男">男</Select.Option>
              <Select.Option value="女">女</Select.Option>
              <Select.Option value="其他">其他</Select.Option>
            </Select>
          </Form.Item>
        </Col>
      </Row>

      <Row gutter={12}>
        <Col span={12}>
          <Form.Item label="性格特点" name="personality" style={{ marginBottom: 12 }}>
            <TextArea rows={2} placeholder="角色的性格、习惯与处事方式..." />
          </Form.Item>
        </Col>
        <Col span={12}>
          <Form.Item label="外貌特征" name="appearance" style={{ marginBottom: 12 }}>
            <TextArea rows={2} placeholder="外形、服饰、气质或辨识特征..." />
          </Form.Item>
        </Col>
      </Row>

      {mode === 'edit' && record?.relationships ? (
        <Form.Item label="当前关系摘要" style={{ marginBottom: 12 }}>
          <Input.TextArea
            value={record.relationships}
            readOnly
            autoSize={{ minRows: 1, maxRows: 3 }}
            style={{ backgroundColor: token.colorFillTertiary, cursor: 'default' }}
          />
        </Form.Item>
      ) : null}

      <Form.Item label="背景故事" name="background" style={{ marginBottom: 12 }}>
        <TextArea rows={2} placeholder="角色经历、动机与重要过往..." />
      </Form.Item>

      {mainCareers.length > 0 || subCareers.length > 0 ? (
        <>
          <Divider style={{ margin: '8px 0' }}>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {mode === 'create' ? '职业设定（可选）' : '职业信息'}
            </Typography.Text>
          </Divider>
          {mainCareers.length > 0 ? (
            <Row gutter={12}>
              <Col span={16}>
                <Form.Item label="主职业" name="main_career_id" tooltip="用于标记角色当前的核心职业" style={{ marginBottom: 12 }}>
                  <Select placeholder="请选择职业" allowClear size="small">
                    {mainCareers.map((career) => (
                      <Select.Option key={career.id} value={career.id}>
                        {career.name}（最高{career.max_stage}阶）
                      </Select.Option>
                    ))}
                  </Select>
                </Form.Item>
              </Col>
              <Col span={8}>
                <Form.Item label="职业阶位" name="main_career_stage" tooltip="填写角色当前主职业所处的阶段" style={{ marginBottom: 12 }}>
                  <InputNumber
                    min={1}
                    max={form.getFieldValue('main_career_id')
                      ? mainCareers.find((career) => career.id === form.getFieldValue('main_career_id'))?.max_stage || 10
                      : 10}
                    style={{ width: '100%' }}
                    placeholder="阶位"
                    size="small"
                  />
                </Form.Item>
              </Col>
            </Row>
          ) : null}
          {subCareers.length > 0 ? (
            <Form.List name="sub_career_data">
              {(fields, { add, remove }) => (
                <>
                  <div style={{ marginBottom: 4 }}>
                    <Typography.Text strong style={{ fontSize: 12 }}>副职业</Typography.Text>
                  </div>
                  <div style={{ maxHeight: '80px', overflowY: 'auto', overflowX: 'hidden', marginBottom: 8, paddingRight: 8 }}>
                    {fields.map((field) => (
                      <Row key={field.key} gutter={8} style={{ marginBottom: 4 }}>
                        <Col span={16}>
                          <Form.Item
                            {...field}
                            name={[field.name, 'career_id']}
                            rules={[{ required: true, message: '请选择职业' }]}
                            style={{ marginBottom: 0 }}
                          >
                            <Select placeholder="请选择职业" size="small">
                              {subCareers.map((career) => (
                                <Select.Option key={career.id} value={career.id}>
                                  {career.name}（最高{career.max_stage}阶）
                                </Select.Option>
                              ))}
                            </Select>
                          </Form.Item>
                        </Col>
                        <Col span={5}>
                          <Form.Item
                            {...field}
                            name={[field.name, 'stage']}
                            rules={[{ required: true, message: '请输入阶位' }]}
                            style={{ marginBottom: 0 }}
                          >
                            <InputNumber
                              min={1}
                              max={(() => {
                                const careerId = form.getFieldValue(['sub_career_data', field.name, 'career_id']);
                                const career = subCareers.find((item) => item.id === careerId);
                                return career?.max_stage || 10;
                              })()}
                              placeholder="阶位"
                              style={{ width: '100%' }}
                              size="small"
                            />
                          </Form.Item>
                        </Col>
                        <Col span={3}>
                          <Button
                            type="text"
                            danger
                            size="small"
                            onClick={() => remove(field.name)}
                          >
                            删除
                          </Button>
                        </Col>
                      </Row>
                    ))}
                  </div>
                  <Button
                    type="dashed"
                    onClick={() => add({ career_id: undefined, stage: 1 })}
                    block
                    size="small"
                  >
                    + 添加副职业
                  </Button>
                </>
              )}
            </Form.List>
          ) : null}
        </>
      ) : null}
    </>
  );
}

function renderOrganizationFields(
  mode: 'create' | 'edit',
  token: ReturnType<typeof theme.useToken>['token'],
) {
  return (
    <>
      <Row gutter={12}>
        <Col span={10}>
          <Form.Item
            label="组织名称"
            name="name"
            rules={[{ required: true, message: '请输入组织名称' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="例如：青岚会" />
          </Form.Item>
        </Col>
        <Col span={8}>
          <Form.Item
            label="组织类型"
            name="organization_type"
            rules={[{ required: true, message: '请输入组织类型' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="例如：宗门 / 商会 / 学院" />
          </Form.Item>
        </Col>
        <Col span={6}>
          <Form.Item
            label="势力等级"
            name="power_level"
            initialValue={mode === 'create' ? 50 : undefined}
            tooltip="0-100 的数值，表示组织的影响力"
            style={{ marginBottom: 12 }}
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} />
          </Form.Item>
        </Col>
      </Row>

      <Form.Item
        label="组织宗旨"
        name="organization_purpose"
        rules={[{ required: true, message: '请输入组织宗旨' }]}
        style={{ marginBottom: 12 }}
      >
        <Input placeholder="组织追求什么、维护什么、想改变什么..." />
      </Form.Item>

      {mode === 'edit' ? (
        <>
          <Form.Item
            label="组织成员"
            name="organization_members"
            style={{ marginBottom: 4 }}
            tooltip="仅展示已关联到该组织的角色，需在角色信息中维护归属"
          >
            <TextArea
              disabled
              autoSize={{ minRows: 1, maxRows: 4 }}
              placeholder="暂无已关联成员"
              style={{ color: token.colorText, backgroundColor: token.colorFillAlter }}
            />
          </Form.Item>
          <div style={{ marginBottom: 12, fontSize: 12, color: token.colorTextTertiary }}>
            组织成员会随着角色归属调整自动更新
          </div>
        </>
      ) : null}

      <Row gutter={12}>
        <Col span={12}>
          <Form.Item label="所在地" name="location" style={{ marginBottom: 12 }}>
            <Input placeholder="组织的主要活动区域或总部位置" />
          </Form.Item>
        </Col>
        <Col span={12}>
          <Form.Item label="代表颜色" name="color" style={{ marginBottom: 12 }}>
            <Input placeholder="例如：深红色 / 金色 / 黑色" />
          </Form.Item>
        </Col>
      </Row>

      <Form.Item label="格言 / 口号" name="motto" style={{ marginBottom: 12 }}>
        <Input placeholder="例如：秩序高于一切" />
      </Form.Item>

      <Form.Item label="组织背景" name="background" style={{ marginBottom: 12 }}>
        <TextArea rows={2} placeholder="组织起源、历史与当前局势..." />
      </Form.Item>
    </>
  );
}

export default function CharacterFormModal({
  open,
  title,
  mode,
  entityType,
  form,
  isMobile,
  record,
  mainCareers,
  subCareers,
  submitText,
  onCancel,
  onFinish,
}: CharacterFormModalProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 96%, ${token.colorFillAlter} 4%) 0%, color-mix(in srgb, ${token.colorBgContainer} 88%, ${token.colorFillAlter} 12%) 100%)`;
  const characterFormWorkspaceFocus = entityType === 'organization'
    ? mode === 'create'
      ? {
          title: '当前更适合先把组织的类型、宗旨和势力等级搭起来',
          note: '这一步先把组织框架建立完整，再继续补地点、口号和背景，会比一开始就写长文本更顺手。',
        }
      : {
          title: '当前更适合先校准组织设定，再回看成员与势力信息',
          note: '编辑模式下已经有现成组织实体，建议优先检查宗旨、势力等级和所在地是否还符合当前剧情结构。',
        }
    : mode === 'create'
      ? {
          title: '当前更适合先建立角色定位，再逐步补人物细节',
          note: '创建角色时先把名称、角色类型和职业主线定下来，会让后续背景与外貌信息更容易保持一致。',
        }
      : {
          title: `当前正在调整 ${record?.name || '该角色'} 的设定，先看核心身份是否仍然成立`,
          note: '编辑模式更适合先复核角色类型、关系摘要和职业信息，再决定要不要继续改动背景或性格描写。',
        };

  return (
    <Modal
      title={title}
      open={open}
      onCancel={onCancel}
      footer={
        <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
          <Button onClick={onCancel}>取消</Button>
          <Button type="primary" onClick={() => form.submit()}>
            {submitText}
          </Button>
        </Space>
      }
      centered
      width={isMobile ? '100%' : 700}
      style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
      styles={{
        body: {
          maxHeight: isMobile ? 'calc(100vh - 110px)' : 'calc(100vh - 200px)',
          overflowY: 'auto',
          overflowX: 'hidden',
        },
      }}
    >
      <Form form={form} layout="vertical" onFinish={onFinish} style={{ marginTop: 8 }}>
        <Card
          bordered={false}
          style={{
            marginBottom: 16,
            borderRadius: 18,
            background: quietPanelBackground,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
              gap: 16,
            }}
          >
            <div>
              <Typography.Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Form Guide
              </Typography.Text>
              <Typography.Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                {entityType === 'organization' ? '组织表单填写顺序' : '角色表单填写顺序'}
              </Typography.Title>
              <Typography.Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                这里现在只增强表单阅读顺序与当前焦点说明，不改变 `Form` 提交、字段校验、职业列表、组织成员回填或移动端弹窗行为。
              </Typography.Paragraph>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
                {characterFormGuideSteps.map((item, index) => (
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
                borderRadius: 16,
                padding: '16px 18px',
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Typography.Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                当前工作焦点
              </Typography.Text>
              <Typography.Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                {characterFormWorkspaceFocus.title}
              </Typography.Title>
              <Typography.Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                {characterFormWorkspaceFocus.note}
              </Typography.Paragraph>
              <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                <Tag color={mode === 'create' ? 'green' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  {mode === 'create' ? '新建模式' : '编辑模式'}
                </Tag>
                <Tag color={entityType === 'organization' ? 'purple' : 'processing'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  {entityType === 'organization' ? '组织表单' : '角色表单'}
                </Tag>
                {entityType === 'character' ? (
                  <Tag color={mainCareers.length > 0 || subCareers.length > 0 ? 'gold' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    {mainCareers.length > 0 || subCareers.length > 0 ? '可补职业信息' : '无职业选项'}
                  </Tag>
                ) : (
                  <Tag color={mode === 'edit' ? 'orange' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    {mode === 'edit' ? '可回看成员信息' : '创建阶段无成员摘要'}
                  </Tag>
                )}
              </Space>
            </div>
          </div>
        </Card>

        {entityType === 'character'
          ? renderCharacterFields(mode, form, record, token, mainCareers, subCareers)
          : renderOrganizationFields(mode, token)}
      </Form>
    </Modal>
  );
}
