import { Table } from 'antd';
import type { TableProps } from 'antd';

type DeferredAntdTableProps = Record<string, unknown>;

export default function DeferredAntdTable(props: DeferredAntdTableProps) {
  return <Table {...(props as TableProps<Record<string, unknown>>)} />;
}
