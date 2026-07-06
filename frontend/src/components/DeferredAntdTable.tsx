import { Table } from 'antd';
import type { TableProps } from 'antd';
import type { CSSProperties } from 'react';

type DeferredAntdTableProps = Record<string, unknown>;

const tableFrameStyle: CSSProperties = {
  position: 'relative',
  overflow: 'hidden',
  borderRadius: 24,
  border: '1px solid rgba(148, 163, 184, 0.16)',
  background: 'linear-gradient(180deg, rgba(255,255,255,0.96) 0%, rgba(248, 250, 252, 0.98) 100%)',
  boxShadow: '0 20px 48px rgba(15, 23, 42, 0.08)',
};

const tableGlowStyle: CSSProperties = {
  position: 'absolute',
  inset: '-20% auto auto 72%',
  width: 220,
  height: 220,
  borderRadius: '50%',
  background: 'radial-gradient(circle, rgba(34, 197, 94, 0.12) 0%, rgba(34, 197, 94, 0) 72%)',
  pointerEvents: 'none',
};

const tableInnerStyle: CSSProperties = {
  position: 'relative',
  zIndex: 1,
};

export default function DeferredAntdTable(props: DeferredAntdTableProps) {
  return (
    <div style={tableFrameStyle}>
      <div style={tableGlowStyle} />
      <div style={tableInnerStyle}>
        <Table {...(props as TableProps<Record<string, unknown>>)} />
      </div>
    </div>
  );
}
