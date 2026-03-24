import { Button, Col, Divider, Form, Input, InputNumber, Modal, Row, Select, Space, Typography, theme } from 'antd';
import type { FormInstance } from 'antd';
import type { Character } from '../types';

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
            label="????"
            name="name"
            rules={[{ required: true, message: '???????' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="????" />
          </Form.Item>
        </Col>
        <Col span={6}>
          <Form.Item
            label="????"
            name="role_type"
            initialValue={mode === 'create' ? 'supporting' : undefined}
            style={{ marginBottom: 12 }}
          >
            <Select>
              <Select.Option value="protagonist">??</Select.Option>
              <Select.Option value="supporting">??</Select.Option>
              <Select.Option value="antagonist">??</Select.Option>
            </Select>
          </Form.Item>
        </Col>
        <Col span={5}>
          <Form.Item label="??" name="age" style={{ marginBottom: 12 }}>
            <Input placeholder="??25?" />
          </Form.Item>
        </Col>
        <Col span={5}>
          <Form.Item label="??" name="gender" style={{ marginBottom: 12 }}>
            <Select placeholder="??">
              <Select.Option value="?">?</Select.Option>
              <Select.Option value="?">?</Select.Option>
              <Select.Option value="??">??</Select.Option>
            </Select>
          </Form.Item>
        </Col>
      </Row>

      <Row gutter={12}>
        <Col span={12}>
          <Form.Item label="????" name="personality" style={{ marginBottom: 12 }}>
            <TextArea rows={2} placeholder="?????????..." />
          </Form.Item>
        </Col>
        <Col span={12}>
          <Form.Item label="????" name="appearance" style={{ marginBottom: 12 }}>
            <TextArea rows={2} placeholder="?????????..." />
          </Form.Item>
        </Col>
      </Row>

      {mode === 'edit' && record?.relationships ? (
        <Form.Item label="?????????????" style={{ marginBottom: 12 }}>
          <Input.TextArea
            value={record.relationships}
            readOnly
            autoSize={{ minRows: 1, maxRows: 3 }}
            style={{ backgroundColor: token.colorFillTertiary, cursor: 'default' }}
          />
        </Form.Item>
      ) : null}

      <Form.Item label="????" name="background" style={{ marginBottom: 12 }}>
        <TextArea rows={2} placeholder="?????????..." />
      </Form.Item>

      {mainCareers.length > 0 || subCareers.length > 0 ? (
        <>
          <Divider style={{ margin: '8px 0' }}>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {mode === 'create' ? '????????' : '????'}
            </Typography.Text>
          </Divider>
          {mainCareers.length > 0 ? (
            <Row gutter={12}>
              <Col span={16}>
                <Form.Item label="???" name="main_career_id" tooltip="?????????" style={{ marginBottom: 12 }}>
                  <Select placeholder="?????" allowClear size="small">
                    {mainCareers.map((career) => (
                      <Select.Option key={career.id} value={career.id}>
                        {career.name}???{career.max_stage}??
                      </Select.Option>
                    ))}
                  </Select>
                </Form.Item>
              </Col>
              <Col span={8}>
                <Form.Item label="????" name="main_career_stage" tooltip="???????????" style={{ marginBottom: 12 }}>
                  <InputNumber
                    min={1}
                    max={form.getFieldValue('main_career_id')
                      ? mainCareers.find((career) => career.id === form.getFieldValue('main_career_id'))?.max_stage || 10
                      : 10}
                    style={{ width: '100%' }}
                    placeholder="??"
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
                    <Typography.Text strong style={{ fontSize: 12 }}>???</Typography.Text>
                  </div>
                  <div style={{ maxHeight: '80px', overflowY: 'auto', overflowX: 'hidden', marginBottom: 8, paddingRight: 8 }}>
                    {fields.map((field) => (
                      <Row key={field.key} gutter={8} style={{ marginBottom: 4 }}>
                        <Col span={16}>
                          <Form.Item
                            {...field}
                            name={[field.name, 'career_id']}
                            rules={[{ required: true, message: '??????' }]}
                            style={{ marginBottom: 0 }}
                          >
                            <Select placeholder="?????" size="small">
                              {subCareers.map((career) => (
                                <Select.Option key={career.id} value={career.id}>
                                  {career.name}???{career.max_stage}??
                                </Select.Option>
                              ))}
                            </Select>
                          </Form.Item>
                        </Col>
                        <Col span={5}>
                          <Form.Item
                            {...field}
                            name={[field.name, 'stage']}
                            rules={[{ required: true, message: '??' }]}
                            style={{ marginBottom: 0 }}
                          >
                            <InputNumber
                              min={1}
                              max={(() => {
                                const careerId = form.getFieldValue(['sub_career_data', field.name, 'career_id']);
                                const career = subCareers.find((item) => item.id === careerId);
                                return career?.max_stage || 10;
                              })()}
                              placeholder="??"
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
                            ??
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
                    + ?????
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
            label="????"
            name="name"
            rules={[{ required: true, message: '???????' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="????" />
          </Form.Item>
        </Col>
        <Col span={8}>
          <Form.Item
            label="????"
            name="organization_type"
            rules={[{ required: true, message: '???????' }]}
            style={{ marginBottom: 12 }}
          >
            <Input placeholder="???????" />
          </Form.Item>
        </Col>
        <Col span={6}>
          <Form.Item
            label="????"
            name="power_level"
            initialValue={mode === 'create' ? 50 : undefined}
            tooltip="0-100???"
            style={{ marginBottom: 12 }}
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} />
          </Form.Item>
        </Col>
      </Row>

      <Form.Item
        label="????"
        name="organization_purpose"
        rules={[{ required: true, message: '???????' }]}
        style={{ marginBottom: 12 }}
      >
        <Input placeholder="??????????..." />
      </Form.Item>

      {mode === 'edit' ? (
        <>
          <Form.Item
            label="????"
            name="organization_members"
            style={{ marginBottom: 4 }}
            tooltip="???????????????????"
          >
            <TextArea
              disabled
              autoSize={{ minRows: 1, maxRows: 4 }}
              placeholder="??????????????"
              style={{ color: token.colorText, backgroundColor: token.colorFillAlter }}
            />
          </Form.Item>
          <div style={{ marginBottom: 12, fontSize: 12, color: token.colorTextTertiary }}>
            ?? ????????????????????
          </div>
        </>
      ) : null}

      <Row gutter={12}>
        <Col span={12}>
          <Form.Item label="???" name="location" style={{ marginBottom: 12 }}>
            <Input placeholder="????" />
          </Form.Item>
        </Col>
        <Col span={12}>
          <Form.Item label="????" name="color" style={{ marginBottom: 12 }}>
            <Input placeholder="????" />
          </Form.Item>
        </Col>
      </Row>

      <Form.Item label="??/??" name="motto" style={{ marginBottom: 12 }}>
        <Input placeholder="???????????" />
      </Form.Item>

      <Form.Item label="????" name="background" style={{ marginBottom: 12 }}>
        <TextArea rows={2} placeholder="?????????..." />
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

  return (
    <Modal
      title={title}
      open={open}
      onCancel={onCancel}
      footer={
        <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
          <Button onClick={onCancel}>??</Button>
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
        {entityType === 'character'
          ? renderCharacterFields(mode, form, record, token, mainCareers, subCareers)
          : renderOrganizationFields(mode, token)}
      </Form>
    </Modal>
  );
}
