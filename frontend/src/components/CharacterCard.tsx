import { Card, Space, Tag, Typography, Popconfirm, theme } from 'antd';
import { EditOutlined, DeleteOutlined, UserOutlined, BankOutlined, ExportOutlined } from '@ant-design/icons';
import { characterCardStyles } from './CardStyles';
import type { Character } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

const { Paragraph, Text, Title } = Typography;

interface CharacterCardProps {
  character: Character;
  onEdit?: (character: Character) => void;
  onDelete: (id: string) => void;
  onExport?: () => void;
}

export const CharacterCard: React.FC<CharacterCardProps> = ({ character, onEdit, onDelete, onExport }) => {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const getRoleTypeColor = (roleType?: string) => {
    const roleColors: Record<string, string> = {
      'protagonist': 'blue',
      'supporting': 'green',
      'antagonist': 'red',
    };
    return roleColors[roleType || ''] || 'default';
  };

  const getRoleTypeLabel = (roleType?: string) => {
    const roleLabels: Record<string, string> = {
      'protagonist': '主角',
      'supporting': '配角',
      'antagonist': '反派',
    };
    return roleLabels[roleType || ''] || '其他';
  };

  const isOrganization = character.is_organization;
  const charStatus = character.status || 'active';
  const isInactive = charStatus !== 'active';
  const heroBackground = isOrganization
    ? `linear-gradient(135deg, color-mix(in srgb, ${token.colorInfo} 26%, ${token.colorBgContainer} 74%) 0%, color-mix(in srgb, ${token.colorSuccess} 18%, ${token.colorBgContainer} 82%) 100%)`
    : `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 18%, ${token.colorBgContainer} 82%) 0%, color-mix(in srgb, ${token.colorWarning} 12%, ${token.colorBgContainer} 88%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 94%, ${token.colorFillAlter} 6%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;

  const toPreviewText = (value: unknown, maxLength = 120) => {
    if (value === null || value === undefined) {
      return '';
    }

    const text = typeof value === 'string' ? value : JSON.stringify(value);
    if (!text) {
      return '';
    }

    return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
  };

  const organizationMembersFullText = character.organization_members
    ? (typeof character.organization_members === 'string'
      ? character.organization_members
      : JSON.stringify(character.organization_members))
    : '';
  const personalityPreviewText = toPreviewText(character.personality, 100);
  const relationshipsPreviewText = toPreviewText(character.relationships, 100);
  const locationPreviewText = toPreviewText(character.location, 80);
  const mottoPreviewText = toPreviewText(character.motto, 100);
  const organizationPurposePreviewText = toPreviewText(character.organization_purpose, 100);
  const organizationMembersText = toPreviewText(organizationMembersFullText, 100);
  const backgroundPreviewText = toPreviewText(character.background, 180);
  const singleLinePreviewStyle = {
    flex: 1,
    minWidth: 0,
    overflow: 'hidden' as const,
    whiteSpace: 'nowrap' as const,
    textOverflow: 'ellipsis' as const,
  };

  const getStatusTag = () => {
    const statusConfig: Record<string, { color: string; label: string }> = {
      deceased: { color: token.colorTextBase, label: '已死亡' },
      missing: { color: token.colorWarning, label: '已失踪' },
      retired: { color: token.colorTextTertiary, label: '已退场' },
      destroyed: { color: token.colorTextBase, label: '已覆灭' },
    };
    const config = statusConfig[charStatus];
    if (!config) return null;
    return <Tag color={config.color} style={{ margin: 0, borderRadius: 999 }}>{config.label}</Tag>;
  };

  const detailItems = (
    isOrganization
      ? [
          character.organization_type ? { label: '类型', value: character.organization_type, tagColor: 'cyan' } : null,
          character.power_level !== undefined && character.power_level !== null
            ? {
                label: '势力等级',
                value: String(character.power_level),
                tagColor: character.power_level >= 70 ? 'red' : character.power_level >= 50 ? 'orange' : 'default',
              }
            : null,
          character.location ? { label: '所在地', value: locationPreviewText } : null,
          character.color ? { label: '代表颜色', value: character.color } : null,
          character.motto ? { label: '格言', value: mottoPreviewText } : null,
          character.organization_purpose ? { label: '目的', value: organizationPurposePreviewText } : null,
          character.organization_members ? { label: '成员', value: organizationMembersText } : null,
        ]
      : [
          character.age ? { label: '年龄', value: character.age } : null,
          character.gender ? { label: '性别', value: character.gender } : null,
          character.personality ? { label: '性格', value: personalityPreviewText } : null,
          character.relationships ? { label: '关系', value: relationshipsPreviewText } : null,
        ]
  ).filter((item): item is { label: string; value: string; tagColor?: string } => Boolean(item));
  const heroSummary = isOrganization
    ? organizationPurposePreviewText || mottoPreviewText || locationPreviewText || '尚未补充这个组织的目标与风格摘要。'
    : personalityPreviewText || relationshipsPreviewText || '尚未补充这个角色的气质与关系摘要。';
  const sectionEyebrow = isOrganization ? 'Organization Profile' : 'Character Profile';
  const sectionTitle = isOrganization ? '组织档案' : '角色档案';

  return (
    <Card
      hoverable
      style={{
        ...(isOrganization ? characterCardStyles.organizationCard : characterCardStyles.characterCard),
        border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        boxShadow: `0 20px 40px ${alphaColor(token.colorText, 0.08)}`,
        ...(isInactive ? { opacity: 0.6, filter: 'grayscale(40%)' } : {}),
      }}
      styles={{
        body: {
          flex: 1,
          overflow: 'auto',
          display: 'flex',
          flexDirection: 'column',
          padding: 18,
        },
        actions: {
          borderRadius: '0 0 18px 18px',
        },
      }}
      actions={[
        ...(onEdit ? [<EditOutlined key="edit" onClick={() => onEdit(character)} />] : []),
        ...(onExport ? [<ExportOutlined key="export" onClick={onExport} />] : []),
        <Popconfirm
          key="delete"
          title={`确定删除这个${isOrganization ? '组织' : '角色'}吗？`}
          onConfirm={() => onDelete(character.id)}
          okText="确定"
          cancelText="取消"
        >
          <DeleteOutlined />
        </Popconfirm>,
      ]}
    >
      <div style={{ display: 'grid', gap: 16, height: '100%' }}>
        <div
          style={{
            padding: '16px 16px 14px',
            borderRadius: 18,
            background: heroBackground,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          }}
        >
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            {sectionEyebrow}
          </Text>
          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-start', marginTop: 10 }}>
            <div
              style={{
                width: 42,
                height: 42,
                borderRadius: 14,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: alphaColor(isOrganization ? token.colorSuccess : token.colorPrimary, 0.14),
                color: isOrganization ? token.colorSuccess : token.colorPrimary,
                flexShrink: 0,
              }}
            >
              {isOrganization ? <BankOutlined style={{ fontSize: 22 }} /> : <UserOutlined style={{ fontSize: 22 }} />}
            </div>
            <div style={{ minWidth: 0, flex: 1 }}>
              <Title
                level={4}
                style={{
                  margin: '0 0 6px',
                  fontFamily: designDisplayFont,
                  letterSpacing: '-0.02em',
                }}
              >
                <span style={characterCardStyles.nameEllipsis}>{character.name}</span>
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                {heroSummary}
              </Paragraph>
            </div>
          </div>
          <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
            <Tag color={isOrganization ? 'green' : 'blue'} style={{ margin: 0, borderRadius: 999 }}>
              {sectionTitle}
            </Tag>
            {!isOrganization && character.role_type ? (
              <Tag color={getRoleTypeColor(character.role_type)} style={{ margin: 0, borderRadius: 999 }}>
                {getRoleTypeLabel(character.role_type)}
              </Tag>
            ) : null}
            {getStatusTag()}
            {isInactive ? (
              <Tag color="default" style={{ margin: 0, borderRadius: 999 }}>
                非活跃
              </Tag>
            ) : null}
          </Space>
        </div>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
            gap: 10,
          }}
        >
          {detailItems.length > 0 ? (
            detailItems.map((item) => (
              <div
                key={`${item.label}-${item.value}`}
                style={{
                  padding: '12px 14px',
                  borderRadius: 16,
                  background: quietPanelBackground,
                  border: `1px solid ${token.colorBorderSecondary}`,
                  minWidth: 0,
                }}
              >
                <Text style={{ display: 'block', fontSize: 12, color: token.colorTextTertiary, marginBottom: 6 }}>
                  {item.label}
                </Text>
                {item.tagColor ? (
                  <Tag color={item.tagColor} style={{ margin: 0, borderRadius: 999 }}>
                    {item.value}
                  </Tag>
                ) : (
                  <Text style={singleLinePreviewStyle} title={item.value}>
                    {item.value}
                  </Text>
                )}
              </div>
            ))
          ) : (
            <div
              style={{
                padding: '14px 16px',
                borderRadius: 16,
                background: quietPanelBackground,
                border: `1px dashed ${token.colorBorderSecondary}`,
                color: token.colorTextSecondary,
              }}
            >
              暂无更多结构化信息，先保留基础档案入口。
            </div>
          )}
        </div>

        {character.background ? (
          <div
            style={{
              borderRadius: 18,
              padding: '14px 16px',
              background: quietPanelBackground,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 12, color: token.colorTextTertiary, marginBottom: 6 }}>
              背景摘录
            </Text>
            <Paragraph
              style={{
                margin: 0,
                color: token.colorTextSecondary,
                lineHeight: 1.75,
                wordBreak: 'break-word',
              }}
              title={character.background}
            >
              {backgroundPreviewText}
            </Paragraph>
          </div>
        ) : null}
      </div>
    </Card>
  );
};
