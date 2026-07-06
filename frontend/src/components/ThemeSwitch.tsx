import { Segmented, Tooltip, theme } from 'antd';
import { BulbOutlined, MoonOutlined, DesktopOutlined } from '@ant-design/icons';
import { useThemeMode } from '../theme/useThemeMode';
import type { ThemeMode } from '../theme/themeStorage';
import type { CSSProperties, ReactNode } from 'react';
import { designDisplayFont } from '../theme/themeConfig';

interface ThemeSwitchProps {
  size?: 'small' | 'middle' | 'large';
  block?: boolean;
}

const iconBadgeStyle = (background: string): CSSProperties => ({
  width: 24,
  height: 24,
  borderRadius: '50%',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  background,
  boxShadow: 'inset 0 0 0 1px color-mix(in srgb, rgba(255,255,255,0.46) 45%, transparent)',
});

const buildOptionLabel = (
  title: string,
  heading: string,
  subtitle: string,
  icon: ReactNode,
  badgeBackground: string,
) => (
  <Tooltip title={title}>
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 10,
        minWidth: 0,
      }}
    >
      <span style={iconBadgeStyle(badgeBackground)}>{icon}</span>
      <span style={{ display: 'inline-flex', flexDirection: 'column', alignItems: 'flex-start', lineHeight: 1.05 }}>
        <span style={{ fontSize: 12, fontWeight: 600, fontFamily: designDisplayFont, letterSpacing: '0.01em' }}>{heading}</span>
        <span style={{ fontSize: 10, opacity: 0.72 }}>{subtitle}</span>
      </span>
    </span>
  </Tooltip>
);

const options: Array<{ value: ThemeMode; label: ReactNode }> = [
  {
    value: 'light',
    label: buildOptionLabel('浅色模式', 'Light', '浅色', <BulbOutlined />, 'linear-gradient(135deg, #fff4bf 0%, #ffe08a 100%)'),
  },
  {
    value: 'dark',
    label: buildOptionLabel('深色模式', 'Dark', '深色', <MoonOutlined />, 'linear-gradient(135deg, #d7dcff 0%, #a9b6ff 100%)'),
  },
  {
    value: 'system',
    label: buildOptionLabel('跟随系统', 'System', '跟随系统', <DesktopOutlined />, 'linear-gradient(135deg, #d9f6f0 0%, #b4ebe0 100%)'),
  },
];

export default function ThemeSwitch({ size = 'middle', block = false }: ThemeSwitchProps) {
  const { mode, setMode } = useThemeMode();
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const sizePaddingMap = {
    small: 4,
    middle: 5,
    large: 6,
  } as const;

  return (
    <div
      style={{
        display: block ? 'block' : 'inline-flex',
        width: block ? '100%' : 'auto',
        padding: sizePaddingMap[size],
        borderRadius: size === 'small' ? 16 : 18,
        background: `linear-gradient(135deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.72)} 100%)`,
        border: `1px solid ${alphaColor(token.colorPrimary, 0.1)}`,
        boxShadow: `0 12px 28px ${alphaColor(token.colorText, 0.08)}`,
      }}
    >
      <Segmented
        size={size}
        value={mode}
        onChange={(value) => setMode(value as ThemeMode)}
        options={options}
        block={block}
        style={{
          width: block ? '100%' : 'auto',
          background: 'transparent',
          borderRadius: size === 'small' ? 12 : 14,
        }}
      />
    </div>
  );
}
