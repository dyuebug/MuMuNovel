import { Suspense, lazy, memo, useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { Button, Modal, Form, Input, Select, message, Row, Col, Empty, Tabs, Divider, Typography, Space, Checkbox, theme } from 'antd';
import { ThunderboltOutlined, UserOutlined, TeamOutlined, PlusOutlined, ExportOutlined, ImportOutlined, DownloadOutlined } from '@ant-design/icons';
import { useStore } from '../store';
import { useCharacterSync } from '../store/hooks';
import { charactersPageGridConfig } from '../components/CardStyles';
import { CharacterCard } from '../components/CharacterCard';
import type { CSSProperties } from 'react';
import type { Character, ApiError } from '../types';
import { backgroundTaskApi, characterApi } from '../services/api';
import { getCachedProjectCareers, loadProjectCareers } from '../services/projectCareers';



const { Title } = Typography;
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

export default function Characters() {
  const { token } = theme.useToken();
  const currentProject = useStore((state) => state.currentProject);
  const storeCharacters = useStore((state) => state.characters);
  const [isGenerating, setIsGenerating] = useState(false);
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
  const taskPollTimerRef = useRef<number | null>(null);
  const currentTaskIdRef = useRef<string | null>(null);



  const {
    refreshCharacters,
    createCharacter,
    updateCharacter,
    deleteCharacter
  } = useCharacterSync();



  const characters = useMemo(() => {
    if (!currentProject?.id) {
      return [];
    }



    return storeCharacters.filter((character) => character.project_id === currentProject.id);
  }, [currentProject?.id, storeCharacters]);



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



  useEffect(() => {
    return () => {
      if (taskPollTimerRef.current) {
        window.clearInterval(taskPollTimerRef.current);
        taskPollTimerRef.current = null;
      }
      currentTaskIdRef.current = null;
    };
  }, []);



  const handleDeleteCharacter = useCallback(async (id: string) => {
    try {
      await deleteCharacter(id);
      message.success('\u5220\u9664\u6210\u529f');
    } catch {
      message.error('\u5220\u9664\u5931\u8d25');
    }
  }, [deleteCharacter]);



  const stopTaskPolling = () => {
    if (taskPollTimerRef.current) {
      window.clearInterval(taskPollTimerRef.current);
      taskPollTimerRef.current = null;
    }
  };



  const startTaskPolling = (taskId: string, successMessage: string) => {
    stopTaskPolling();
    currentTaskIdRef.current = taskId;
    setIsCancellingTask(false);



    const poll = async () => {
      try {
        const task = await backgroundTaskApi.getTaskStatus(taskId);
        setProgress(task.progress || 0);
        setProgressMessage(task.message || '');



        if (task.status === 'completed') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsGenerating(false);
          message.success(successMessage);
          await refreshCharacters();
          return;
        }



        if (task.status === 'failed') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsGenerating(false);
          message.error(task.error || task.message || '生成失败');
          return;
        }



        if (task.status === 'cancelled') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsGenerating(false);
          message.info(task.message || '后台任务已取消');
        }
      } catch (error) {
        console.error('轮询后台任务状态失败:', error);
      }
    };



    void poll();
    taskPollTimerRef.current = window.setInterval(() => {
      void poll();
    }, 1500);
  };



  const handleGenerateBackground = async (values: { name?: string; role_type: string; background?: string }) => {
    if (isGenerating) {
      message.info('已有后台生成任务在运行，请稍后查看结果');
      return;
    }



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
      startTaskPolling(task.task_id, '智能生成角色成功');
    } catch (error: unknown) {
      stopTaskPolling();
      currentTaskIdRef.current = null;
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
    if (isGenerating) {
      message.info('已有后台生成任务在运行，请稍后查看结果');
      return;
    }



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
      startTaskPolling(task.task_id, '智能生成组织成功');
    } catch (error: unknown) {
      stopTaskPolling();
      currentTaskIdRef.current = null;
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
      message.success('\u5bfc\u51fa\u6210\u529f');
    } catch (error) {
      message.error('\u5bfc\u51fa\u5931\u8d25');
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
        <Form form={generateForm} layout="vertical" style={{ marginTop: 16 }}>
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
        <Form form={generateOrgForm} layout="vertical" style={{ marginTop: 16 }}>
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



  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {contextHolder}
      <div style={{
        position: 'sticky',
        top: 0,
        zIndex: 10,
        backgroundColor: 'var(--color-bg-container)',
        padding: isMobile ? '12px 0' : '16px 0',
        marginBottom: isMobile ? 12 : 16,
        borderBottom: '1px solid var(--color-border-secondary)',
        display: 'flex',
        flexDirection: isMobile ? 'column' : 'row',
        gap: isMobile ? 12 : 0,
        justifyContent: 'space-between',
        alignItems: isMobile ? 'stretch' : 'center'
      }}>
        <h2 style={{ margin: 0, fontSize: isMobile ? 18 : 24 }}>
          <TeamOutlined style={{ marginRight: 8 }} />
          角色与组织管理
        </h2>
        <Space wrap>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => {
              setCreateType('character');
              ensureCareersLoaded();
              setIsCreateModalOpen(true);
            }}
            size={isMobile ? 'small' : 'middle'}
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
          >
            创建组织
          </Button>
          <Button
            type="dashed"
            icon={<ThunderboltOutlined />}
            onClick={showGenerateModal}
            loading={isGenerating}
            size={isMobile ? 'small' : 'middle'}
          >
            智能生成角色
          </Button>
          <Button
            type="dashed"
            icon={<ThunderboltOutlined />}
            onClick={showGenerateOrgModal}
            loading={isGenerating}
            size={isMobile ? 'small' : 'middle'}
          >
            智能生成组织
          </Button>
          <Button
            icon={<ImportOutlined />}
            onClick={() => setIsImportModalOpen(true)}
            size={isMobile ? 'small' : 'middle'}
          >
            导入
          </Button>
          {selectedCharacters.length > 0 && (
            <Button
              icon={<ExportOutlined />}
              onClick={handleExportSelected}
              size={isMobile ? 'small' : 'middle'}
            >
              批量导出 ({selectedCharacters.length})
            </Button>
          )}
        </Space>
      </div>



      {characters.length > 0 && (
        <div style={{
          position: 'sticky',
          top: isMobile ? 60 : 72,
          zIndex: 9,
          backgroundColor: 'var(--color-bg-container)',
          paddingBottom: 8,
          borderBottom: '1px solid var(--color-border-secondary)',
        }}>
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
        </div>
      )}



      {/* 批量选择工具栏 */}
      {characters.length > 0 && (
        <div style={{
          position: 'sticky',
          top: isMobile ? 120 : 132,
          zIndex: 8,
          backgroundColor: 'var(--color-bg-container)',
          paddingBottom: 8,
          paddingTop: 8,
          marginTop: 8,
          borderBottom: selectedCharacters.length > 0 ? '1px solid var(--color-border-secondary)' : 'none',
        }}>
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
        </div>
      )}



      <div style={{ flex: 1, overflowY: 'auto' }}>
        {characters.length === 0 ? (
          <Empty description="还没有角色或组织，开始创建吧！" />
        ) : (
          <>
            <Row gutter={isMobile ? [8, 8] : charactersPageGridConfig.gutter}>
              {activeTab === 'all' && (
                <>
                  {characterList.length > 0 && (
                    <>
                      <Col span={24}>
                        <Divider orientation="left">
                          <Title level={5} style={{ margin: 0 }}>
                            <UserOutlined style={{ marginRight: 8 }} />
                            角色 ({characterList.length})
                          </Title>
                        </Divider>
                      </Col>
                      {visibleCharacterList.map((character) => (
                        <SelectableCharacterCard
                          key={character.id}
                          item={character}
                          selected={selectedCharacterIds.has(character.id)}
                          cardColStyle={cardColStyle}
                          onToggle={toggleSelectCharacter}
                          onEdit={handleEditCharacter}
                          onDelete={handleDeleteCharacterWrapper}
                          onExport={handleExportSingle}
                        />
                      ))}
                    </>
                  )}



                  {organizationList.length > 0 && (
                    <>
                      <Col span={24}>
                        <Divider orientation="left">
                          <Title level={5} style={{ margin: 0 }}>
                            <TeamOutlined style={{ marginRight: 8 }} />
                            组织 ({organizationList.length})
                          </Title>
                        </Divider>
                      </Col>
                      {visibleOrganizationList.map((org) => (
                        <SelectableCharacterCard
                          key={org.id}
                          item={org}
                          selected={selectedCharacterIds.has(org.id)}
                          cardColStyle={cardColStyle}
                          onToggle={toggleSelectCharacter}
                          onEdit={handleEditCharacter}
                          onDelete={handleDeleteCharacterWrapper}
                          onExport={handleExportSingle}
                        />
                      ))}
                    </>
                  )}
                </>
              )}



              {activeTab === 'character' && visibleCharacterList.map((character) => (
                <SelectableCharacterCard
                  key={character.id}
                  item={character}
                  selected={selectedCharacterIds.has(character.id)}
                  cardColStyle={cardColStyle}
                  onToggle={toggleSelectCharacter}
                  onEdit={handleEditCharacter}
                  onDelete={handleDeleteCharacterWrapper}
                  onExport={handleExportSingle}
                />
              ))}



              {activeTab === 'organization' && visibleOrganizationList.map((org) => (
                <SelectableCharacterCard
                  key={org.id}
                  item={org}
                  selected={selectedCharacterIds.has(org.id)}
                  cardColStyle={cardColStyle}
                  onToggle={toggleSelectCharacter}
                  onEdit={handleEditCharacter}
                  onDelete={handleDeleteCharacterWrapper}
                  onExport={handleExportSingle}
                />
              ))}
            </Row>



            {isProgressiveRenderPending && (
              <div style={{ textAlign: 'center', padding: '12px 0', color: 'var(--color-text-tertiary)' }}>
                {'\u6b63\u5728\u52a0\u8f7d\u5176\u4f59\u89d2\u8272\u4e0e\u7ec4\u7ec7...'}
              </div>
            )}



            {displayList.length === 0 && (
              <Empty
                description={
                  activeTab === 'character'
                    ? '暂无角色'
                    : activeTab === 'organization'
                      ? '暂无组织'
                      : '暂无数据'
                }
              />
            )}
          </>
        )}
      </div>



      {isEditModalOpen && editingCharacter ? (
        <Suspense fallback={null}>
          <LazyCharacterFormModal
            open={isEditModalOpen}
            title={editingCharacter.is_organization ? '????' : '????'}
            mode="edit"
            entityType={editingCharacter.is_organization ? 'organization' : 'character'}
            form={editForm}
            isMobile={isMobile}
            record={editingCharacter}
            mainCareers={mainCareers}
            subCareers={subCareers}
            submitText="??"
            onCancel={closeEditModal}
            onFinish={handleUpdateCharacter}
          />
        </Suspense>
      ) : null}

      {/* ??????/????? */}
      {isCreateModalOpen ? (
        <Suspense fallback={null}>
          <LazyCharacterFormModal
            open={isCreateModalOpen}
            title={createType === 'character' ? '????' : '????'}
            mode="create"
            entityType={createType === 'character' ? 'character' : 'organization'}
            form={createForm}
            isMobile={isMobile}
            mainCareers={mainCareers}
            subCareers={subCareers}
            submitText="??"
            onCancel={closeCreateModal}
            onFinish={handleCreateCharacter}
          />
        </Suspense>
      ) : null}

      {isImportModalOpen ? (
      <Modal
        title="导入角色/组织"
        open={isImportModalOpen}
        onCancel={() => setIsImportModalOpen(false)}
        footer={null}
        width={500}
        centered
      >
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
        <Suspense fallback={null}>
          <LazySSELoadingOverlay
            loading={isGenerating}
            progress={progress}
            message={progressMessage}
            blocking={false}
            onCancel={handleCancelGeneratingTask}
            cancelButtonLoading={isCancellingTask}
            cancelButtonDisabled={isCancellingTask || !currentTaskIdRef.current}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
