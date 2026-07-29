import { Suspense, lazy, memo, useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { Button, Modal, Form, Input, Select, message, Row, Col, Empty, Tabs, Divider, Typography, Space, Checkbox, theme, Card } from 'antd';
import { ThunderboltOutlined, UserOutlined, TeamOutlined, PlusOutlined, ExportOutlined, ImportOutlined, DownloadOutlined } from '@ant-design/icons';
import { useShallow } from 'zustand/react/shallow';
import { useStore } from '../store';
import { isActiveBackgroundTask, useBackgroundTaskStore } from '../store/backgroundTasks';
import { useCharacterSync } from '../store/hooks';
import { charactersPageGridConfig } from '../components/CardStyles';
import { CharacterCard } from '../components/CharacterCard';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import { designDisplayFont } from '../theme/themeConfig';
import type { CSSProperties } from 'react';
import type { Character, ApiError } from '../types';
import { backgroundTaskApi, characterApi } from '../services/modularApi';
import { getCachedProjectCareers, loadProjectCareers } from '../services/projectCareers';
import { formatBackgroundTaskError } from '../utils/taskPolling';
import { useRestorableBackgroundTaskPolling } from '../hooks/useRestorableBackgroundTaskPolling';
import { useBackgroundTaskOutputStream } from '../hooks/useBackgroundTaskOutputStream';
import { isRequestCancelledError } from '../services/core/httpClient';



const { Title, Paragraph, Text } = Typography;
const { TextArea } = Input;



const LazySSELoadingOverlay = lazy(async () => {
  const module = await import('../components/SSELoadingOverlay');
  return { default: module.SSELoadingOverlay };
});

const LazyCharacterFormModal = lazy(() => import('../components/CharacterFormModal'));



interface Career {
  id: string;
  name: string;
  type: 'main' | 'sub';
  max_stage: number;
}



// 副职业数据类型
interface SubCareerData {
  career_id: string;
  stage: number;
}



interface SelectableCharacterCardProps {
  item: Character;
  selected: boolean;
  cardColStyle: CSSProperties;
  onToggle: (id: string) => void;
  onEdit: (character: Character) => void;
  onDelete: (id: string) => void;
  onExport: (id: string) => void;
}



const SelectableCharacterCard = memo(function SelectableCharacterCard({
  item,
  selected,
  cardColStyle,
  onToggle,
  onEdit,
  onDelete,
  onExport,
}: SelectableCharacterCardProps) {
  return (
    <Col
      xs={24}
      sm={charactersPageGridConfig.sm}
      md={charactersPageGridConfig.md}
      lg={charactersPageGridConfig.lg}
      xl={charactersPageGridConfig.xl}
      style={cardColStyle}
    >
      <div style={{ position: 'relative' }}>
        <Checkbox
          checked={selected}
          onChange={() => onToggle(item.id)}
          style={{ position: 'absolute', top: 8, left: 8, zIndex: 1 }}
        />
        <CharacterCard
          character={item}
          onEdit={onEdit}
          onDelete={onDelete}
          onExport={() => onExport(item.id)}
        />
      </div>
    </Col>
  );
});



// 角色创建表单值类型
interface CharacterFormValues {
  name: string;
  age?: string;
  gender?: string;
  role_type?: string;
  personality?: string;
  appearance?: string;
  background?: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_career_data?: SubCareerData[];
  // 组织字段
  organization_type?: string;
  organization_purpose?: string;
  organization_members?: string;
  power_level?: number;
  location?: string;
  motto?: string;
  color?: string;
}



// 角色创建数据类型
interface CharacterCreateData {
  project_id: string;
  name: string;
  is_organization: boolean;
  age?: string;
  gender?: string;
  role_type?: string;
  personality?: string;
  appearance?: string;
  background?: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_careers?: string;
  organization_type?: string;
  organization_purpose?: string;
  organization_members?: string;
  power_level?: number;
  location?: string;
  motto?: string;
  color?: string;
}



// 角色更新数据类型
interface CharacterUpdateData {
  name?: string;
  age?: string;
  gender?: string;
  role_type?: string;
  personality?: string;
  appearance?: string;
  background?: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_careers?: string;
  organization_type?: string;
  organization_purpose?: string;
  organization_members?: string;
  power_level?: number;
  location?: string;
  motto?: string;
  color?: string;
}




const INITIAL_CHARACTER_RENDER_COUNT = 8;
const CHARACTER_TASK_REFRESH_KEY_PREFIX = 'background-task-refresh:characters:';
const CHARACTER_TASK_REFRESH_RETRY_DELAY_MS = 2000;

const hasCharacterTaskRefreshBeenHandled = (taskId: string): boolean => {
  try {
    return sessionStorage.getItem(`${CHARACTER_TASK_REFRESH_KEY_PREFIX}${taskId}`) === '1';
  } catch {
    return false;
  }
};

const markCharacterTaskRefreshHandled = (taskId: string) => {
  try {
    sessionStorage.setItem(`${CHARACTER_TASK_REFRESH_KEY_PREFIX}${taskId}`, '1');
  } catch {
    // ignore sessionStorage failures
  }
};

const createCharacterRefreshTaskLock = () => {
  const inFlightTaskIds = new Set<string>();

  return {
    acquire(taskId: string) {
      if (!taskId || inFlightTaskIds.has(taskId)) {
        return false;
      }
      inFlightTaskIds.add(taskId);
      return true;
    },
    release(taskId: string) {
      if (!taskId) {
        return;
      }
      inFlightTaskIds.delete(taskId);
    },
  };
};

const isCharacterGenerationTaskType = (taskType?: string | null): taskType is 'character_generate' | 'organization_generate' =>
  taskType === 'character_generate' || taskType === 'organization_generate';

const selectActiveCharacterGenerationTask = (
  tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
  projectId?: string | null,
) => {
  if (!projectId) {
    return null;
  }

  return Object.values(tasks)
    .filter(
      (task) => task.projectId === projectId
        && isCharacterGenerationTaskType(task.taskType)
        && isActiveBackgroundTask(task)
    )
    .sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null;
};

const selectCompletedCharacterRefreshTaskSignature = (
  tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
  projectId?: string | null,
): string => {
  if (!projectId) {
    return '';
  }

  const completedTask = Object.values(tasks)
    .filter(
      (task) => task.projectId === projectId
        && (task.taskType === 'character_generate' || task.taskType === 'organization_generate')
        && task.status === 'completed'
        && !hasCharacterTaskRefreshBeenHandled(task.taskId)
    )
    .sort((left, right) => (right.completedAt ?? right.updatedAt) - (left.completedAt ?? left.updatedAt))[0];

  if (!completedTask) {
    return '';
  }

  return `${completedTask.taskId}:${completedTask.completedAt ?? completedTask.updatedAt}`;
};

const getCharacterGenerationSuccessMessage = (taskType: 'character_generate' | 'organization_generate') =>
  taskType === 'organization_generate' ? '智能生成组织成功' : '智能生成角色成功';

export default function Characters() {
  const { token } = theme.useToken();
  const currentProject = useStore((state) => state.currentProject);
  const projectCharacters = useStore(
    useShallow((state) => state.characters.filter((character) => character.project_id === currentProject?.id)),
  );
  const activeTrackedGenerationTask = useBackgroundTaskStore(
    (state) => selectActiveCharacterGenerationTask(state.tasks, currentProject?.id)
  );
  const completedCharacterRefreshTaskSignature = useBackgroundTaskStore(
    (state) => selectCompletedCharacterRefreshTaskSignature(state.tasks, currentProject?.id)
  );
  const [isGenerating, setIsGenerating] = useState(false);
  const [outputTaskId, setOutputTaskId] = useState<string | null>(null);
  const modelOutput = useBackgroundTaskOutputStream(outputTaskId);
  const [isCancellingTask, setIsCancellingTask] = useState(false);
  const [progress, setProgress] = useState(0);
  const [progressMessage, setProgressMessage] = useState('');
  const [activeTab, setActiveTab] = useState<'all' | 'character' | 'organization'>('all');
  const [generateForm] = Form.useForm();
  const [generateOrgForm] = Form.useForm();
  const [createForm] = Form.useForm();
  const [editForm] = Form.useForm();
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [createType, setCreateType] = useState<'character' | 'organization'>('character');
  const [editingCharacter, setEditingCharacter] = useState<Character | null>(null);
  const [mainCareers, setMainCareers] = useState<Career[]>([]);
  const [subCareers, setSubCareers] = useState<Career[]>([]);
  const [selectedCharacters, setSelectedCharacters] = useState<string[]>([]);
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const taskCreationInFlightRef = useRef(false);
  const completedCharacterRefreshLockRef = useRef(createCharacterRefreshTaskLock());
  const completedCharacterRefreshRetryTimerRef = useRef<number | null>(null);
  const [completedCharacterRefreshRetryTick, setCompletedCharacterRefreshRetryTick] = useState(0);

  const {
    refreshCharacters,
    createCharacter,
    updateCharacter,
    deleteCharacter
  } = useCharacterSync();

  const scheduleCompletedCharacterRefreshRetry = useCallback(() => {
    if (completedCharacterRefreshRetryTimerRef.current) {
      clearTimeout(completedCharacterRefreshRetryTimerRef.current);
    }

    completedCharacterRefreshRetryTimerRef.current = window.setTimeout(() => {
      completedCharacterRefreshRetryTimerRef.current = null;
      setCompletedCharacterRefreshRetryTick((value) => value + 1);
    }, CHARACTER_TASK_REFRESH_RETRY_DELAY_MS);
  }, []);



  const characters = useMemo(() => {
    if (!currentProject?.id) {
      return [];
    }

    return projectCharacters;
  }, [currentProject?.id, projectCharacters]);



  const fetchCareers = useCallback(async (projectId = currentProject?.id) => {
    if (!projectId) return;



    try {
      const nextCareers = await loadProjectCareers(projectId);
      setMainCareers(nextCareers.mainCareers);
      setSubCareers(nextCareers.subCareers);
    } catch (error) {
      console.error('load careers failed:', error);
    }
  }, [currentProject?.id]);



  const ensureCareersLoaded = useCallback((projectId = currentProject?.id) => {
    if (!projectId) return;



    const cachedCareers = getCachedProjectCareers(projectId);
    if (cachedCareers) {
      setMainCareers(cachedCareers.mainCareers);
      setSubCareers(cachedCareers.subCareers);
      return;
    }



    void fetchCareers(projectId);
  }, [currentProject?.id, fetchCareers]);



  useEffect(() => {
    if (!currentProject?.id) return;

    const projectId = currentProject.id;
    const cachedCareers = getCachedProjectCareers(projectId);
    const hasProjectCharacters = useStore.getState().characters.some((character) => character.project_id === projectId);

    if (!hasProjectCharacters) {
      void refreshCharacters(projectId);
    }

    if (cachedCareers) {
      setMainCareers(cachedCareers.mainCareers);
      setSubCareers(cachedCareers.subCareers);
    }
  }, [currentProject?.id, refreshCharacters]);
  const [modal, contextHolder] = Modal.useModal();



  const handleDeleteCharacter = useCallback(async (id: string) => {
    try {
      await deleteCharacter(id);
      message.success('删除成功');
    } catch {
      message.error('删除失败');
    }
  }, [deleteCharacter]);



  const { currentTaskIdRef, startTaskPolling, stopTaskPolling } = useRestorableBackgroundTaskPolling({
    projectId: currentProject?.id,
    activeTrackedTask: activeTrackedGenerationTask,
    canRestore: !isGenerating && !taskCreationInFlightRef.current,
    isMatchingTask: (task) => isCharacterGenerationTaskType(task.task_type) && (task.status === 'pending' || task.status === 'running'),
    onRestoreTask: ({ taskId, progress: progressValue, message: messageValue }) => {
      setOutputTaskId(taskId);
      setIsGenerating(true);
      setIsCancellingTask(false);
      setProgress(progressValue || 0);
      setProgressMessage(messageValue || '正在恢复生成任务...');
    },
    createPollingOptions: () => ({
      pollTask: (currentPollingTaskId) => backgroundTaskApi.getTaskStatus(currentPollingTaskId),
      onTask: (task) => {
        setProgress(task.progress || 0);
        setProgressMessage(task.message || '');
      },
      onCompleted: (task) => {
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setOutputTaskId(null);
        setIsCancellingTask(false);
        setIsGenerating(false);
        message.success(
          isCharacterGenerationTaskType(task.task_type)
            ? getCharacterGenerationSuccessMessage(task.task_type)
            : '智能生成成功',
        );
      },
      onFailed: (task) => {
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setOutputTaskId(null);
        setIsCancellingTask(false);
        setIsGenerating(false);
        message.error(formatBackgroundTaskError(task.error, task.message, '生成失败'));
      },
      onCancelled: (task) => {
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setOutputTaskId(null);
        setIsCancellingTask(false);
        setIsGenerating(false);
        message.info(task.message || '任务已取消');
      },
      onPollingError: (error) => {
        if (isRequestCancelledError(error)) {
          return;
        }
        console.error('轮询角色或组织生成任务失败:', error);
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setOutputTaskId(null);
        setIsCancellingTask(false);
        setIsGenerating(false);
        setProgressMessage('生成状态同步失败，请刷新后重试');
        void refreshCharacters();
        message.error('生成状态同步失败，请刷新后重试');
      },
    }),
  });

  useEffect(() => {
    setOutputTaskId(null);
  }, [currentProject?.id]);

  useEffect(() => {
    return () => {
      currentTaskIdRef.current = null;
      taskCreationInFlightRef.current = false;
    };
  }, [currentTaskIdRef]);



  useEffect(() => {
    if (!currentProject?.id || currentTaskIdRef.current || isGenerating) {
      return;
    }

    if (!completedCharacterRefreshTaskSignature) {
      return;
    }

    const [taskId] = completedCharacterRefreshTaskSignature.split(':');
    if (!taskId) {
      return;
    }
    if (!completedCharacterRefreshLockRef.current.acquire(taskId)) {
      return;
    }

    void refreshCharacters(currentProject.id)
      .then(() => {
        markCharacterTaskRefreshHandled(taskId);
      })
      .catch((error) => {
        console.error('刷新角色列表失败:', error);
        scheduleCompletedCharacterRefreshRetry();
      })
      .finally(() => {
        completedCharacterRefreshLockRef.current.release(taskId);
      });
  }, [
    completedCharacterRefreshRetryTick,
    completedCharacterRefreshTaskSignature,
    currentProject?.id,
    currentTaskIdRef,
    isGenerating,
    refreshCharacters,
    scheduleCompletedCharacterRefreshRetry,
  ]);

  useEffect(() => {
    return () => {
      if (completedCharacterRefreshRetryTimerRef.current) {
        clearTimeout(completedCharacterRefreshRetryTimerRef.current);
        completedCharacterRefreshRetryTimerRef.current = null;
      }
    };
  }, []);



  const handleGenerateBackground = async (values: { name?: string; role_type: string; background?: string }) => {
    if (isGenerating || activeTrackedGenerationTask) {
      message.info('已有后台生成任务在运行，请稍后查看结果');
      return;
    }



    taskCreationInFlightRef.current = true;
    setIsGenerating(true);
    setIsCancellingTask(false);
    setProgress(0);
    setProgressMessage('正在创建后台任务...');



    try {
      const task = await backgroundTaskApi.createTask({
        task_type: 'character_generate',
        project_id: currentProject!.id,
        payload: {
          name: values.name,
          role_type: values.role_type,
          background: values.background,
        }
      });



      message.success('后台角色生成任务已启动，可继续进行其他操作');
      currentTaskIdRef.current = task.task_id;
      setOutputTaskId(task.task_id);
      startTaskPolling(task.task_id);
      taskCreationInFlightRef.current = false;
    } catch (error: unknown) {
      stopTaskPolling();
      currentTaskIdRef.current = null;
      setOutputTaskId(null);
      taskCreationInFlightRef.current = false;
      setIsCancellingTask(false);
      setIsGenerating(false);
      const errorMessage = error instanceof Error ? error.message : '智能生成失败';
      message.error(errorMessage);
    }
  };



  const handleGenerateOrganizationBackground = async (values: {
    name?: string;
    organization_type?: string;
    background?: string;
    requirements?: string;
  }) => {
    if (isGenerating || activeTrackedGenerationTask) {
      message.info('已有后台生成任务在运行，请稍后查看结果');
      return;
    }



    taskCreationInFlightRef.current = true;
    setIsGenerating(true);
    setIsCancellingTask(false);
    setProgress(0);
    setProgressMessage('正在创建后台任务...');



    try {
      const task = await backgroundTaskApi.createTask({
        task_type: 'organization_generate',
        project_id: currentProject!.id,
        payload: {
          name: values.name,
          organization_type: values.organization_type,
          background: values.background,
          requirements: values.requirements,
        }
      });



      message.success('后台组织生成任务已启动，可继续进行其他操作');
      currentTaskIdRef.current = task.task_id;
      setOutputTaskId(task.task_id);
      startTaskPolling(task.task_id);
      taskCreationInFlightRef.current = false;
    } catch (error: unknown) {
      stopTaskPolling();
      currentTaskIdRef.current = null;
      setOutputTaskId(null);
      taskCreationInFlightRef.current = false;
      setIsCancellingTask(false);
      setIsGenerating(false);
      const errorMessage = error instanceof Error ? error.message : '智能生成失败';
      message.error(errorMessage);
    }
  };



  const handleCancelGeneratingTask = async () => {
    const taskId = currentTaskIdRef.current;
    if (!taskId || isCancellingTask) {
      return;
    }



    setIsCancellingTask(true);
    try {
      await backgroundTaskApi.cancelTask(taskId);
      message.info('正在取消后台任务...');
      stopTaskPolling();
      currentTaskIdRef.current = null;
      setOutputTaskId(null);
      setIsGenerating(false);
      setProgress(0);
      setProgressMessage('');
    } catch (error) {
      console.error('取消角色/组织生成任务失败:', error);
      message.error('取消任务失败，请重试');
    } finally {
      setIsCancellingTask(false);
    }
  };



  const handleGenerate = async (values: { name?: string; role_type: string; background?: string }) => {
    return handleGenerateBackground(values);
  };



  const handleGenerateOrganization = async (values: {
    name?: string;
    organization_type?: string;
    background?: string;
    requirements?: string;
  }) => {
    return handleGenerateOrganizationBackground(values);
  };



  const handleCreateCharacter = async (values: CharacterFormValues) => {
    try {
      const createData: CharacterCreateData = {
        project_id: currentProject!.id,
        name: values.name,
        is_organization: createType === 'organization',
      };



      if (createType === 'character') {
        // 角色字段
        createData.age = values.age;
        createData.gender = values.gender;
        createData.role_type = values.role_type || 'supporting';
        createData.personality = values.personality;
        createData.appearance = values.appearance;
        createData.background = values.background;
        
        // 职业字段
        if (values.main_career_id) {
          createData.main_career_id = values.main_career_id;
          createData.main_career_stage = values.main_career_stage || 1;
        }
        
        // 处理副职业数据
        if (values.sub_career_data && Array.isArray(values.sub_career_data) && values.sub_career_data.length > 0) {
          createData.sub_careers = JSON.stringify(values.sub_career_data);
        }
      } else {
        // 组织字段
        createData.organization_type = values.organization_type;
        createData.organization_purpose = values.organization_purpose;
        createData.background = values.background;
        createData.power_level = values.power_level;
        createData.location = values.location;
        createData.motto = values.motto;
        createData.color = values.color;
        createData.role_type = 'supporting'; // 组织默认为配角
      }



      await createCharacter(createData);
      message.success(`${createType === 'character' ? '角色' : '组织'}创建成功`);
      setIsCreateModalOpen(false);
      createForm.resetFields();
    } catch {
      message.error('创建失败');
    }
  };



  const handleEditCharacter = useCallback((character: Character) => {
    setEditingCharacter(character);



    const subCareerData: SubCareerData[] = character.sub_careers?.map((sc) => ({
      career_id: sc.career_id,
      stage: sc.stage || 1
    })) || [];



    editForm.setFieldsValue({
      ...character,
      sub_career_data: subCareerData
    });



    if (!character.is_organization) {
      ensureCareersLoaded(character.project_id);
    }



    setIsEditModalOpen(true);
  }, [editForm, ensureCareersLoaded]);





  const handleUpdateCharacter = async (values: CharacterFormValues) => {
    if (!editingCharacter) return;



    try {
      // 提取副职业数据，剩余的作为更新数据
      const { sub_career_data: subCareerData, ...restValues } = values;
      const updateData: CharacterUpdateData = { ...restValues };



      // 转换为sub_careers格式
      if (subCareerData && Array.isArray(subCareerData) && subCareerData.length > 0) {
        updateData.sub_careers = JSON.stringify(subCareerData);
      } else {
        updateData.sub_careers = JSON.stringify([]);
      }



      await updateCharacter(editingCharacter.id, updateData);
      message.success('更新成功');
      setIsEditModalOpen(false);
      editForm.resetFields();
      setEditingCharacter(null);
    } catch (error) {
      console.error('更新失败:', error);
      message.error('更新失败');
    }
  };

  const closeEditModal = useCallback(() => {
    setIsEditModalOpen(false);
    editForm.resetFields();
    setEditingCharacter(null);
  }, [editForm]);

  const closeCreateModal = useCallback(() => {
    setIsCreateModalOpen(false);
    createForm.resetFields();
  }, [createForm]);



  const handleDeleteCharacterWrapper = useCallback((id: string) => {
    void handleDeleteCharacter(id);
  }, [handleDeleteCharacter]);



  // 导出选中的角色/组织
  const handleExportSelected = async () => {
    if (selectedCharacters.length === 0) {
      message.warning('请至少选择一个角色或组织');
      return;
    }



    try {
      await characterApi.exportCharacters(selectedCharacters);
      message.success(`成功导出 ${selectedCharacters.length} 个角色/组织`);
      setSelectedCharacters([]);
    } catch (error) {
      message.error('导出失败');
      console.error('导出错误:', error);
    }
  };



  // 导出单个角色/组织
  const handleExportSingle = useCallback(async (characterId: string) => {
    try {
      await characterApi.exportCharacters([characterId]);
      message.success('导出成功');
    } catch (error) {
      message.error('导出失败');
      console.error('export failed:', error);
    }
  }, []);



  // 处理文件选择
  const handleFileSelect = async (file: File) => {
    try {
      // 验证文件
      const validation = await characterApi.validateImportCharacters(file);
      
      if (!validation.valid) {
        modal.error({
          title: '文件验证失败',
          centered: true,
          content: (
            <div>
              {validation.errors.map((error, index) => (
                <div key={index} style={{ color: token.colorError }}>• {error}</div>
              ))}
            </div>
          ),
        });
        return;
      }



      // 显示预览对话框
      modal.confirm({
        title: '导入预览',
        width: 500,
        centered: true,
        content: (
          <div>
            <p><strong>文件版本:</strong> {validation.version}</p>
            <Divider style={{ margin: '12px 0' }} />
            <p><strong>将要导入:</strong></p>
            <ul style={{ marginLeft: 20 }}>
              <li>角色: {validation.statistics.characters} 个</li>
              <li>组织: {validation.statistics.organizations} 个</li>
            </ul>
            {validation.warnings.length > 0 && (
              <>
                <Divider style={{ margin: '12px 0' }} />
                <p style={{ color: token.colorWarning }}><strong>⚠️ 警告:</strong></p>
                <ul style={{ marginLeft: 20 }}>
                  {validation.warnings.map((warning, index) => (
                    <li key={index} style={{ color: token.colorWarning }}>{warning}</li>
                  ))}
                </ul>
              </>
            )}
          </div>
        ),
        okText: '确认导入',
        cancelText: '取消',
        onOk: async () => {
          try {
            const result = await characterApi.importCharacters(currentProject!.id, file);
            
            if (result.success) {
              // 显示导入结果
              modal.success({
                title: '导入完成',
                width: 600,
                centered: true,
                content: (
                  <div>
                    <p><strong>✅ 成功导入: {result.statistics.imported} 个</strong></p>
                    {result.details.imported_characters.length > 0 && (
                      <>
                        <p style={{ marginTop: 12, marginBottom: 4 }}>角色:</p>
                        <ul style={{ marginLeft: 20 }}>
                          {result.details.imported_characters.map((name, index) => (
                            <li key={index}>{name}</li>
                          ))}
                        </ul>
                      </>
                    )}
                    {result.details.imported_organizations.length > 0 && (
                      <>
                        <p style={{ marginTop: 12, marginBottom: 4 }}>组织:</p>
                        <ul style={{ marginLeft: 20 }}>
                          {result.details.imported_organizations.map((name, index) => (
                            <li key={index}>{name}</li>
                          ))}
                        </ul>
                      </>
                    )}
                    {result.statistics.skipped > 0 && (
                      <>
                        <Divider style={{ margin: '12px 0' }} />
                        <p style={{ color: token.colorWarning }}>⚠️ 跳过: {result.statistics.skipped} 个</p>
                        <ul style={{ marginLeft: 20 }}>
                          {result.details.skipped.map((name, index) => (
                            <li key={index} style={{ color: token.colorWarning }}>{name}</li>
                          ))}
                        </ul>
                      </>
                    )}
                    {result.warnings.length > 0 && (
                      <>
                        <Divider style={{ margin: '12px 0' }} />
                        <p style={{ color: token.colorWarning }}>⚠️ 警告:</p>
                        <ul style={{ marginLeft: 20 }}>
                          {result.warnings.map((warning, index) => (
                            <li key={index} style={{ color: token.colorWarning }}>{warning}</li>
                          ))}
                        </ul>
                      </>
                    )}
                    {result.details.errors.length > 0 && (
                      <>
                        <Divider style={{ margin: '12px 0' }} />
                        <p style={{ color: token.colorError }}>❌ 失败: {result.statistics.errors} 个</p>
                        <ul style={{ marginLeft: 20 }}>
                          {result.details.errors.map((error, index) => (
                            <li key={index} style={{ color: token.colorError }}>{error}</li>
                          ))}
                        </ul>
                      </>
                    )}
                  </div>
                ),
              });
              
              // 刷新列表
              await refreshCharacters();
              setIsImportModalOpen(false);
            } else {
              message.error(result.message || '导入失败');
            }
          } catch (error: unknown) {
            const apiError = error as ApiError;
            message.error(apiError.response?.data?.detail || '导入失败');
            console.error('导入错误:', error);
          }
        },
      });
    } catch (error: unknown) {
      const apiError = error as ApiError;
      message.error(apiError.response?.data?.detail || '文件验证失败');
      console.error('验证错误:', error);
    }
  };



  // 切换选择
  const toggleSelectCharacter = useCallback((id: string) => {
    setSelectedCharacters(prev =>
      prev.includes(id) ? prev.filter(cid => cid !== id) : [...prev, id]
    );
  }, []);



  // 全选/取消全选
  const toggleSelectAll = () => {
    if (selectedCharacters.length === displayList.length) {
      setSelectedCharacters([]);
    } else {
      setSelectedCharacters(displayList.map(c => c.id));
    }
  };



  const showGenerateModal = () => {
    modal.confirm({
      title: '智能生成角色',
      width: 600,
      centered: true,
      content: (
        <Space direction="vertical" size={12} style={{ width: '100%', marginTop: 12 }}>
          <Card
            size="small"
            variant="borderless"
            style={{ borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-info-bg) 82%, var(--ant-color-bg-container) 18%)' }}
          >
            <Text type="secondary">
              角色生成更适合用来起草候选人物。先给出角色定位和最基本的背景方向，后续再进入编辑表单细修。
            </Text>
          </Card>
          <Form form={generateForm} layout="vertical">
            <Form.Item
              label="角色名称"
              name="name"
            >
              <Input placeholder="如：张三、李四（可选，系统会自动生成）" />
            </Form.Item>
            <Form.Item
              label="角色定位"
              name="role_type"
              rules={[{ required: true, message: '请选择角色定位' }]}
            >
              <Select placeholder="选择角色定位">
                <Select.Option value="protagonist">主角</Select.Option>
                <Select.Option value="supporting">配角</Select.Option>
                <Select.Option value="antagonist">反派</Select.Option>
              </Select>
            </Form.Item>
            <Form.Item label="背景设定" name="background">
              <TextArea rows={3} placeholder="简要描述角色背景和故事环境..." />
            </Form.Item>
          </Form>
        </Space>
      ),
      okText: '生成',
      cancelText: '取消',
      onOk: async () => {
        const values = await generateForm.validateFields();
        void handleGenerate(values);
      },
    });
  };



  const showGenerateOrgModal = () => {
    modal.confirm({
      title: '智能生成组织',
      width: 600,
      centered: true,
      content: (
        <Space direction="vertical" size={12} style={{ width: '100%', marginTop: 12 }}>
          <Card
            size="small"
            variant="borderless"
            style={{ borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-warning-bg) 76%, var(--ant-color-bg-container) 24%)' }}
          >
            <Text type="secondary">
              组织生成适合快速起一个势力草稿。先定义类型和环境，再让模型补足成员结构、口号和组织目的。
            </Text>
          </Card>
          <Form form={generateOrgForm} layout="vertical">
            <Form.Item
              label="组织名称"
              name="name"
            >
              <Input placeholder="如：天剑门、黑龙会（可选，系统会自动生成）" />
            </Form.Item>
            <Form.Item
              label="组织类型"
              name="organization_type"
            >
              <Input placeholder="如：门派、帮派、公司、学院（可选，系统会根据世界观生成）" />
            </Form.Item>
            <Form.Item label="背景设定" name="background">
              <TextArea rows={3} placeholder="简要描述组织的背景和环境..." />
            </Form.Item>
            <Form.Item label="其他要求" name="requirements">
              <TextArea rows={2} placeholder="其他特殊要求..." />
            </Form.Item>
          </Form>
        </Space>
      ),
      okText: '生成',
      cancelText: '取消',
      onOk: async () => {
        const values = await generateOrgForm.validateFields();
        void handleGenerateOrganization(values);
      },
    });
  };



  const { characterList, organizationList } = useMemo(() => {
    const nextCharacterList: Character[] = [];
    const nextOrganizationList: Character[] = [];

    for (const character of characters) {
      if (character.is_organization) {
        nextOrganizationList.push(character);
      } else {
        nextCharacterList.push(character);
      }
    }

    return { characterList: nextCharacterList, organizationList: nextOrganizationList };
  }, [characters]);



  const displayList = useMemo(() => {
    if (activeTab === 'character') return characterList;
    if (activeTab === 'organization') return organizationList;
    return characters;
  }, [activeTab, characterList, organizationList, characters]);



  const selectedCharacterIds = useMemo(() => new Set(selectedCharacters), [selectedCharacters]);



  const [visibleCharacterCount, setVisibleCharacterCount] = useState(INITIAL_CHARACTER_RENDER_COUNT);
  const [visibleOrganizationCount, setVisibleOrganizationCount] = useState(INITIAL_CHARACTER_RENDER_COUNT);



  useEffect(() => {
    const windowWithIdleCallback = window as Window & typeof globalThis & {
      requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;
      cancelIdleCallback?: (handle: number) => void;
    };

    let cancelled = false;
    let characterIdleHandle: number | null = null;
    let organizationIdleHandle: number | null = null;
    let characterTimer: number | null = null;
    let organizationTimer: number | null = null;

    const scheduleVisibleCount = (
      totalCount: number,
      setCount: (value: number) => void,
      target: 'character' | 'organization',
    ) => {
      const initialCount = Math.min(totalCount, INITIAL_CHARACTER_RENDER_COUNT);
      setCount(initialCount);

      if (totalCount <= initialCount) {
        return;
      }

      const flush = () => {
        if (!cancelled) {
          setCount(totalCount);
        }
      };

      if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {
        const handle = windowWithIdleCallback.requestIdleCallback(() => flush(), { timeout: 400 });
        if (target === 'character') {
          characterIdleHandle = handle;
        } else {
          organizationIdleHandle = handle;
        }
        return;
      }

      const timer = window.setTimeout(flush, 80);
      if (target === 'character') {
        characterTimer = timer;
      } else {
        organizationTimer = timer;
      }
    };

    if (activeTab === 'all' || activeTab === 'character') {
      scheduleVisibleCount(characterList.length, setVisibleCharacterCount, 'character');
    } else {
      setVisibleCharacterCount(0);
    }

    if (activeTab === 'all' || activeTab === 'organization') {
      scheduleVisibleCount(organizationList.length, setVisibleOrganizationCount, 'organization');
    } else {
      setVisibleOrganizationCount(0);
    }

    return () => {
      cancelled = true;
      if (characterIdleHandle !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {
        windowWithIdleCallback.cancelIdleCallback(characterIdleHandle);
      }
      if (organizationIdleHandle !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {
        windowWithIdleCallback.cancelIdleCallback(organizationIdleHandle);
      }
      if (characterTimer !== null) {
        window.clearTimeout(characterTimer);
      }
      if (organizationTimer !== null) {
        window.clearTimeout(organizationTimer);
      }
    };
  }, [activeTab, characterList.length, organizationList.length]);



  const visibleCharacterList = useMemo(
    () => characterList.slice(0, visibleCharacterCount),
    [characterList, visibleCharacterCount]
  );



  const visibleOrganizationList = useMemo(
    () => organizationList.slice(0, visibleOrganizationCount),
    [organizationList, visibleOrganizationCount]
  );



  const visibleDisplayList = useMemo(() => {
    if (activeTab === 'character') return visibleCharacterList;
    if (activeTab === 'organization') return visibleOrganizationList;
    return characters;
  }, [activeTab, visibleCharacterList, visibleOrganizationList, characters]);



  const isProgressiveRenderPending = useMemo(() => {
    if (activeTab === 'all') {
      return visibleCharacterList.length < characterList.length || visibleOrganizationList.length < organizationList.length;
    }
    return visibleDisplayList.length < displayList.length;
  }, [
    activeTab,
    characterList.length,
    organizationList.length,
    visibleCharacterList.length,
    visibleOrganizationList.length,
    visibleDisplayList.length,
    displayList.length,
  ]);



  const isMobile = window.innerWidth <= 768;



  const cardColStyle = useMemo(() => ({
    padding: isMobile ? '4px' : '8px',
    contentVisibility: 'auto' as const,
    containIntrinsicSize: isMobile ? '420px' : '360px',
  }), [isMobile]);

  if (!currentProject) return null;

  const editorialInk = token.colorText;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 98%, ${token.colorBgLayout} 2%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorBgLayout} 8%) 100%)`;
  const panelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 96%, ${token.colorPrimary} 4%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorWarning} 8%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorPrimary} 12%, ${token.colorBorder} 88%)`;
  const modalSurfaceStyles = {
    header: { padding: '22px 24px 0', borderBottom: 'none' },
    body: { padding: '0 24px 24px' },
    footer: { padding: '0 24px 24px', borderTop: 'none' },
  } as const;
  const actionButtonStyle = {
    borderRadius: 999,
    background: 'color-mix(in srgb, var(--ant-color-bg-container) 14%, transparent)',
    border: '1px solid color-mix(in srgb, var(--ant-color-bg-container) 20%, transparent)',
    color: editorialInk,
    boxShadow: `0 10px 18px color-mix(in srgb, ${token.colorText} 18%, transparent)`,
    backdropFilter: 'blur(8px)',
  } as const;
  const summaryItems = [
    { label: '全部条目', value: `${characters.length}` },
    { label: '角色卡', value: `${characterList.length}` },
    { label: '组织卡', value: `${organizationList.length}` },
  ];
  const renderSelectableGrid = (items: Character[]) => (
    <Row gutter={isMobile ? [8, 8] : [8, 8]}>
      {items.map((item) => (
        <SelectableCharacterCard
          key={item.id}
          item={item}
          selected={selectedCharacterIds.has(item.id)}
          cardColStyle={cardColStyle}
          onToggle={toggleSelectCharacter}
          onEdit={handleEditCharacter}
          onDelete={handleDeleteCharacterWrapper}
          onExport={handleExportSingle}
        />
      ))}
    </Row>
  );



  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, paddingBottom: 24 }}>
      {contextHolder}
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: isMobile ? 22 : 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
          flexShrink: 0,
        }}
        styles={{ body: { padding: isMobile ? 18 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -40, width: 180, height: 180, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: '24%', width: 110, height: 110, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Character Workspace
              </Text>
              <Title level={isMobile ? 3 : 2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                <TeamOutlined style={{ marginRight: 8, color: 'rgba(255,255,255,0.9)' }} />
                角色与组织管理
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: isMobile ? 13 : 15, lineHeight: 1.8 }}>
                管理项目人物、组织、导入导出与生成入口。
              </Paragraph>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              {summaryItems.map((item) => (
                <div
                  key={item.label}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 12,
                    borderRadius: 18,
                    padding: '12px 14px',
                    background: 'rgba(255,255,255,0.08)',
                    border: '1px solid rgba(255,255,255,0.1)',
                    backdropFilter: 'blur(10px)',
                  }}
                >
                  <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12 }}>{item.label}</Text>
                  <Text style={{ color: editorialInk, fontWeight: 600 }}>{item.value}</Text>
                </div>
              ))}
            </Space>
          </Col>
        </Row>
        <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => {
              setCreateType('character');
              ensureCareersLoaded();
              setIsCreateModalOpen(true);
            }}
            size={isMobile ? 'small' : 'middle'}
            style={{ borderRadius: 999, paddingInline: 16 }}
          >
            创建角色
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => {
              setCreateType('organization');
              setIsCreateModalOpen(true);
            }}
            size={isMobile ? 'small' : 'middle'}
            style={{ borderRadius: 999, paddingInline: 16 }}
          >
            创建组织
          </Button>
          <Button
            type="dashed"
            icon={<ThunderboltOutlined />}
            onClick={showGenerateModal}
            loading={isGenerating}
            size={isMobile ? 'small' : 'middle'}
            style={actionButtonStyle}
          >
            智能生成角色
          </Button>
          <Button
            type="dashed"
            icon={<ThunderboltOutlined />}
            onClick={showGenerateOrgModal}
            loading={isGenerating}
            size={isMobile ? 'small' : 'middle'}
            style={actionButtonStyle}
          >
            智能生成组织
          </Button>
          <Button
            icon={<ImportOutlined />}
            onClick={() => setIsImportModalOpen(true)}
            size={isMobile ? 'small' : 'middle'}
            style={actionButtonStyle}
          >
            导入
          </Button>
          {selectedCharacters.length > 0 && (
            <Button
              icon={<ExportOutlined />}
              onClick={handleExportSelected}
              size={isMobile ? 'small' : 'middle'}
              style={actionButtonStyle}
            >
              批量导出 ({selectedCharacters.length})
            </Button>
          )}
        </Space>
      </Card>

      {characters.length > 0 && (
        <Card
          variant="borderless"
          style={{
            background: quietPanelBackground,
            borderRadius: isMobile ? 18 : 22,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: isMobile ? 12 : 16 } }}
        >
          <Tabs
            activeKey={activeTab}
            onChange={(key) => setActiveTab(key as 'all' | 'character' | 'organization')}
            items={[
              {
                key: 'all',
                label: `全部 (${characters.length})`,
              },
              {
                key: 'character',
                label: (
                  <span>
                    <UserOutlined /> 角色 ({characterList.length})
                  </span>
                ),
              },
              {
                key: 'organization',
                label: (
                  <span>
                    <TeamOutlined /> 组织 ({organizationList.length})
                  </span>
                ),
              },
            ]}
          />
        </Card>
      )}



      {/* 批量选择工具栏 */}
      {characters.length > 0 && (
        <Card
          variant="borderless"
          style={{
            background: panelBackground,
            borderRadius: isMobile ? 18 : 22,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: isMobile ? 12 : 14 } }}
        >
          <Space>
            <Checkbox
              checked={selectedCharacters.length === displayList.length && displayList.length > 0}
              indeterminate={selectedCharacters.length > 0 && selectedCharacters.length < displayList.length}
              onChange={toggleSelectAll}
            >
              {selectedCharacters.length > 0 ? `已选 ${selectedCharacters.length} 个` : '全选'}
            </Checkbox>
            {selectedCharacters.length > 0 && (
              <Button
                type="link"
                size="small"
                onClick={() => setSelectedCharacters([])}
              >
                取消选择
              </Button>
            )}
          </Space>
        </Card>
      )}



      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        {characters.length === 0 ? (
          <Card
            variant="borderless"
            style={{
              background: quietPanelBackground,
              borderRadius: isMobile ? 18 : 22,
              border: panelBorder,
              boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
            }}
          >
            <Empty description="还没有角色或组织，开始创建吧！" style={{ padding: '64px 0' }}>
              <Paragraph type="secondary" style={{ maxWidth: 520, margin: '8px auto 20px', lineHeight: 1.8 }}>
                角色与组织会成为章节推进、关系图谱和世界细节的主要承载体。你可以先手动建立核心人物，也可以直接用智能生成起一版草稿。
              </Paragraph>
              <Space wrap>
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={() => {
                    setCreateType('character');
                    ensureCareersLoaded();
                    setIsCreateModalOpen(true);
                  }}
                >
                  创建角色
                </Button>
                <Button
                  icon={<ThunderboltOutlined />}
                  onClick={showGenerateModal}
                  loading={isGenerating}
                >
                  智能生成角色
                </Button>
              </Space>
            </Empty>
          </Card>
        ) : (
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            {activeTab === 'all' && (
              <>
                <Card
                  variant="borderless"
                  style={{
                    background: quietPanelBackground,
                    borderRadius: isMobile ? 18 : 22,
                    border: panelBorder,
                    boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                  }}
                  styles={{ body: { padding: isMobile ? 14 : 18 } }}
                >
                  <Divider orientation="left" style={{ marginTop: 0 }}>
                    <Title level={5} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
                      <UserOutlined style={{ marginRight: 8 }} />
                      角色 ({characterList.length})
                    </Title>
                  </Divider>
                  {characterList.length > 0 ? renderSelectableGrid(visibleCharacterList) : <Empty description="暂无角色" />}
                </Card>

                <Card
                  variant="borderless"
                  style={{
                    background: quietPanelBackground,
                    borderRadius: isMobile ? 18 : 22,
                    border: panelBorder,
                    boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                  }}
                  styles={{ body: { padding: isMobile ? 14 : 18 } }}
                >
                  <Divider orientation="left" style={{ marginTop: 0 }}>
                    <Title level={5} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
                      <TeamOutlined style={{ marginRight: 8 }} />
                      组织 ({organizationList.length})
                    </Title>
                  </Divider>
                  {organizationList.length > 0 ? renderSelectableGrid(visibleOrganizationList) : <Empty description="暂无组织" />}
                </Card>
              </>
            )}

            {activeTab === 'character' && (
              <Card
                variant="borderless"
                style={{
                  background: quietPanelBackground,
                  borderRadius: isMobile ? 18 : 22,
                  border: panelBorder,
                  boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                }}
                styles={{ body: { padding: isMobile ? 14 : 18 } }}
              >
                <Divider orientation="left" style={{ marginTop: 0 }}>
                  <Title level={5} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
                    <UserOutlined style={{ marginRight: 8 }} />
                    角色 ({characterList.length})
                  </Title>
                </Divider>
                {characterList.length > 0 ? renderSelectableGrid(visibleCharacterList) : <Empty description="暂无角色" />}
              </Card>
            )}

            {activeTab === 'organization' && (
              <Card
                variant="borderless"
                style={{
                  background: quietPanelBackground,
                  borderRadius: isMobile ? 18 : 22,
                  border: panelBorder,
                  boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                }}
                styles={{ body: { padding: isMobile ? 14 : 18 } }}
              >
                <Divider orientation="left" style={{ marginTop: 0 }}>
                  <Title level={5} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
                    <TeamOutlined style={{ marginRight: 8 }} />
                    组织 ({organizationList.length})
                  </Title>
                </Divider>
                {organizationList.length > 0 ? renderSelectableGrid(visibleOrganizationList) : <Empty description="暂无组织" />}
              </Card>
            )}

            {isProgressiveRenderPending && (
              <Card
                variant="borderless"
                style={{
                  background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimaryBg} 90%, transparent) 0%, ${quietPanelBackground} 100%)`,
                  borderRadius: isMobile ? 16 : 18,
                  border: panelBorder,
                  boxShadow: `0 14px 28px color-mix(in srgb, ${token.colorText} 6%, transparent)`,
                }}
                styles={{ body: { padding: isMobile ? '12px 14px' : '14px 16px' } }}
              >
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                  Progressive Render
                </Text>
                <Text strong style={{ display: 'block', marginTop: 6 }}>
                  正在继续补齐剩余角色与组织卡片
                </Text>
                <Text type="secondary" style={{ display: 'block', marginTop: 6, lineHeight: 1.7 }}>
                  当前页面已先展示首批内容，后续卡片会继续渲染。
                </Text>
              </Card>
            )}

            {displayList.length === 0 && activeTab !== 'all' && (
              <Card
                variant="borderless"
                style={{
                  background: quietPanelBackground,
                  borderRadius: isMobile ? 18 : 22,
                  border: panelBorder,
                }}
              >
                <Empty
                  description={activeTab === 'character' ? '暂无角色' : '暂无组织'}
                  style={{ padding: '40px 0' }}
                />
              </Card>
            )}
          </Space>
        )}
      </div>



      {isEditModalOpen && editingCharacter ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Character Workspace"
              title="正在展开角色 / 组织编辑面板"
              message="系统正在恢复角色档案字段、职业信息与提交入口，原有编辑表单和保存逻辑保持不变。"
              tags={[
                { label: '角色 / 组织编辑', color: 'blue' },
                { label: '档案面板恢复中', color: 'processing' },
                { label: '保存逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyCharacterFormModal
            open={isEditModalOpen}
            title={editingCharacter.is_organization ? '编辑组织' : '编辑角色'}
            mode="edit"
            entityType={editingCharacter.is_organization ? 'organization' : 'character'}
            form={editForm}
            isMobile={isMobile}
            record={editingCharacter}
            mainCareers={mainCareers}
            subCareers={subCareers}
            submitText="保存"
            onCancel={closeEditModal}
            onFinish={handleUpdateCharacter}
          />
        </Suspense>
      ) : null}

      {/* 新建角色/组织 */}
      {isCreateModalOpen ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Character Creation"
              title="正在展开角色 / 组织创建面板"
              message="系统正在恢复新建档案字段、职业选项与提交入口，原有创建表单和保存逻辑保持不变。"
              tags={[
                { label: '角色 / 组织创建', color: 'purple' },
                { label: '新建面板恢复中', color: 'processing' },
                { label: '创建逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyCharacterFormModal
            open={isCreateModalOpen}
            title={createType === 'character' ? '新建角色' : '新建组织'}
            mode="create"
            entityType={createType === 'character' ? 'character' : 'organization'}
            form={createForm}
            isMobile={isMobile}
            mainCareers={mainCareers}
            subCareers={subCareers}
            submitText="创建"
            onCancel={closeCreateModal}
            onFinish={handleCreateCharacter}
          />
        </Suspense>
      ) : null}

      {isImportModalOpen ? (
      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Import Desk
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              导入角色/组织
            </Title>
            <Text type="secondary">
              适合把已经整理过的 JSON 档案带回项目。导入前先确认命名与职业字段是否符合当前世界设定。
            </Text>
          </Space>
        )}
        open={isImportModalOpen}
        onCancel={() => setIsImportModalOpen(false)}
        footer={null}
        width={500}
        centered
        styles={modalSurfaceStyles}
      >
        <Card
          size="small"
          variant="borderless"
          style={{ marginBottom: 16, borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-success-bg) 74%, var(--ant-color-bg-container) 26%)' }}
        >
          <Text type="secondary">
            导入更适合已经清洗过的角色包。重复名称会被跳过，缺失的职业信息会按现有规则忽略。
          </Text>
        </Card>
        <div style={{ textAlign: 'center', padding: '40px 20px' }}>
          <DownloadOutlined style={{ fontSize: 48, color: '#1890ff', marginBottom: 16 }} />
          <p style={{ fontSize: 16, marginBottom: 24 }}>
            选择之前导出的角色/组织JSON文件进行导入
          </p>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json"
            style={{ display: 'none' }}
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) {
                handleFileSelect(file);
                e.target.value = ''; // 清空input，允许重复选择同一文件
              }
            }}
          />
          <Button
            type="primary"
            size="large"
            icon={<ImportOutlined />}
            onClick={() => fileInputRef.current?.click()}
          >
            选择文件
          </Button>
          <Divider />
          <div style={{ textAlign: 'left', fontSize: 12, color: '#666' }}>
            <p style={{ marginBottom: 8 }}><strong>说明：</strong></p>
            <ul style={{ marginLeft: 20 }}>
              <li>支持导入.json格式的角色/组织文件</li>
              <li>重复名称的角色/组织将被跳过</li>
              <li>职业信息如不存在将被忽略</li>
            </ul>
          </div>
        </div>
      </Modal>
      ) : null}



      {/* SSE进度显示 */}
      {isGenerating ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              variant="fullscreen"
              eyebrow="Archive Generation"
              title="正在接管角色档案生成覆盖层"
              message="系统正在恢复角色 / 组织生成进度与取消入口，原有生成状态、轮询提示和中断逻辑保持不变。"
              tags={[
                { label: '档案生成', color: 'gold' },
                { label: '覆盖层恢复中', color: 'processing' },
                { label: '状态逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazySSELoadingOverlay
            loading={isGenerating}
            progress={progress}
            message={progressMessage}
            blocking={false}
            onCancel={handleCancelGeneratingTask}
            cancelButtonLoading={isCancellingTask}
            cancelButtonDisabled={isCancellingTask || !currentTaskIdRef.current}
            modelOutput={modelOutput}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
