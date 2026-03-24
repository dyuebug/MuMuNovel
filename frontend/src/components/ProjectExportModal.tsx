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
      title="\u5bfc\u51fa\u9879\u76ee"
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={exporting}
      okText={selectedProjectIds.length > 0 ? `\u5bfc\u51fa (${selectedProjectIds.length})` : '\u5bfc\u51fa'}
      cancelText="\u53d6\u6d88"
      width={isMobile ? '90%' : 700}
      centered
      okButtonProps={{ disabled: selectedProjectIds.length === 0 }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <Card size="small" style={{ background: token.colorFillTertiary }}>
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <Text strong>{"\u5bfc\u51fa\u9009\u9879"}</Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px 24px' }}>
              <Checkbox checked={exportOptions.includeWritingStyles} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeWritingStyles: event.target.checked }))}>{"\u5199\u4f5c\u98ce\u683c"}</Checkbox>
              <Checkbox checked={exportOptions.includeCareers} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeCareers: event.target.checked }))}>{"\u804c\u4e1a\u7cfb\u7edf"}</Checkbox>
              <Tooltip title="\u5305\u542b\u751f\u6210\u5386\u53f2\u8bb0\u5f55\uff0c\u6587\u4ef6\u53ef\u80fd\u8f83\u5927">
                <Checkbox checked={exportOptions.includeGenerationHistory} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeGenerationHistory: event.target.checked }))}>{"\u751f\u6210\u5386\u53f2"}</Checkbox>
              </Tooltip>
              <Tooltip title="\u5305\u542b\u6545\u4e8b\u8bb0\u5fc6\u6570\u636e\uff0c\u6587\u4ef6\u53ef\u80fd\u8f83\u5927">
                <Checkbox checked={exportOptions.includeMemories} onChange={(event) => setExportOptions((prev) => ({ ...prev, includeMemories: event.target.checked }))}>{"\u6545\u4e8b\u8bb0\u5fc6"}</Checkbox>
              </Tooltip>
              <Tooltip title="\u5305\u542b AI \u5267\u60c5\u5206\u6790\u6570\u636e">
                <Checkbox checked={exportOptions.includePlotAnalysis} onChange={(event) => setExportOptions((prev) => ({ ...prev, includePlotAnalysis: event.target.checked }))}>{"\u5267\u60c5\u5206\u6790"}</Checkbox>
              </Tooltip>
            </div>
          </Space>
        </Card>

        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
            <Text>{"\u9009\u62e9\u9879\u76ee"} ({exportableProjects.length})</Text>
            <Checkbox
              checked={selectedProjectIds.length === exportableProjects.length && exportableProjects.length > 0}
              indeterminate={selectedProjectIds.length > 0 && selectedProjectIds.length < exportableProjects.length}
              onChange={onToggleAll}
            >
              {"\u5168\u9009"}
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
                      {formatWordCount(project.current_words || 0)} {"\u5b57 \u00b7 "}{renderProjectStatus(project)}
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
