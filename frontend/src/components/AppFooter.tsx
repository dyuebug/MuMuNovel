import { useState, useEffect, useRef } from 'react';
import { Typography, Space, Badge, Grid, theme } from 'antd';
import { GithubOutlined, CopyrightOutlined, HeartFilled, ClockCircleOutlined } from '@ant-design/icons';
import { VERSION_INFO, getVersionString } from '../config/version';
import { checkLatestVersion } from '../services/versionService';
import { useThemeMode } from '../theme/useThemeMode';

const { Text, Link } = Typography;
const { useBreakpoint } = Grid;

interface AppFooterProps {
  sidebarWidth?: number;
  floating?: boolean;
}

export default function AppFooter({ sidebarWidth = 0, floating = false }: AppFooterProps) {
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const [hasUpdate, setHasUpdate] = useState(false);
  const [latestVersion, setLatestVersion] = useState('');
  const [releaseUrl, setReleaseUrl] = useState('');
  const { token } = theme.useToken();
  const { resolvedMode } = useThemeMode();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);
  const editorialSurface = resolvedMode === 'dark'
    ? 'linear-gradient(135deg, #141311 0%, #1d1916 100%)'
    : 'linear-gradient(135deg, #191613 0%, #241d18 100%)';
  const editorialText = '#f7f1e8';
  const editorialTextSoft = alphaColor(editorialText, 0.76);
  const editorialBorder = alphaColor(editorialText, 0.12);

  // 点击版本号查看更新
  const handleVersionClick = () => {
    if (hasUpdate && releaseUrl) {
      window.open(releaseUrl, '_blank');
    }
  };

  const resourceLinks = [
    {
      key: 'github',
      label: 'GitHub 仓库',
      href: VERSION_INFO.githubUrl,
      icon: <GithubOutlined style={{ fontSize: 12 }} />,
    },
    {
      key: 'community',
      label: 'LinuxDO 社区',
      href: VERSION_INFO.linuxDoUrl,
    },
    {
      key: 'license',
      label: VERSION_INFO.license,
      href: VERSION_INFO.licenseUrl,
      icon: <CopyrightOutlined style={{ fontSize: 11 }} />,
    },
  ];
  const statusItems = [
    {
      key: 'version',
      label: `版本 ${getVersionString()}`,
      title: hasUpdate ? `发现新版本 v${latestVersion}，点击查看` : '当前版本',
      clickable: hasUpdate && Boolean(releaseUrl),
      onClick: handleVersionClick,
    },
    {
      key: 'build',
      label: `Build ${VERSION_INFO.buildTime}`,
      title: '当前构建时间',
    },
    {
      key: 'update',
      label: hasUpdate ? `可更新到 v${latestVersion}` : '已是当前已知版本',
      title: hasUpdate ? `发现新版本 v${latestVersion}` : '当前未发现新版本',
      clickable: hasUpdate && Boolean(releaseUrl),
      onClick: handleVersionClick,
    },
  ];

  useEffect(() => {
    mountedRef.current = true;

    // 检查版本更新（每次都重新检查）
    const checkVersion = async () => {
      requestIdRef.current += 1;
      const requestId = requestIdRef.current;
      try {
        const result = await checkLatestVersion();
        if (!mountedRef.current || requestIdRef.current !== requestId) {
          return;
        }
        setHasUpdate(result.hasUpdate);
        setLatestVersion(result.latestVersion);
        setReleaseUrl(result.releaseUrl);
      } catch {
        // 静默失败
      }
    };

    // 延迟3秒后检查，避免影响首次加载
    const timer = setTimeout(checkVersion, 3000);
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
      clearTimeout(timer);
    };
  }, []);

  // 计算左边距：桌面端有侧边栏时需要偏移
  const leftOffset = isMobile ? 0 : sidebarWidth;

  return (
    <div
      style={{
        position: floating ? 'fixed' : 'relative',
        bottom: floating ? 0 : 'auto',
        left: floating ? leftOffset : 'auto',
        right: floating ? 0 : 'auto',
        width: '100%',
        padding: isMobile ? '8px 10px' : '8px 16px',
        zIndex: floating ? 100 : 'auto',
        transition: floating ? 'left 0.3s ease' : undefined,
        pointerEvents: floating ? 'none' : 'auto',
        marginTop: floating ? 0 : (isMobile ? 8 : 12),
      }}
    >
      <div
        style={{
          maxWidth: 1240,
          margin: '0 auto',
          borderRadius: isMobile ? 12 : 14,
          border: `1px solid ${editorialBorder}`,
          background: editorialSurface,
          boxShadow: `0 12px 30px ${alphaColor('#000000', 0.18)}`,
          padding: isMobile ? '8px 10px' : '8px 14px',
          pointerEvents: 'auto',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: isMobile ? 8 : 14,
          flexWrap: 'wrap',
        }}
      >
        <Badge dot={hasUpdate} offset={[-4, 4]}>
          <button
            type="button"
            onClick={hasUpdate ? handleVersionClick : undefined}
            title={hasUpdate ? `发现新版本 v${latestVersion}，点击查看` : '当前版本'}
            style={{
              appearance: 'none',
              border: 0,
              padding: 0,
              margin: 0,
              background: 'transparent',
              color: editorialText,
              cursor: hasUpdate ? 'pointer' : 'default',
              fontWeight: 700,
              fontSize: isMobile ? 13 : 14,
              lineHeight: 1.4,
            }}
          >
            {VERSION_INFO.projectName}
          </button>
        </Badge>

        <Space wrap size={[8, 6]}>
          {statusItems.map((item) => (
            <button
              key={item.key}
              type="button"
              onClick={item.clickable ? item.onClick : undefined}
              title={item.title}
              style={{
                appearance: 'none',
                border: `1px solid ${editorialBorder}`,
                borderRadius: 999,
                background: alphaColor('#ffffff', 0.03),
                color: editorialTextSoft,
                fontSize: 11,
                lineHeight: 1.4,
                padding: '4px 9px',
                cursor: item.clickable ? 'pointer' : 'default',
              }}
            >
              {item.key === 'build' ? <ClockCircleOutlined style={{ marginRight: 5, fontSize: 11 }} /> : null}
              {item.label}
            </button>
          ))}
        </Space>

        <Space wrap size={[10, 6]}>
          {resourceLinks.map((item) => (
            <Link
              key={item.key}
              href={item.href}
              target="_blank"
              rel="noopener noreferrer"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 5,
                color: editorialTextSoft,
                fontSize: 12,
              }}
            >
              {item.icon}
              <span>{item.label}</span>
            </Link>
          ))}
          <Text style={{ fontSize: 12, color: editorialTextSoft }}>
            <HeartFilled style={{ color: token.colorError, fontSize: 11, marginRight: 5 }} />
            {VERSION_INFO.author}
          </Text>
        </Space>
      </div>
    </div>
  );
}
