import { memo, type CSSProperties } from 'react';
import { ApartmentOutlined, TeamOutlined, TrophyOutlined, UserOutlined } from '@ant-design/icons';
import { Button, Card, Space, Tag, Typography, theme } from 'antd';

import { safeParseSubCareers } from './selectors';
import type { CareerItem, CharacterDetail } from './types';

const { Text } = Typography;

const clampTextStyle = (rows: number): CSSProperties => ({
  margin: '4px 0 0',
  color: 'var(--ant-color-text-secondary)',
  fontSize: 14,
  lineHeight: '22px',
  display: '-webkit-box',
  WebkitBoxOrient: 'vertical',
  WebkitLineClamp: rows,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  wordBreak: 'break-word',
});

const safeParseStringArray = (raw: unknown): string[] => {
  if (!raw) return [];

  if (Array.isArray(raw)) {
    return raw.map((item) => String(item)).filter(Boolean);
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed)) {
        return parsed.map((item) => String(item)).filter(Boolean);
      }
    } catch {
      return raw
        .split(/[，,]/)
        .map((item) => item.trim())
        .filter(Boolean);
    }
  }

  return [];
};

const InfoField = ({
  label,
  value,
  rows = 2,
}: {
  label: string;
  value?: string | null;
  rows?: number;
}) => {
  if (!value) return null;

  return (
    <div
      style={{
        marginBottom: 12,
        padding: '12px 14px',
        borderRadius: 12,
        background: 'var(--ant-color-fill-quaternary)',
        border: '1px solid var(--ant-color-border-secondary)',
        boxShadow: '0 2px 4px color-mix(in srgb, var(--ant-color-text) 6%, transparent)',
      }}
    >
      <Text strong style={{ fontSize: 14, color: 'var(--ant-color-text)' }}>
        {label}
      </Text>
      <div style={clampTextStyle(rows)}>{value}</div>
    </div>
  );
};

interface RelationshipGraphDetailPanelProps {
  selectedNodeId: string | null;
  nodeDetail: CharacterDetail | null;
  careerNameMap: Record<string, CareerItem>;
  onClose: () => void;
}

function RelationshipGraphDetailPanel({
  selectedNodeId,
  nodeDetail,
  careerNameMap,
  onClose,
}: RelationshipGraphDetailPanelProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const detailGuideSteps = [
    '先确认当前选中的是角色、组织还是职业节点，再决定要看人物信息、组织势力还是职业关联。',
    '再用顶部焦点卡快速锁定这次阅读重点，把详情面板当作关系校对入口，而不是一次性看完所有字段。',
    '最后再下钻到具体字段，结合关系图主视图判断这条节点在世界观中的位置是否清晰。',
  ];

  if (!selectedNodeId) {
    return null;
  }

  if (!nodeDetail) {
    if (!selectedNodeId.startsWith('career-main-') && !selectedNodeId.startsWith('career-sub-')) {
      return null;
    }

    return (
      <div
        style={{
          position: 'fixed',
          right: 20,
          top: 80,
          zIndex: 1000,
        }}
      >
        <Card
          size="small"
          style={{
            width: 320,
            borderRadius: 16,
            border: `1px solid ${alphaColor(token.colorWarning, 0.18)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorWarningBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            boxShadow: `0 10px 24px ${alphaColor(token.colorTextBase, 0.14)}`,
          }}
          bodyStyle={{ padding: 16 }}
        >
          <div style={{ fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Career Node Guide
          </div>
          <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
            职业节点说明
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            该节点用于展示职业体系中的职业，可通过连线查看角色与职业的关联。这里先给出用途说明，不展开角色或组织详情。
          </Text>
          <Space align="start">
            <TrophyOutlined style={{ color: token.colorWarning, marginTop: 4 }} />
            <div>
              <Text strong>当前工作焦点</Text>
              <p style={{ ...clampTextStyle(2), marginTop: 2 }}>
                先从主视图里的连线关系判断谁与这个职业节点有关，再回到关系图继续查看具体角色或组织。
              </p>
            </div>
          </Space>
        </Card>
      </div>
    );
  }

  const traitList = safeParseStringArray(nodeDetail.traits);
  const orgMembers = safeParseStringArray(nodeDetail.organization_members);
  const subCareerData = nodeDetail.is_organization ? [] : safeParseSubCareers(nodeDetail.sub_careers);
  const detailWorkspaceFocus = nodeDetail.is_organization
    ? {
        title: '先确认这个组织节点在关系图里承担的是势力还是成员聚合角色',
        note: '当前更适合优先查看组织宗旨、所在地和势力等级，再结合成员信息判断它在剧情推进中的组织地位。',
      }
    : nodeDetail.role_type === 'protagonist'
      ? {
          title: '先确认主角当前的人设、职业与关系图中心位置是否一致',
          note: '当前节点是主角，更适合优先检查性格、背景和职业阶段，确认它和关系图主线是否对齐。',
        }
      : nodeDetail.role_type === 'antagonist'
        ? {
            title: '先确认反派节点的身份、动机和职业路径是否足够清晰',
            note: '当前节点承担对抗角色，更适合先看背景、职业与关键标签，再判断关系图里的冲突关系是否明确。',
          }
        : {
            title: '先从配角的功能定位和职业信息切入详情回看',
            note: '当前节点更适合作为辅助角色回看，建议优先检查职业、特征与背景，确认它在关系图中的叙事作用。',
          };

  const renderCareerTags = () => {
    if (nodeDetail.is_organization) return null;

    return (
      <div
        style={{
          marginBottom: 12,
          padding: '12px 14px',
          borderRadius: 12,
          background: token.colorFillQuaternary,
          border: `1px solid ${token.colorBorderSecondary}`,
          boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
        }}
      >
        <Text strong style={{ fontSize: 14, color: token.colorText }}>
          职业信息
        </Text>
        <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
          {nodeDetail.main_career_id ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag color="gold" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>主职业</Tag>
              <span style={{ fontSize: 14, color: token.colorText }}>
                {careerNameMap[nodeDetail.main_career_id]?.name || nodeDetail.main_career_id}
                {nodeDetail.main_career_stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>（第{nodeDetail.main_career_stage}阶）</span> : ''}
              </span>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>主职业</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>未设置</span>
            </div>
          )}

          {subCareerData.length > 0 ? (
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
              <Tag color="cyan" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>副职业</Tag>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, flex: 1 }}>
                {subCareerData.map((sub, index) => (
                  <span key={`${sub.career_id}-${index}`} style={{ fontSize: 14, color: token.colorText, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, borderRadius: token.borderRadiusSM, padding: '0 6px' }}>
                    {careerNameMap[sub.career_id]?.name || sub.career_id}
                    {sub.stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>（第{sub.stage}阶）</span> : ''}
                  </span>
                ))}
              </div>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>副职业</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>未设置</span>
            </div>
          )}
        </div>
      </div>
    );
  };

  return (
    <div
      style={{
        position: 'fixed',
        right: 24,
        top: 80,
        width: 400,
        height: 'calc(100vh - 100px)',
        maxHeight: 700,
        zIndex: 1000,
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <Card
        size="small"
        style={{
          width: '100%',
          flex: 1,
          borderRadius: 16,
          boxShadow: `0 12px 32px ${alphaColor(token.colorTextBase, 0.22)}`,
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
        }}
        bodyStyle={{
          flex: 1,
          overflow: 'hidden',
          padding: '12px 16px',
          display: 'flex',
          flexDirection: 'column',
        }}
        title={
          <Space>
            {nodeDetail.is_organization ? <ApartmentOutlined /> : <UserOutlined />}
            <span>{nodeDetail.is_organization ? '组织详情' : '角色详情'}</span>
          </Space>
        }
        extra={
          <Button type="text" size="small" onClick={onClose}>
            关闭
          </Button>
        }
      >
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          <div
            style={{
              marginBottom: 14,
              padding: '14px 16px',
              borderRadius: 20,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
              background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
                gap: 14,
              }}
            >
              <div>
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  Detail Guide
                </Text>
                <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
                  关系图详情导览
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                  这里负责把关系图里的节点信息展开成可读详情。当前只增强阅读顺序和焦点说明，不改变任何节点解析、字段映射或关闭交互。
                </Text>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {detailGuideSteps.map((item, index) => (
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
                  borderRadius: 16,
                  padding: '14px 16px 12px',
                  background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                }}
              >
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  当前工作焦点
                </Text>
                <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 8 }}>
                  {detailWorkspaceFocus.title}
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                  {detailWorkspaceFocus.note}
                </Text>
                <Space wrap>
                  <Tag color={nodeDetail.is_organization ? 'green' : nodeDetail.role_type === 'protagonist' ? 'red' : nodeDetail.role_type === 'antagonist' ? 'purple' : 'blue'}>
                    {nodeDetail.is_organization ? '组织节点' : nodeDetail.role_type === 'protagonist' ? '主角节点' : nodeDetail.role_type === 'antagonist' ? '反派节点' : '配角节点'}
                  </Tag>
                  {!nodeDetail.is_organization && nodeDetail.main_career_id ? (
                    <Tag color="gold">
                      主职业: {careerNameMap[nodeDetail.main_career_id]?.name || nodeDetail.main_career_id}
                    </Tag>
                  ) : null}
                  {nodeDetail.is_organization && nodeDetail.power_level !== undefined && nodeDetail.power_level !== null ? (
                    <Tag color="orange">势力等级: {nodeDetail.power_level}/100</Tag>
                  ) : null}
                </Space>
              </div>
            </div>
          </div>

          <div
            style={{
              textAlign: 'center',
              marginBottom: 16,
              padding: '8px 12px 0',
              minHeight: 140,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
            }}
          >
            <div style={{ position: 'relative', width: 84, height: 84, marginBottom: 12 }}>
              {nodeDetail.avatar_url ? (
                <img
                  src={nodeDetail.avatar_url}
                  alt={nodeDetail.name}
                  style={{
                    width: '100%',
                    height: '100%',
                    borderRadius: '50%',
                    objectFit: 'cover',
                    border: `3px solid ${token.colorBgContainer}`,
                    boxShadow: `0 4px 12px ${alphaColor(token.colorTextBase, 0.18)}`,
                  }}
                />
              ) : (
                <div
                  style={{
                    width: '100%',
                    height: '100%',
                    borderRadius: '50%',
                    backgroundColor: nodeDetail.color || (nodeDetail.is_organization ? token.colorSuccess : token.colorPrimary),
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 32,
                    color: token.colorWhite,
                    border: `3px solid ${token.colorBgContainer}`,
                    boxShadow: `0 4px 12px ${alphaColor(token.colorTextBase, 0.18)}`,
                  }}
                >
                  {nodeDetail.is_organization ? <TeamOutlined /> : <UserOutlined />}
                </div>
              )}
              <div
                style={{
                  position: 'absolute',
                  bottom: -4,
                  right: -4,
                  background: nodeDetail.is_organization ? token.colorSuccess : (nodeDetail.role_type === 'protagonist' ? token.colorError : nodeDetail.role_type === 'antagonist' ? token.colorPrimary : token.colorInfo),
                  borderRadius: '50%',
                  width: 28,
                  height: 28,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  border: `2px solid ${token.colorBgContainer}`,
                  color: token.colorWhite,
                  boxShadow: `0 2px 6px ${alphaColor(token.colorTextBase, 0.22)}`,
                }}
              >
                {nodeDetail.is_organization ? <ApartmentOutlined style={{ fontSize: 14 }} /> : <UserOutlined style={{ fontSize: 14 }} />}
              </div>
            </div>

            <div style={{ fontSize: 20, fontWeight: 600, color: token.colorText, marginBottom: 8 }}>{nodeDetail.name}</div>
            <Space size={6} wrap style={{ justifyContent: 'center' }}>
              {!nodeDetail.is_organization && (
                <Tag
                  color={
                    nodeDetail.role_type === 'protagonist'
                      ? 'red'
                      : nodeDetail.role_type === 'antagonist'
                        ? 'purple'
                        : 'blue'
                  }
                  style={{ borderRadius: 12, padding: '0 10px', fontWeight: 500 }}
                >
                  {nodeDetail.role_type === 'protagonist'
                    ? '主角'
                    : nodeDetail.role_type === 'antagonist'
                      ? '反派'
                      : '配角'}
                </Tag>
              )}
              {nodeDetail.gender && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.gender}</Tag>}
              {nodeDetail.age && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.age}岁</Tag>}
            </Space>
          </div>

          <div style={{ flex: 1, overflowY: 'auto', paddingRight: 8, paddingLeft: 4, paddingBottom: 16 }}>
            {!nodeDetail.is_organization ? (
              <>
                {renderCareerTags()}
                <InfoField label="外貌特征" value={nodeDetail.appearance} rows={2} />
                <InfoField label="性格特点" value={nodeDetail.personality} rows={3} />
                <InfoField label="背景故事" value={nodeDetail.background} rows={4} />

                {traitList.length > 0 && (
                  <div
                    style={{
                      marginBottom: 12,
                      padding: '12px 14px',
                      borderRadius: 12,
                      background: token.colorFillQuaternary,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                    }}
                  >
                    <Text strong style={{ fontSize: 14, color: token.colorText }}>
                      特征标签
                    </Text>
                    <Space size={[6, 8]} wrap style={{ marginTop: 10 }}>
                      {traitList.slice(0, 12).map((trait, index) => (
                        <Tag key={`${trait}-${index}`} color="blue" style={{ borderRadius: 12, padding: '0 10px', margin: 0 }}>
                          {trait}
                        </Tag>
                      ))}
                    </Space>
                  </div>
                )}
              </>
            ) : (
              <>
                <InfoField label="组织类型" value={nodeDetail.organization_type} rows={2} />
                <InfoField label="组织宗旨" value={nodeDetail.organization_purpose} rows={3} />
                <InfoField label="所在地" value={nodeDetail.location} rows={2} />
                <InfoField label="格言/口号" value={nodeDetail.motto} rows={2} />

                {nodeDetail.power_level !== undefined && nodeDetail.power_level !== null && (
                  <div
                    style={{
                      marginBottom: 12,
                      padding: '12px 14px',
                      borderRadius: 12,
                      background: token.colorFillQuaternary,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                    }}
                  >
                    <Text strong style={{ fontSize: 14, color: token.colorText }}>
                      势力等级
                    </Text>
                    <div style={{ ...clampTextStyle(1), fontSize: 18, color: token.colorWarning, fontWeight: 'bold' }}>
                      {nodeDetail.power_level}<span style={{ fontSize: 14, color: token.colorTextTertiary, fontWeight: 'normal' }}>/100</span>
                    </div>
                  </div>
                )}

                {orgMembers.length > 0 && (
                  <div
                    style={{
                      marginBottom: 12,
                      padding: '12px 14px',
                      borderRadius: 12,
                      background: token.colorFillQuaternary,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                    }}
                  >
                    <Text strong style={{ fontSize: 14, color: token.colorText }}>
                      组织成员
                    </Text>
                    <Space size={[6, 8]} wrap style={{ marginTop: 10 }}>
                      {orgMembers.slice(0, 16).map((member, index) => (
                        <Tag key={`${member}-${index}`} color="green" style={{ borderRadius: 12, padding: '0 10px', margin: 0 }}>
                          {member}
                        </Tag>
                      ))}
                    </Space>
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      </Card>
    </div>
  );
}

export default memo(RelationshipGraphDetailPanel);
