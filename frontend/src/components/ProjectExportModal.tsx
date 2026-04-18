import { Card, Checkbox, Modal, Space, Tooltip, Typography } from 'antd';
import type { Dispatch, ReactNode, SetStateAction } from 'react';
import type { Project } from '../types';

type ProjectExportOptions = {
  includeWritingStyles: boolean;
  includeGenerationHistory: boolean;
  includeCareers: boolean;
  includeMemories: boolean;
  includePlotAnalysis: boolean;
};

type ProjectExportModalToken = {
  colorFillTertiary: string;
  colorBorderSecondary: string;
  colorPrimaryBg: string;
  colorBgContainer: string;
  colorTextTertiary: string;
};

type ProjectExportModalProps = {
  open: boolean;
  isMobile: boolean;
  exporting: boolean;
  exportableProjects: Project[];
  selectedProjectIds: string[];
  exportOptions: ProjectExportOptions;
  setExportOptions: Dispatch<SetStateAction<ProjectExportOptions>>;
  token: ProjectExportModalToken;
  formatWordCount: (count: number) => string;
  renderProjectStatus: (project: Project) => ReactNode;
  onOk: () => void;
  onCancel: () => void;
  onToggleAll: () => void;
  onToggleProject: (projectId: string) => void;
};

const { Text } = Typography;

export default function ProjectExportModal({
  open,
  isMobile,
  exporting,
  exportableProjects,
  selectedProjectIds,
  exportOptions,
  setExportOptions,
  token,
  formatWordCount,
  renderProjectStatus,
  onOk,
  onCancel,
  onToggleAll,
  onToggleProject,
}: ProjectExportModalProps) {
  return (
    <Modal
      title="导出项目"
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={exporting}
      okText={selectedProjectIds.length > 0 ? `导出 (${selectedProjectIds.length})` : '导出'}
      cancelText="取消"
      width={isMobile ? '90%' : 700}
      centered
      okButtonProps={{ disabled: selectedProjectIds.length === 0 }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <Card size="small" style={{ background: token.colorFillTertiary }}>
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <Text strong>{"导出选项"}</Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px 24px' }}>
              <Checkbox checked={exportOptions.includeWritingStyles} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeWritingStyles: event.target.checked }))}>{"写作风格"}</Checkbox>
              <Checkbox checked={exportOptions.includeCareers} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeCareers: event.target.checked }))}>{"职业系统"}</Checkbox>
              <Tooltip title="包含生成历史记录，文件可能较大">
                <Checkbox checked={exportOptions.includeGenerationHistory} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeGenerationHistory: event.target.checked }))}>{"生成历史"}</Checkbox>
              </Tooltip>
              <Tooltip title="包含故事记忆数据，文件可能较大">
                <Checkbox checked={exportOptions.includeMemories} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeMemories: event.target.checked }))}>{"故事记忆"}</Checkbox>
              </Tooltip>
              <Tooltip title="包含 AI 剧情分析数据">
                <Checkbox checked={exportOptions.includePlotAnalysis} onChange={(event) => setExportOptions((prev) => ({ ...prev, includePlotAnalysis: event.target.checked }))}>{"剧情分析"}</Checkbox>
              </Tooltip>
            </div>
          </Space>
        </Card>

        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
            <Text>{"选择项目"} ({exportableProjects.length})</Text>
            <Checkbox
              checked={selectedProjectIds.length === exportableProjects.length && exportableProjects.length > 0}
              indeterminate={selectedProjectIds.length > 0 && selectedProjectIds.length < exportableProjects.length}
              onChange={onToggleAll}
            >
              {"全选"}
            </Checkbox>
          </div>
          <div style={{ maxHeight: 300, overflowY: 'auto', border: `1px solid ${token.colorBorderSecondary}`, borderRadius: 8, padding: 8 }}>
            <Space direction="vertical" style={{ width: '100%' }}>
              {exportableProjects.map((project) => (
                <div
                  key={project.id}
                  style={{
                    padding: '8px 12px',
                    background: selectedProjectIds.includes(project.id) ? token.colorPrimaryBg : token.colorBgContainer,
                    borderRadius: 6,
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                  }}
                  onClick={() => onToggleProject(project.id)}
                >
                  <Checkbox checked={selectedProjectIds.includes(project.id)} />
                  <div style={{ flex: 1 }}>
                    <div>{project.title}</div>
                    <div style={{ fontSize: 12, color: token.colorTextTertiary }}>
                      {formatWordCount(project.current_words || 0)} {"字 · "}{renderProjectStatus(project)}
                    </div>
                  </div>
                </div>
              ))}
            </Space>
          </div>
        </div>
      </Space>
    </Modal>
  );
}
