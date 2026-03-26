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
        .split(/[?,?]/)
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
        <Card size="small" style={{ width: 300, borderRadius: 10, boxShadow: `0 6px 18px ${alphaColor(token.colorTextBase, 0.2)}` }}>
          <Space align="start">
            <TrophyOutlined style={{ color: token.colorWarning, marginTop: 4 }} />
            <div>
              <Text strong>????</Text>
              <p style={{ ...clampTextStyle(2), marginTop: 2 }}>
                ?????????/?????????????????????????
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
          ????
        </Text>
        <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
          {nodeDetail.main_career_id ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag color="gold" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>???</Tag>
              <span style={{ fontSize: 14, color: token.colorText }}>
                {careerNameMap[nodeDetail.main_career_id]?.name || nodeDetail.main_career_id}
                {nodeDetail.main_career_stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>?{nodeDetail.main_career_stage}?</span> : ''}
              </span>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>???</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>???</span>
            </div>
          )}

          {subCareerData.length > 0 ? (
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
              <Tag color="cyan" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>???</Tag>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, flex: 1 }}>
                {subCareerData.map((sub, index) => (
                  <span key={`${sub.career_id}-${index}`} style={{ fontSize: 14, color: token.colorText, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, borderRadius: token.borderRadiusSM, padding: '0 6px' }}>
                    {careerNameMap[sub.career_id]?.name || sub.career_id}
                    {sub.stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>?{sub.stage}</span> : ''}
                  </span>
                ))}
              </div>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>???</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>???</span>
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
            <span>{nodeDetail.is_organization ? '????' : '????'}</span>
          </Space>
        }
        extra={
          <Button type="text" size="small" onClick={onClose}>
            ?
          </Button>
        }
      >
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
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
                    ? '??'
                    : nodeDetail.role_type === 'antagonist'
                      ? '??'
                      : '??'}
                </Tag>
              )}
              {nodeDetail.gender && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.gender}</Tag>}
              {nodeDetail.age && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.age}?</Tag>}
            </Space>
          </div>

          <div style={{ flex: 1, overflowY: 'auto', paddingRight: 8, paddingLeft: 4, paddingBottom: 16 }}>
            {!nodeDetail.is_organization ? (
              <>
                {renderCareerTags()}
                <InfoField label="????" value={nodeDetail.appearance} rows={2} />
                <InfoField label="????" value={nodeDetail.personality} rows={3} />
                <InfoField label="????" value={nodeDetail.background} rows={4} />

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
                      ????
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
                <InfoField label="????" value={nodeDetail.organization_type} rows={2} />
                <InfoField label="????" value={nodeDetail.organization_purpose} rows={3} />
                <InfoField label="???" value={nodeDetail.location} rows={2} />
                <InfoField label="????" value={nodeDetail.motto} rows={2} />

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
                      ????
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
                      ????
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
