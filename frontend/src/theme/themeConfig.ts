import type { ThemeConfig } from 'antd';
import { theme } from 'antd';
import type { ThemeMode } from './themeStorage';

export type ResolvedThemeMode = Exclude<ThemeMode, 'system'>;

export const designSansFont = 'Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
export const designCodeFont = '"JetBrains Mono", "Fira Code", ui-monospace, monospace';
export const designDisplayFont = '"Iowan Old Style", "Palatino Linotype", "Book Antiqua", Georgia, serif';

const sharedToken: ThemeConfig['token'] = {
  colorPrimary: '#cc785c',
  colorInfo: '#5db8a6',
  colorSuccess: '#5db872',
  colorWarning: '#e8a55a',
  colorError: '#c64545',
  colorLink: '#cc785c',
  borderRadius: 14,
  borderRadiusSM: 10,
  borderRadiusLG: 20,
  boxShadow: '0 16px 36px color-mix(in srgb, #141413 10%, transparent)',
  boxShadowSecondary: '0 22px 48px color-mix(in srgb, #141413 12%, transparent)',
  wireframe: false,
  fontFamily: designSansFont,
  fontFamilyCode: designCodeFont,
};

const sharedComponents: ThemeConfig['components'] = {
  Button: {
    borderRadius: 14,
    controlHeight: 40,
    controlHeightLG: 46,
    contentFontSize: 14,
    contentFontSizeLG: 15,
    fontWeight: 600,
    primaryShadow: '0 18px 32px color-mix(in srgb, #cc785c 34%, transparent)',
  },
  Card: {
    borderRadiusLG: 20,
    boxShadow: '0 18px 42px color-mix(in srgb, #141413 8%, transparent)',
  },
  Input: {
    borderRadius: 14,
    activeBorderColor: '#cc785c',
    hoverBorderColor: '#cc785c',
    activeShadow: '0 0 0 3px color-mix(in srgb, #cc785c 14%, transparent)',
  },
  InputNumber: {
    borderRadius: 14,
    activeBorderColor: '#cc785c',
    hoverBorderColor: '#cc785c',
  },
  Select: {
    borderRadius: 14,
    optionSelectedBg: '#efe9de',
    activeBorderColor: '#cc785c',
    hoverBorderColor: '#cc785c',
  },
  Segmented: {
    borderRadius: 14,
    itemActiveBg: '#efe9de',
    itemSelectedBg: '#ffffff',
    itemSelectedColor: '#141413',
    trackBg: '#f5f0e8',
  },
  Menu: {
    itemBorderRadius: 14,
    itemHeight: 40,
    itemSelectedBg: '#efe9de',
    itemSelectedColor: '#141413',
    itemHoverColor: '#141413',
    itemColor: '#6c6a64',
    groupTitleColor: '#8e8b82',
    iconSize: 15,
  },
  Layout: {
    triggerBg: '#181715',
    triggerColor: '#faf9f5',
  },
  Drawer: {
    colorBgElevated: '#faf9f5',
    borderRadiusLG: 20,
  },
  Modal: {
    borderRadiusLG: 20,
  },
  Tooltip: {
    colorBgSpotlight: sharedToken.colorPrimary,
  },
};

const lightThemeConfig: ThemeConfig = {
  algorithm: theme.defaultAlgorithm,
  token: {
    ...sharedToken,
    colorBgBase: '#faf9f5',
    colorTextBase: '#141413',
    colorBgLayout: '#f5f0e8',
    colorBgContainer: '#fffaf3',
    colorBorder: '#e6dfd8',
    colorBorderSecondary: '#ebe6df',
    colorFillAlter: '#efe9de',
    colorFillSecondary: '#f5f0e8',
    colorTextSecondary: '#3d3d3a',
    colorTextTertiary: '#6c6a64',
    colorTextQuaternary: '#8e8b82',
  },
  components: {
    ...sharedComponents,
    Layout: {
      bodyBg: '#f5f0e8',
      headerBg: '#fffaf3',
      siderBg: '#fffaf3',
    },
    Menu: {
      ...sharedComponents.Menu,
      itemSelectedBg: '#efe9de',
      itemSelectedColor: '#141413',
    },
  },
};

const darkThemeConfig: ThemeConfig = {
  algorithm: theme.darkAlgorithm,
  token: {
    ...sharedToken,
    colorBgBase: '#141311',
    colorTextBase: '#faf9f5',
    colorBgLayout: '#100f0e',
    colorBgContainer: '#181715',
    colorBorder: '#2a2724',
    colorBorderSecondary: '#312e2b',
    colorFillAlter: '#252320',
    colorFillSecondary: '#1f1e1b',
    colorTextSecondary: '#e5ded5',
    colorTextTertiary: '#a09d96',
    colorTextQuaternary: '#8e8b82',
    boxShadow: '0 18px 42px color-mix(in srgb, #000000 28%, transparent)',
    boxShadowSecondary: '0 24px 56px color-mix(in srgb, #000000 34%, transparent)',
  },
  components: {
    ...sharedComponents,
    Layout: {
      bodyBg: '#100f0e',
      headerBg: '#181715',
      siderBg: '#181715',
    },
    Button: {
      ...sharedComponents.Button,
      defaultBg: '#252320',
      defaultBorderColor: '#312e2b',
      defaultColor: '#faf9f5',
    },
    Input: {
      ...sharedComponents.Input,
      activeBg: '#1f1e1b',
      hoverBg: '#1f1e1b',
    },
    Select: {
      ...sharedComponents.Select,
      optionSelectedBg: '#252320',
      selectorBg: '#1f1e1b',
    },
    Segmented: {
      ...sharedComponents.Segmented,
      itemActiveBg: '#252320',
      itemSelectedBg: '#181715',
      itemSelectedColor: '#faf9f5',
      trackBg: '#1f1e1b',
    },
    Menu: {
      ...sharedComponents.Menu,
      itemSelectedBg: '#252320',
      itemSelectedColor: '#faf9f5',
      itemHoverColor: '#faf9f5',
      itemColor: '#a09d96',
      groupTitleColor: '#8e8b82',
    },
    Drawer: {
      ...sharedComponents.Drawer,
      colorBgElevated: '#181715',
    },
  },
};

export const getThemeConfig = (mode: ResolvedThemeMode): ThemeConfig => {
  return mode === 'dark' ? darkThemeConfig : lightThemeConfig;
};
