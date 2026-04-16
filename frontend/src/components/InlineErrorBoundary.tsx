import { Component, type ErrorInfo, type ReactNode } from 'react';

type InlineErrorBoundaryProps = {
  children: ReactNode;
  fallback: ReactNode;
};

type InlineErrorBoundaryState = {
  hasError: boolean;
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
      return this.props.fallback;
    }

    return this.props.children;
  }
}
