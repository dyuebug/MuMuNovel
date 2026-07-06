import { Component, type CSSProperties, type ErrorInfo, type ReactNode } from 'react';

type InlineErrorBoundaryProps = {
  children: ReactNode;
  fallback: ReactNode;
};

type InlineErrorBoundaryState = {
  hasError: boolean;
};

const boundaryShellStyle: CSSProperties = {
  position: 'relative',
  overflow: 'hidden',
  borderRadius: 24,
  border: '1px solid rgba(15, 23, 42, 0.08)',
  background: 'linear-gradient(145deg, rgba(255,255,255,0.96) 0%, rgba(245, 247, 250, 0.98) 52%, rgba(238, 243, 248, 0.98) 100%)',
  boxShadow: '0 24px 60px rgba(15, 23, 42, 0.12)',
  padding: 24,
};

const boundaryGlowStyle: CSSProperties = {
  position: 'absolute',
  inset: 'auto -12% 62% auto',
  width: 220,
  height: 220,
  borderRadius: '50%',
  background: 'radial-gradient(circle, rgba(56, 189, 248, 0.18) 0%, rgba(56, 189, 248, 0) 72%)',
  pointerEvents: 'none',
};

const boundaryContentStyle: CSSProperties = {
  position: 'relative',
  zIndex: 1,
  display: 'flex',
  flexDirection: 'column',
  gap: 16,
};

const boundaryEyebrowStyle: CSSProperties = {
  display: 'inline-flex',
  alignSelf: 'flex-start',
  borderRadius: 999,
  padding: '6px 12px',
  background: 'rgba(15, 23, 42, 0.06)',
  color: '#475569',
  fontSize: 12,
  fontWeight: 700,
  letterSpacing: '0.12em',
  textTransform: 'uppercase',
};

const boundaryTitleStyle: CSSProperties = {
  margin: 0,
  color: '#0f172a',
  fontSize: 20,
  fontWeight: 700,
  lineHeight: 1.35,
};

const boundaryDescriptionStyle: CSSProperties = {
  margin: 0,
  color: '#475569',
  fontSize: 14,
  lineHeight: 1.7,
};

const fallbackSurfaceStyle: CSSProperties = {
  borderRadius: 20,
  border: '1px solid rgba(148, 163, 184, 0.18)',
  background: 'rgba(255, 255, 255, 0.78)',
  boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.6)',
  padding: '4px 18px',
};

export default class InlineErrorBoundary extends Component<InlineErrorBoundaryProps, InlineErrorBoundaryState> {
  state: InlineErrorBoundaryState = {
    hasError: false,
  };

  static getDerivedStateFromError(): InlineErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('InlineErrorBoundary caught render error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <section style={boundaryShellStyle}>
          <div style={boundaryGlowStyle} />
          <div style={boundaryContentStyle}>
            <span style={boundaryEyebrowStyle}>Panel Recovery</span>
            <div>
              <h3 style={boundaryTitleStyle}>当前面板暂时不可用</h3>
              <p style={boundaryDescriptionStyle}>
                我们保留了原有流程与状态，只把异常反馈整理成更清晰的阅读顺序，方便你判断是否需要关闭面板后重试。
              </p>
            </div>
            <div style={fallbackSurfaceStyle}>{this.props.fallback}</div>
          </div>
        </section>
      );
    }

    return this.props.children;
  }
}
