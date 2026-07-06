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
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const selectedProjectCount = selectedProjectIds.length;
  const enabledExportOptionCount = [
    exportOptions.includeWritingStyles,
    exportOptions.includeCareers,
    exportOptions.includeGenerationHistory,
    exportOptions.includeMemories,
    exportOptions.includePlotAnalysis,
  ].filter(Boolean).length;
  const exportGuideSteps = [
    '先确认这轮要打包哪些项目，再决定是做完整备份，还是只导出一批准备迁移的项目。',
    '再勾选需要附带的风格、职业、历史和分析数据，把归档粒度控制在真正需要的范围内。',
    '最后确认项目清单和附带内容无误后再触发导出，避免反复生成体积过大的归档文件。',
  ];
  const exportWorkspaceFocus = exporting
    ? {
        title: '等待当前归档批次导出完成',
        note: '导出已经开始，适合先等待当前文件生成完成，不要重复变更项目勾选或再次触发导出。',
      }
    : selectedProjectCount === 0
      ? {
          title: '先选出这轮要归档的项目',
          note: '当前还没有导出目标，适合先从项目清单里划定范围，再决定是否需要附带更多上下文数据。',
        }
      : enabledExportOptionCount >= 4
        ? {
            title: `准备导出 ${selectedProjectCount} 个高完整度项目归档`,
            note: '当前附带项较多，更接近完整备份。适合再确认是否真的需要把生成历史、记忆和分析数据一并打包。',
          }
        : enabledExportOptionCount <= 1
          ? {
              title: `准备导出 ${selectedProjectCount} 个轻量归档`,
              note: '当前附带项较少，更适合快速迁移或分享基础项目内容。若需要完整恢复环境，记得补上必要的上下文数据。',
            }
          : {
              title: `整理 ${selectedProjectCount} 个项目的导出范围`,
              note: '当前已经选好项目并保留了一部分附带数据，适合最后复核导出用途，再决定是否扩大或收窄归档内容。',
            };

  return (
    <Modal
      title={(
        <Space direction="vertical" size={2}>
          <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Archive Dispatch
          </Text>
          <Text strong style={{ fontSize: 20 }}>
            导出项目
          </Text>
          <Text type="secondary" style={{ fontSize: 13, lineHeight: 1.7 }}>
            选择要打包的项目与附带数据，把当前工作区内容整理成可迁移、可备份的归档文件。
          </Text>
        </Space>
      )}
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={exporting}
      okText={selectedProjectIds.length > 0 ? `导出 (${selectedProjectIds.length})` : '导出'}
      cancelText="取消"
      width={isMobile ? '90%' : 700}
      centered
      okButtonProps={{ disabled: selectedProjectIds.length === 0 }}
      styles={{
        header: {
          borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          paddingBottom: 10,
        },
        body: {
          paddingTop: 20,
        },
      }}
    >
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <Card
          size="small"
          style={{
            borderRadius: 22,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.55fr) minmax(240px, 0.95fr)',
              gap: 14,
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Export Guide
              </Text>
              <Text style={{ lineHeight: 1.7 }}>
                这个模态更像项目归档的出库检查台。现有的项目勾选、附带内容选择和导出触发流程都保持不变，这里只把打包顺序和当前判断重点提前说明。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {exportGuideSteps.map((item, index) => (
                  <span
                    key={item}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 8,
                      padding: '6px 12px',
                      borderRadius: 999,
                      background: token.colorBgContainer,
                      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
                      fontSize: 12,
                    }}
                  >
                    <span style={{ color: token.colorTextTertiary, fontWeight: 700 }}>{index + 1}</span>
                    {item}
                  </span>
                ))}
              </div>
            </div>
            <div
              style={{
                borderRadius: 18,
                padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                background: alphaColor(token.colorBgContainer, 0.98),
                border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
              }}
            >
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                当前工作焦点
              </Text>
              <Text strong style={{ display: 'block', margin: '8px 0 6px', fontSize: 16 }}>
                {exportWorkspaceFocus.title}
              </Text>
              <Text type="secondary" style={{ lineHeight: 1.7 }}>
                {exportWorkspaceFocus.note}
              </Text>
            </div>
          </div>
        </Card>

        <Card
          size="small"
          style={{
            borderRadius: 22,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <div>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Export Scope
              </Text>
              <Text strong style={{ fontSize: 16 }}>
                导出选项
              </Text>
            </div>
            <Text type="secondary" style={{ lineHeight: 1.7 }}>
              根据归档用途决定是否附带风格、职业、生成历史和分析类数据。内容越完整，导出文件通常也会更大。
            </Text>
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

        <Card
          size="small"
          style={{
            borderRadius: 22,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillTertiary, 0.78)} 100%)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
            <div>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Project Ledger
              </Text>
              <Text strong>{"选择项目"} ({exportableProjects.length})</Text>
            </div>
            <Checkbox
              checked={selectedProjectIds.length === exportableProjects.length && exportableProjects.length > 0}
              indeterminate={selectedProjectIds.length > 0 && selectedProjectIds.length < exportableProjects.length}
              onChange={onToggleAll}
            >
              {"全选"}
            </Checkbox>
          </div>
          <div
            style={{
              maxHeight: 300,
              overflowY: 'auto',
              border: `1px solid ${token.colorBorderSecondary}`,
              borderRadius: 16,
              padding: 10,
              background: alphaColor(token.colorBgContainer, 0.98),
            }}
          >
            <Space direction="vertical" style={{ width: '100%' }}>
              {exportableProjects.map((project) => (
                <div
                  key={project.id}
                  style={{
                    padding: '10px 12px',
                    background: selectedProjectIds.includes(project.id)
                      ? `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                      : token.colorBgContainer,
                    borderRadius: 12,
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                    border: `1px solid ${selectedProjectIds.includes(project.id) ? alphaColor(token.colorBorderSecondary, 0.92) : 'transparent'}`,
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
        </Card>
      </Space>
    </Modal>
  );
}
