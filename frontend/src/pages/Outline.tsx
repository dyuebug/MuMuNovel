import { Suspense, lazy, useState, useEffect, useMemo, useRef, useCallback } from 'react';


import { Button, List, Modal, Form, Input, message, Empty, Space, Popconfirm, Card, Select, Radio, Tag, InputNumber } from 'antd';


import { EditOutlined, DeleteOutlined, ThunderboltOutlined, BranchesOutlined, AppstoreAddOutlined, CheckCircleOutlined, ExclamationCircleOutlined, PlusOutlined, FileTextOutlined } from '@ant-design/icons';


import { useStore } from '../store';


import { useCharacterSync, useOutlineSync } from '../store/hooks';


import { backgroundTaskApi, outlineApi, chapterApi, projectApi } from '../services/api';
import { hasUsableApiCredentials } from '../utils/apiKey';


import type { OutlineExpansionResponse, BatchOutlineExpansionResponse, ChapterPlanItem, ApiError, Character, CreativeMode, PlotStage, QualityPreset, StoryFocus } from '../types';


// 大纲生成请求数据类型


interface OutlineGenerateRequestData {


  project_id: string;


  genre: string;


  theme: string;


  chapter_count: number;


  narrative_perspective: string;


  target_words: number;


  requirements?: string;


  mode: 'auto' | 'new' | 'continue';


  story_direction?: string;


  plot_stage: PlotStage;


  model?: string;


  provider?: string;


  creative_mode?: CreativeMode;


  story_focus?: StoryFocus;


  story_creation_brief?: string;


  quality_preset?: QualityPreset;


  quality_notes?: string;


}




// 跳过的大纲信息类型


// 场景类型


interface SceneInfo {


  location: string;


  characters: string[];


  purpose: string;


}


// 角色/组织条目类型（新格式）


interface CharacterEntry {


  name: string;


  type: 'character' | 'organization';


}


/**


 * 解析 characters 字段，兼容新旧格式


 * 旧格式: string[] -> 全部当作 character


 * 新格式: {name: string, type: "character"|"organization"}[]


 */


function parseCharacterEntries(characters: unknown): CharacterEntry[] {


  if (!Array.isArray(characters) || characters.length === 0) return [];


  


  return characters.map((entry) => {


    if (typeof entry === 'string') {


      // 旧格式：纯字符串，默认为 character


      return { name: entry, type: 'character' as const };


    }


    if (typeof entry === 'object' && entry !== null && 'name' in entry) {


      // 新格式：带类型标识的对象


      return {


        name: (entry as { name: string }).name,


        type: ((entry as { type?: string }).type === 'organization' ? 'organization' : 'character') as 'character' | 'organization'


      };


    }


    return null;


  }).filter((e): e is CharacterEntry => e !== null);


}


/** 从 entries 中提取角色名称列表 */


function getCharacterNames(entries: CharacterEntry[]): string[] {


  return entries.filter(e => e.type === 'character').map(e => e.name);


}


/** 从 entries 中提取组织名称列表 */


function getOrganizationNames(entries: CharacterEntry[]): string[] {


  return entries.filter(e => e.type === 'organization').map(e => e.name);


}


interface OutlineStructureData {


  key_events?: string[];


  key_points?: string[];


  characters_involved?: string[];


  characters?: unknown[];


  scenes?: string[] | SceneInfo[];


  emotion?: string;


  goal?: string;


}


interface ParsedOutlineViewData {


  structureData: OutlineStructureData;


  characterNames: string[];


  organizationNames: string[];


}


function parseOutlineStructure(structure?: string): OutlineStructureData {


  if (!structure) return {};


  try {


    return JSON.parse(structure) as OutlineStructureData;


  } catch (error) {


    console.error('parse outline structure failed:', error);


    return {};


  }


}


function getPreviewText(text: string | undefined, maxLength: number): string {


  if (!text) return '';


  const normalizedText = text.replace(/\s+/g, ' ').trim();


  if (normalizedText.length <= maxLength) {


    return normalizedText;


  }


  return `${normalizedText.slice(0, maxLength).trimEnd()}...`;


}


const { TextArea } = Input;


const outlineChapterStatusCache = new Map<string, boolean>();


const outlineChapterStatusPromises = new Map<string, Promise<boolean>>();


const INITIAL_OUTLINE_RENDER_COUNT = 6;


const OUTLINE_RENDER_BATCH_SIZE = 8;


const OUTLINE_RENDER_BATCH_DELAY_MS = 120;


const LazySSEProgressModal = lazy(async () => {


  const module = await import('../components/SSEProgressModal');


  return { default: module.SSEProgressModal };


});


const LazyOutlineBatchPreviewModal = lazy(() => import('../components/OutlineBatchPreviewModal'));
const LazyOutlineExpansionPreviewContent = lazy(() => import('../components/OutlineExpansionPreviewContent'));
const LazyOutlineExistingExpansionContent = lazy(() => import('../components/OutlineExistingExpansionContent'));
const LazyOutlineBatchExpandConfigForm = lazy(() => import('../components/OutlineBatchExpandConfigForm'));
const LazyOutlineGenerateModalContent = lazy(() => import('../components/OutlineGenerateModalContent'));

const outlineLazyFallback = (
  <div style={{ padding: '16px 0', textAlign: 'center' }}>
    {"加载中..."}
  </div>
);

export default function Outline() {


  const currentProject = useStore((state) => state.currentProject);


  const outlines = useStore((state) => state.outlines);


  const storeCharacters = useStore((state) => state.characters);


  const setCurrentProject = useStore((state) => state.setCurrentProject);


  const [isGenerating, setIsGenerating] = useState(false);


  const [editForm] = Form.useForm();


  const [generateForm] = Form.useForm();


  const [expansionForm] = Form.useForm();


  const [modalApi, contextHolder] = Modal.useModal();


  const [batchExpansionForm] = Form.useForm();


  const [manualCreateForm] = Form.useForm();

  const projectDefaultCreativeMode = currentProject?.default_creative_mode as CreativeMode | undefined;
  const projectDefaultStoryFocus = currentProject?.default_story_focus as StoryFocus | undefined;
  const projectDefaultPlotStage = currentProject?.default_plot_stage as PlotStage | undefined;
  const projectDefaultStoryCreationBrief = currentProject?.default_story_creation_brief || undefined;
  const projectDefaultQualityPreset = currentProject?.default_quality_preset as QualityPreset | undefined;
  const projectDefaultQualityNotes = currentProject?.default_quality_notes || undefined;



  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);


  const [isExpanding, setIsExpanding] = useState(false);


  // ✅ 新增：记录每个大纲的展开状态


  const [outlineExpandStatus, setOutlineExpandStatus] = useState<Record<string, boolean>>({});


  


  // ✅ 新增：记录场景区域的展开/折叠状态


  const [scenesExpandStatus, setScenesExpandStatus] = useState<Record<string, boolean>>({});


  // 缓存批量展开的规划数据，避免重复AI调用


  const [cachedBatchExpansionResponse, setCachedBatchExpansionResponse] = useState<BatchOutlineExpansionResponse | null>(null);


  // 批量展开预览的状态


  const [batchPreviewVisible, setBatchPreviewVisible] = useState(false);


  const [batchPreviewData, setBatchPreviewData] = useState<BatchOutlineExpansionResponse | null>(null);


  const [visibleOutlineCount, setVisibleOutlineCount] = useState(INITIAL_OUTLINE_RENDER_COUNT);


  // SSE进度状态


  const [sseProgress, setSSEProgress] = useState(0);


  const [sseMessage, setSSEMessage] = useState('');


  const [sseModalVisible, setSSEModalVisible] = useState(false);


  const generateTaskPollTimerRef = useRef<number | null>(null);


  const expandTaskPollTimerRef = useRef<number | null>(null);


  const generateTaskIdRef = useRef<string | null>(null);


  const expandTaskIdRef = useRef<string | null>(null);


  useEffect(() => {


    const handleResize = () => {


      setIsMobile(window.innerWidth <= 768);


    };


    window.addEventListener('resize', handleResize);


    return () => window.removeEventListener('resize', handleResize);


  }, []);


  const stopGenerateTaskPolling = () => {


    if (generateTaskPollTimerRef.current) {


      window.clearInterval(generateTaskPollTimerRef.current);


      generateTaskPollTimerRef.current = null;


    }


  };


  const stopExpandTaskPolling = () => {


    if (expandTaskPollTimerRef.current) {


      window.clearInterval(expandTaskPollTimerRef.current);


      expandTaskPollTimerRef.current = null;


    }


  };


  useEffect(() => {


    return () => {


      stopGenerateTaskPolling();


      stopExpandTaskPolling();


    };


  }, []);


  const startGenerateTaskPolling = (taskId: string) => {


    stopGenerateTaskPolling();


    generateTaskIdRef.current = taskId;


    const poll = async () => {


      try {


        const task = await backgroundTaskApi.getTaskStatus(taskId);


        setSSEProgress(task.progress || 0);


        setSSEMessage(task.message || '');


        if (task.status === 'completed') {


          stopGenerateTaskPolling();


          generateTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsGenerating(false);


          message.success('大纲生成完成！');


          void refreshOutlines();


          return;


        }


        if (task.status === 'failed') {


          stopGenerateTaskPolling();


          generateTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsGenerating(false);


          message.error(task.error || task.message || '生成失败');


          return;


        }


        if (task.status === 'cancelled') {


          stopGenerateTaskPolling();


          generateTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsGenerating(false);


          message.info(task.message || '任务已取消');


        }


      } catch (error) {


        console.error('轮询大纲生成任务失败:', error);


      }


    };


    void poll();


    generateTaskPollTimerRef.current = window.setInterval(() => {


      void poll();


    }, 1500);


  };


  const startExpandTaskPolling = (


    taskId: string,


    onCompleted: (result: Record<string, unknown> | null) => void


  ) => {


    stopExpandTaskPolling();


    expandTaskIdRef.current = taskId;


    const poll = async () => {


      try {


        const task = await backgroundTaskApi.getTaskStatus(taskId);


        setSSEProgress(task.progress || 0);


        setSSEMessage(task.message || '');


        if (task.status === 'completed') {


          stopExpandTaskPolling();


          expandTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsExpanding(false);


          onCompleted((task.result as Record<string, unknown> | null) || null);


          return;


        }


        if (task.status === 'failed') {


          stopExpandTaskPolling();


          expandTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsExpanding(false);


          message.error(task.error || task.message || '任务执行失败');


          return;


        }


        if (task.status === 'cancelled') {


          stopExpandTaskPolling();


          expandTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsExpanding(false);


          message.info(task.message || '任务已取消');


        }


      } catch (error) {


        console.error('轮询大纲展开任务失败:', error);


      }


    };


    void poll();


    expandTaskPollTimerRef.current = window.setInterval(() => {


      void poll();


    }, 1500);


  };


  const handleCancelGenerateTask = async () => {


    const taskId = generateTaskIdRef.current;


    if (!taskId) return;


    try {


      await backgroundTaskApi.cancelTask(taskId);


      message.info('已取消大纲生成任务');


    } catch (error) {


      console.error('取消大纲生成任务失败:', error);


      message.error('取消任务失败');


    } finally {


      stopGenerateTaskPolling();


      generateTaskIdRef.current = null;


      setSSEModalVisible(false);


      setIsGenerating(false);


    }


  };


  const handleCancelExpandTask = async () => {


    const taskId = expandTaskIdRef.current;


    if (!taskId) return;


    try {


      await backgroundTaskApi.cancelTask(taskId);


      message.info('已取消大纲展开任务');


    } catch (error) {


      console.error('取消大纲展开任务失败:', error);


      message.error('取消任务失败');


    } finally {


      stopExpandTaskPolling();


      expandTaskIdRef.current = null;


      setSSEModalVisible(false);


      setIsExpanding(false);


    }


  };


  // 使用同步 hooks


  const {


    refreshOutlines,


    updateOutline,


    deleteOutline


  } = useOutlineSync();


  const { refreshCharacters } = useCharacterSync();


  // 确保项目大纲已加载


  useEffect(() => {


    if (!currentProject?.id) return;


    const projectId = currentProject.id;


    const existingProjectOutlines = useStore.getState().outlines.filter((outline) => outline.project_id === projectId);


    if (existingProjectOutlines.length === 0) {


      void refreshOutlines(projectId);


    }


    // eslint-disable-next-line react-hooks/exhaustive-deps


  }, [currentProject?.id]);


  const ensureProjectCharactersLoaded = useCallback((projectId = currentProject?.id) => {


    if (!projectId) {


      return;


    }


    const hasProjectCharacters = useStore.getState().characters.some((character) => character.project_id === projectId);


    if (!hasProjectCharacters) {


      void refreshCharacters(projectId);


    }


  }, [currentProject?.id, refreshCharacters]);


  const projectCharacters = useMemo(() => {


    if (!currentProject?.id) {


      return [];


    }


    return storeCharacters


      .filter((character) => character.project_id === currentProject.id)


      .map((char: Character) => ({


        label: char.name,


        value: char.name


      }));


  }, [currentProject?.id, storeCharacters]);


  const getOutlineHasChapters = async (outline: { id: string; has_chapters?: boolean }) => {


    if (typeof outline.has_chapters === 'boolean') {


      outlineChapterStatusCache.set(outline.id, outline.has_chapters);


      return outline.has_chapters;


    }


    const cachedStatus = outlineChapterStatusCache.get(outline.id);


    if (typeof cachedStatus === 'boolean') {


      return cachedStatus;


    }


    const existingRequest = outlineChapterStatusPromises.get(outline.id);


    if (existingRequest) {


      return existingRequest;


    }


    const request = (async () => {


      const chapters = await outlineApi.getOutlineChapters(outline.id);


      outlineChapterStatusCache.set(outline.id, chapters.has_chapters);


      return chapters.has_chapters;


    })();


    outlineChapterStatusPromises.set(outline.id, request);


    try {


      return await request;


    } finally {


      outlineChapterStatusPromises.delete(outline.id);


    }


  };


  // 按可见项渐进查询展开状态


  // 避免首屏对全部大纲并发查询


  if (!currentProject) return null;


  // 确保大纲按 order_index 排序


  const sortedOutlines = useMemo(


    () => [...outlines].sort((a, b) => a.order_index - b.order_index),


    [outlines]


  );


  useEffect(() => {


    setVisibleOutlineCount(Math.min(sortedOutlines.length, INITIAL_OUTLINE_RENDER_COUNT));


  }, [currentProject?.id, sortedOutlines.length]);


  useEffect(() => {


    if (visibleOutlineCount >= sortedOutlines.length) {


      return;


    }


    let timerId: number | null = null;


    const windowWithIdleCallback = window as Window & typeof globalThis & {


      requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;


      cancelIdleCallback?: (handle: number) => void;


    };


    let idleHandle: number | null = null;


    const expandVisibleOutlines = () => {


      setVisibleOutlineCount((prev) => {


        if (prev >= sortedOutlines.length) {


          return prev;


        }


        return Math.min(sortedOutlines.length, prev + OUTLINE_RENDER_BATCH_SIZE);


      });


    };


    if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {


      idleHandle = windowWithIdleCallback.requestIdleCallback(() => {


        idleHandle = null;


        expandVisibleOutlines();


      }, { timeout: OUTLINE_RENDER_BATCH_DELAY_MS * 4 });


    } else {


      timerId = window.setTimeout(() => {


        timerId = null;


        expandVisibleOutlines();


      }, OUTLINE_RENDER_BATCH_DELAY_MS);


    }


    return () => {


      if (idleHandle !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {


        windowWithIdleCallback.cancelIdleCallback(idleHandle);


      }


      if (timerId !== null) {


        window.clearTimeout(timerId);


      }


    };


  }, [sortedOutlines.length, visibleOutlineCount]);


  const visibleOutlines = useMemo(


    () => sortedOutlines.slice(0, visibleOutlineCount),


    [sortedOutlines, visibleOutlineCount]


  );


  useEffect(() => {


    let cancelled = false;


    let timerId: number | null = null;


    const windowWithIdleCallback = window as Window & typeof globalThis & {


      requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;


      cancelIdleCallback?: (handle: number) => void;


    };


    let idleHandle: number | null = null;


    const loadExpandStatus = async () => {


      if (visibleOutlines.length === 0) {


        setOutlineExpandStatus({});


        return;


      }


      const knownStatusEntries = visibleOutlines


        .filter((outline) => typeof outline.has_chapters === 'boolean')


        .map((outline) => [outline.id, Boolean(outline.has_chapters)] as const);


      if (knownStatusEntries.length > 0) {


        setOutlineExpandStatus((prev) => ({


          ...prev,


          ...Object.fromEntries(knownStatusEntries),


        }));


      }


      const outlinesNeedingFetch = visibleOutlines.filter((outline) => typeof outline.has_chapters !== 'boolean');


      if (outlinesNeedingFetch.length === 0) {


        return;


      }


      const statusEntries = await Promise.all(


        outlinesNeedingFetch.map(async (outline) => {


          try {


            return [outline.id, await getOutlineHasChapters(outline)] as const;


          } catch (error) {


            console.error(`获取大纲 ${outline.id} 展开状态失败:`, error);


            return [outline.id, false] as const;


          }


        })


      );


      if (!cancelled) {


        setOutlineExpandStatus((prev) => ({


          ...prev,


          ...Object.fromEntries(statusEntries),


        }));


      }


    };


    const scheduleLoadExpandStatus = () => {


      if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {


        idleHandle = windowWithIdleCallback.requestIdleCallback(() => {


          idleHandle = null;


          if (!cancelled) {


            void loadExpandStatus();


          }


        }, { timeout: 500 });


        return;


      }


      timerId = window.setTimeout(() => {


        timerId = null;


        if (!cancelled) {


          void loadExpandStatus();


        }


      }, OUTLINE_RENDER_BATCH_DELAY_MS);


    };


    scheduleLoadExpandStatus();


    return () => {


      cancelled = true;


      if (idleHandle !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {


        windowWithIdleCallback.cancelIdleCallback(idleHandle);


      }


      if (timerId !== null) {


        window.clearTimeout(timerId);


      }


    };


  }, [visibleOutlines]);


  const outlineItemStyle = useMemo(() => ({


    marginBottom: 16,


    padding: 0,


    border: 'none',


    contentVisibility: 'auto' as const,


    containIntrinsicSize: isMobile ? '560px' : '520px',


  }), [isMobile]);


  const parsedOutlineViewDataById = useMemo(() => {


    const parsedMap = new Map<string, ParsedOutlineViewData>();


    visibleOutlines.forEach((outline) => {


      const structureData = parseOutlineStructure(outline.structure);


      const characterEntries = parseCharacterEntries(structureData.characters);


      parsedMap.set(outline.id, {


        structureData,


        characterNames: getCharacterNames(characterEntries),


        organizationNames: getOrganizationNames(characterEntries),


      });


    });


    return parsedMap;


  }, [visibleOutlines]);


  const handleOpenEditModal = (id: string) => {


    const outline = outlines.find(o => o.id === id);


    if (outline) {


      ensureProjectCharactersLoaded(outline.project_id);


      // 解析structure数据


      const structureData = parseOutlineStructure(outline.structure);


      const editEntries = parseCharacterEntries(structureData.characters);


      const editCharNames = getCharacterNames(editEntries);


      const editOrgNames = getOrganizationNames(editEntries);


      


      // 处理场景数据 - 可能是字符串数组或对象数组


      let scenesText = '';


      if (structureData.scenes) {


        if (typeof structureData.scenes[0] === 'string') {


          // 字符串数组格式


          scenesText = (structureData.scenes as string[]).join('\n');


        } else {


          // 对象数组格式


          scenesText = (structureData.scenes as Array<{location: string; characters: string[]; purpose: string}>)


            .map(s => `${s.location}|${(s.characters || []).join('、')}|${s.purpose}`)


            .join('\n');


        }


      }


      


      // 处理情节要点数据


      const keyPointsText = structureData.key_points ? structureData.key_points.join('\n') : '';


      


      // 设置表单初始值


      editForm.setFieldsValue({


        title: outline.title,


        content: outline.content,


        characters: editCharNames,


        organizations: editOrgNames,


        scenes: scenesText,


        key_points: keyPointsText,


        emotion: structureData.emotion || '',


        goal: structureData.goal || ''


      });


      


      modalApi.confirm({


        title: '编辑大纲',


        width: 800,


        centered: true,


        styles: {


          body: {


            maxHeight: 'calc(100vh - 200px)',


            overflowY: 'auto'


          }


        },


        content: (


          <Form


            form={editForm}


            layout="vertical"


            style={{ marginTop: 12 }}


          >


            <Form.Item


              label="标题"


              name="title"


              rules={[{ required: true, message: '请输入标题' }]}


              style={{ marginBottom: 12 }}


            >


              <Input placeholder="输入大纲标题" />


            </Form.Item>


            <Form.Item


              label="内容"


              name="content"


              rules={[{ required: true, message: '请输入内容' }]}


              style={{ marginBottom: 12 }}


            >


              <TextArea rows={4} placeholder="输入大纲内容..." />


            </Form.Item>


            


            <Form.Item


              label="涉及角色"


              name="characters"


              tooltip="从项目角色中选择，也可以手动输入新角色名"


              style={{ marginBottom: 12 }}


            >


              <Select


                mode="tags"


                style={{ width: '100%' }}


                placeholder="选择或输入角色名"


                options={projectCharacters}


                tokenSeparators={[',', '，']}


                maxTagCount="responsive"


              />


            </Form.Item>


            


            <Form.Item


              label="涉及组织"


              name="organizations"


              tooltip="从项目组织中选择，也可以手动输入新组织名"


              style={{ marginBottom: 12 }}


            >


              <Select


                mode="tags"


                style={{ width: '100%' }}


                placeholder="选择或输入组织/势力名"


                tokenSeparators={[',', '，']}


                maxTagCount="responsive"


              />


            </Form.Item>


            


            <Form.Item


              label="场景信息"


              name="scenes"


              tooltip="支持两种格式：简单描述（每行一个场景）或详细格式（地点|角色|目的）"


              style={{ marginBottom: 12 }}


            >


              <TextArea


                rows={3}


                placeholder="每行一个场景&#10;详细格式：地点|角色1、角色2|目的"


              />


            </Form.Item>


            


            <Form.Item


              label="情节要点"


              name="key_points"


              tooltip="每行一个情节要点"


              style={{ marginBottom: 12 }}


            >


              <TextArea


                rows={2}


                placeholder="每行一个情节要点"


              />


            </Form.Item>


            


            <Form.Item


              label="情感基调"


              name="emotion"


              tooltip="描述本章的情感氛围"


              style={{ marginBottom: 12 }}


            >


              <Input placeholder="例如：冷冽与躁动并存" />


            </Form.Item>


            


            <Form.Item


              label="叙事目标"


              name="goal"


              tooltip="本章要达成的叙事目的"


              style={{ marginBottom: 0 }}


            >


              <Input placeholder="例如：建立世界观对比并完成主角初遇" />


            </Form.Item>


          </Form>


        ),


        okText: '更新',


        cancelText: '取消',


        onOk: async () => {


          const values = await editForm.validateFields();


          try {


            // 解析并重构structure数据


            const originalStructure = outline.structure ? JSON.parse(outline.structure) : {};


            


            // 处理角色和组织数据 - 合并为带类型标识的新格式


            const charNames = Array.isArray(values.characters)


              ? values.characters.filter((c: string) => c && c.trim())


              : [];


            const orgNames = Array.isArray(values.organizations)


              ? values.organizations.filter((c: string) => c && c.trim())


              : [];


            const characters: CharacterEntry[] = [


              ...charNames.map((name: string) => ({ name: name.trim(), type: 'character' as const })),


              ...orgNames.map((name: string) => ({ name: name.trim(), type: 'organization' as const }))


            ];


            


            // 处理场景数据 - 检测原始格式


            let scenes: string[] | Array<{location: string; characters: string[]; purpose: string}> | undefined;


            if (values.scenes) {


              const lines = values.scenes.split('\n')


                .map((line: string) => line.trim())


                .filter((line: string) => line);


              


              // 检查是否包含管道符，判断格式


              const hasStructuredFormat = lines.some((line: string) => line.includes('|'));


              


              if (hasStructuredFormat) {


                // 尝试解析为对象数组格式


                scenes = lines


                  .map((line: string) => {


                    const parts = line.split('|');


                    if (parts.length >= 3) {


                      return {


                        location: parts[0].trim(),


                        characters: parts[1].split('、').map(c => c.trim()).filter(c => c),


                        purpose: parts[2].trim()


                      };


                    }


                    return null;


                  })


                  .filter((s: { location: string; characters: string[]; purpose: string } | null): s is { location: string; characters: string[]; purpose: string } => s !== null);


              } else {


                // 保持字符串数组格式


                scenes = lines;


              }


            }


            


            // 处理情节要点数据


            const keyPoints = values.key_points


              ? values.key_points.split('\n')


                  .map((line: string) => line.trim())


                  .filter((line: string) => line)


              : undefined;


            


            // 合并structure数据，只包含AI实际生成的字段


            const newStructure = {


              ...originalStructure,


              title: values.title,


              summary: values.content,


              characters: characters.length > 0 ? characters : undefined,


              scenes: scenes && scenes.length > 0 ? scenes : undefined,


              key_points: keyPoints && keyPoints.length > 0 ? keyPoints : undefined,


              emotion: values.emotion || undefined,


              goal: values.goal || undefined


            };


            


            // 更新大纲


            await updateOutline(id, {


              title: values.title,


              content: values.content,


              structure: JSON.stringify(newStructure, null, 2)


            });


            


            message.success('大纲更新成功');


          } catch (error) {


            console.error('更新失败:', error);


            message.error('更新失败');


          }


        },


      });


    }


  };


  const handleDeleteOutline = async (id: string) => {


    try {


      await deleteOutline(id);


      message.success('删除成功');


      // 删除后刷新大纲列表和项目信息，更新字数显示


      await refreshOutlines();


      if (currentProject?.id) {


        const updatedProject = await projectApi.getProject(currentProject.id);


        setCurrentProject(updatedProject);


      }


    } catch {


      message.error('删除失败');


    }


  };


  interface GenerateFormValues {


    theme?: string;


    chapter_count?: number;


    narrative_perspective?: string;


    requirements?: string;


    provider?: string;


    model?: string;


    creative_mode?: CreativeMode;


    story_focus?: StoryFocus;


    story_creation_brief?: string;


    quality_preset?: QualityPreset;


    quality_notes?: string;


    mode?: 'auto' | 'new' | 'continue';


    story_direction?: string;


    plot_stage?: 'development' | 'climax' | 'ending';


    keep_existing?: boolean;


  }


  const handleGenerate = async (values: GenerateFormValues) => {


    try {


      setIsGenerating(true);


      // 添加详细的调试日志


      console.log('=== 大纲生成调试信息 ===');


      console.log('1. Form values 原始数据:', values);


      console.log('2. values.model:', values.model);


      console.log('3. values.provider:', values.provider);


      // 显示进度Modal


      setSSEProgress(0);


      setSSEMessage('正在连接生成服务...');


      setSSEModalVisible(true);


      // 准备请求数据


      const requestData: OutlineGenerateRequestData = {


        project_id: currentProject.id,


        genre: currentProject.genre || '通用',


        theme: values.theme || currentProject.theme || '',


        chapter_count: values.chapter_count || 5,


        narrative_perspective: values.narrative_perspective || currentProject.narrative_perspective || '第三人称',


        target_words: currentProject.target_words || 100000,


        requirements: values.requirements,


        creative_mode: values.creative_mode ?? projectDefaultCreativeMode,


        story_focus: values.story_focus ?? projectDefaultStoryFocus,


        story_creation_brief: values.story_creation_brief ?? projectDefaultStoryCreationBrief,


        quality_preset: values.quality_preset ?? projectDefaultQualityPreset,


        quality_notes: values.quality_notes ?? projectDefaultQualityNotes,


        mode: values.mode || 'auto',


        story_direction: values.story_direction,


        plot_stage: values.plot_stage || projectDefaultPlotStage || 'development'


      };


      // 只有在用户选择了模型时才添加model参数


      if (values.model) {


        requestData.model = values.model;


        console.log('4. 添加model到请求:', values.model);


      } else {


        console.log('4. values.model为空，不添加到请求');


      }


      // 添加provider参数（如果有）


      if (values.provider) {


        requestData.provider = values.provider;


        console.log('5. 添加provider到请求:', values.provider);


      }


      console.log('6. 最终请求数据:', JSON.stringify(requestData, null, 2));


      console.log('=========================');


      const payload: Record<string, unknown> = { ...requestData };


      delete payload.project_id;


      const task = await backgroundTaskApi.createTask({


        task_type: 'outline_generate',


        project_id: currentProject.id,


        payload,


      });


      message.success('大纲生成任务已转为后台执行，可继续进行其他操作');


      startGenerateTaskPolling(task.task_id);


    } catch (error) {


      console.error('AI生成失败:', error);


      message.error('智能生成失败');


      stopGenerateTaskPolling();


      generateTaskIdRef.current = null;


      setSSEModalVisible(false);


      setIsGenerating(false);


    }


  };


  const showGenerateModal = async () => {


    const hasOutlines = outlines.length > 0;




    // 直接加载可用模型列表


    const settingsResponse = await fetch('/api/settings');


    const settings = await settingsResponse.json();


    const { api_key, api_base_url, api_provider } = settings;


    let loadedModels: Array<{ value: string, label: string }> = [];


    let defaultModel: string | undefined = undefined;


    if (hasUsableApiCredentials(api_key, api_base_url)) {


      try {


        const modelsResponse = await fetch(


          `/api/settings/models?api_key=${encodeURIComponent(api_key)}&api_base_url=${encodeURIComponent(api_base_url)}&provider=${api_provider}`


        );


        if (modelsResponse.ok) {


          const data = await modelsResponse.json();


          if (data.models && data.models.length > 0) {


            loadedModels = data.models;


            defaultModel = settings.llm_model;


          }


        }


      } catch {


        console.log('获取模型列表失败，将使用默认模型');


      }


    }


    modalApi.confirm({


      title: hasOutlines ? (


        <Space>


          <span>智能生成/续写大纲</span>


          <Tag color="blue">当前已有 {outlines.length} 卷</Tag>


        </Space>


      ) : '智能生成大纲',


      width: 700,


      centered: true,


      content: (
        <Suspense fallback={outlineLazyFallback}>
          <LazyOutlineGenerateModalContent
            generateForm={generateForm}
            currentProject={currentProject}
            outlinesCount={outlines.length}
            loadedModels={loadedModels}
            defaultModel={defaultModel}
            projectDefaultCreativeMode={projectDefaultCreativeMode}
            projectDefaultStoryFocus={projectDefaultStoryFocus}
            projectDefaultPlotStage={projectDefaultPlotStage}
            projectDefaultStoryCreationBrief={projectDefaultStoryCreationBrief}
            projectDefaultQualityPreset={projectDefaultQualityPreset}
            projectDefaultQualityNotes={projectDefaultQualityNotes}
          />
        </Suspense>
      ),


      okText: hasOutlines ? '开始续写' : '开始生成',


      cancelText: '取消',


      onOk: async () => {


        const values = await generateForm.validateFields();


        await handleGenerate(values);


      },


    });


  };


  // 手动创建大纲


  const showManualCreateOutlineModal = () => {


    const nextOrderIndex = outlines.length > 0


      ? Math.max(...outlines.map(o => o.order_index)) + 1


      : 1;


    modalApi.confirm({


      title: '手动创建大纲',


      width: 600,


      centered: true,


      content: (


        <Form


          form={manualCreateForm}


          layout="vertical"


          initialValues={{ order_index: nextOrderIndex }}


          style={{ marginTop: 16 }}


        >


          <Form.Item


            label="大纲序号"


            name="order_index"


            rules={[{ required: true, message: '请输入序号' }]}


            tooltip={currentProject?.outline_mode === 'one-to-one' ? '在传统模式下，序号即章节编号' : '在细化模式下，序号为卷数'}


          >


            <InputNumber min={1} style={{ width: '100%' }} placeholder="自动计算的下一个序号" />


          </Form.Item>


          <Form.Item


            label="大纲标题"


            name="title"


            rules={[{ required: true, message: '请输入标题' }]}


          >


            <Input placeholder={currentProject?.outline_mode === 'one-to-one' ? '例如：第一章 初入江湖' : '例如：第一卷 初入江湖'} />


          </Form.Item>


          <Form.Item


            label="大纲内容"


            name="content"


            rules={[{ required: true, message: '请输入内容' }]}


          >


            <TextArea


              rows={6}


              placeholder="描述本章/卷的主要情节和发展方向..."


            />


          </Form.Item>


        </Form>


      ),


      okText: '创建',


      cancelText: '取消',


      onOk: async () => {


        const values = await manualCreateForm.validateFields();


        // 校验序号是否重复


        const existingOutline = outlines.find(o => o.order_index === values.order_index);


        if (existingOutline) {


          modalApi.warning({


            title: '序号冲突',


            content: (


              <div>


                <p>序号 <strong>{values.order_index}</strong> 已被使用：</p>


                <div style={{


                  padding: 12,


                  background: 'var(--color-warning-bg)',


                  borderRadius: 4,


                  border: '1px solid var(--color-warning-border)',


                  marginTop: 8


                }}>


                  <div style={{ fontWeight: 500, color: 'var(--color-warning)' }}>


                    {currentProject?.outline_mode === 'one-to-one'


                      ? `第${existingOutline.order_index}章`


                      : `第${existingOutline.order_index}卷`


                    }：{existingOutline.title}


                  </div>


                </div>


                <p style={{ marginTop: 12, color: 'var(--color-text-secondary)' }}>


                  💡 建议使用序号 <strong>{nextOrderIndex}</strong>，或选择其他未使用的序号


                </p>


              </div>


            ),


            okText: '我知道了',


            centered: true


          });


          throw new Error('序号重复');


        }


        try {


          await outlineApi.createOutline({


            project_id: currentProject.id,


            ...values


          });


          message.success('大纲创建成功');


          await refreshOutlines();


          manualCreateForm.resetFields();


        } catch (error: unknown) {


          const err = error as Error;


          if (err.message === '序号重复') {


            // 序号重复错误已经显示了Modal，不需要再显示message


            throw error;


          }


          message.error('创建失败：' + (err.message || '未知错误'));


          throw error;


        }


      }


    });


  };


  // 展开单个大纲为多章 - 使用SSE显示进度


  const handleExpandOutline = async (outlineId: string, outlineTitle: string) => {


    try {


      setIsExpanding(true);


      // ✅ 新增：检查是否需要按顺序展开


      const currentOutline = sortedOutlines.find(o => o.id === outlineId);


      if (currentOutline) {


        // 获取所有在当前大纲之前的大纲


        const previousOutlines = sortedOutlines.filter(


          o => o.order_index < currentOutline.order_index


        );


        // 检查前面的大纲是否都已展开


        for (const prevOutline of previousOutlines) {


          try {


            const prevChapters = await outlineApi.getOutlineChapters(prevOutline.id);


            if (!prevChapters.has_chapters) {


              // 如果前面有未展开的大纲，显示提示并阻止操作


              setIsExpanding(false);


              modalApi.warning({


                title: '请按顺序展开大纲',


                width: 600,


                centered: true,


                content: (


                  <div>


                    <p style={{ marginBottom: 12 }}>


                      为了保持章节编号的连续性和内容的连贯性，请先展开前面的大纲。


                    </p>


                    <div style={{


                      padding: 12,


                      background: 'var(--color-warning-bg)',


                      borderRadius: 4,


                      border: '1px solid var(--color-warning-border)'


                    }}>


                      <div style={{ fontWeight: 500, marginBottom: 8, color: 'var(--color-warning)' }}>


                        ⚠️ 需要先展开：


                      </div>


                      <div style={{ color: 'var(--color-text-secondary)' }}>


                        第{prevOutline.order_index}卷：《{prevOutline.title}》


                      </div>


                    </div>


                    <p style={{ marginTop: 12, color: 'var(--color-text-secondary)', fontSize: 13 }}>


                      💡 提示：您也可以使用「批量展开」功能，系统会自动按顺序处理所有大纲。


                    </p>


                  </div>


                ),


                okText: '我知道了'


              });


              return;


            }


          } catch (error) {


            console.error(`检查大纲 ${prevOutline.id} 失败:`, error);


            // 如果检查失败，继续处理（避免因网络问题阻塞）


          }


        }


      }


      // 第一步：检查是否已有展开的章节


      const existingChapters = await outlineApi.getOutlineChapters(outlineId);


      if (existingChapters.has_chapters && existingChapters.expansion_plans && existingChapters.expansion_plans.length > 0) {


        // 如果已有章节，显示已有的展开规划信息


        setIsExpanding(false);


        showExistingExpansionPreview(outlineTitle, existingChapters);


        return;


      }


      // 如果没有章节，显示展开表单


      setIsExpanding(false);


      modalApi.confirm({


        title: (


          <Space>


            <BranchesOutlined />


            <span>展开大纲为多章</span>


          </Space>


        ),


        width: 600,


        centered: true,


        content: (


          <div>


            <div style={{ marginBottom: 16, padding: 12, background: 'var(--color-bg-layout)', borderRadius: 4 }}>


              <div style={{ fontWeight: 500, marginBottom: 4 }}>大纲标题</div>


              <div style={{ color: 'var(--color-text-secondary)' }}>{outlineTitle}</div>


            </div>


            <Form


              form={expansionForm}


              layout="vertical"


              initialValues={{


                target_chapter_count: 3,


                expansion_strategy: 'balanced',


              }}


            >


              <Form.Item


                label="目标章节数"


                name="target_chapter_count"


                rules={[{ required: true, message: '请输入目标章节数' }]}


                tooltip="将这个大纲展开为几章内容"


              >


                <InputNumber


                  min={2}


                  max={10}


                  style={{ width: '100%' }}


                  placeholder="建议2-5章"


                />


              </Form.Item>


              <Form.Item


                label="展开策略"


                name="expansion_strategy"


                tooltip="选择如何分配内容到各章节"


              >


                <Radio.Group>


                  <Radio.Button value="balanced">均衡分配</Radio.Button>


                  <Radio.Button value="climax">高潮重点</Radio.Button>


                  <Radio.Button value="detail">细节丰富</Radio.Button>


                </Radio.Group>


              </Form.Item>


            </Form>


          </div>


        ),


        okText: '生成规划预览',


        cancelText: '取消',


        onOk: async () => {


          try {


            const values = await expansionForm.validateFields();


            // 显示SSE进度Modal


            setSSEProgress(0);


            setSSEMessage('正在准备展开大纲...');


            setSSEModalVisible(true);


            setIsExpanding(true);


            // 准备请求数据


            const requestData = {


              ...values,


              auto_create_chapters: false, // 第一步：仅生成规划


              enable_scene_analysis: true


            };


            const task = await backgroundTaskApi.createTask({


              task_type: 'outline_expand',


              project_id: currentProject.id,


              payload: {


                outline_id: outlineId,


                ...requestData,


              },


            });


            message.success('大纲展开任务已转为后台执行，可继续进行其他操作');


            startExpandTaskPolling(task.task_id, (result) => {


              if (!result) {


                message.error('展开任务完成但未返回规划数据');


                return;


              }


              showExpansionPreview(outlineId, result as unknown as OutlineExpansionResponse);


            });


          } catch (error) {


            console.error('展开失败:', error);


            message.error('展开失败');


            stopExpandTaskPolling();


            expandTaskIdRef.current = null;


            setSSEModalVisible(false);


            setIsExpanding(false);


          }


        },


      });


    } catch (error) {


      console.error('检查章节失败:', error);


      message.error('检查章节失败');


      setIsExpanding(false);


    }


  };


  // 删除展开的章节内容（保留大纲）


  const handleDeleteExpandedChapters = async (outlineTitle: string, chapters: Array<{ id: string }>) => {


    try {


      // 使用顺序删除避免并发导致的字数计算竞态条件


      // 并发删除会导致多个请求同时读取项目字数并各自减去章节字数，造成计算错误


      for (const chapter of chapters) {


        await chapterApi.deleteChapter(chapter.id);


      }


      message.success(`已删除《${outlineTitle}》展开的所有 ${chapters.length} 个章节`);


      await refreshOutlines();


      // 刷新项目信息以更新字数显示


      if (currentProject?.id) {


        const updatedProject = await projectApi.getProject(currentProject.id);


        setCurrentProject(updatedProject);


      }


      // 更新展开状态


      setOutlineExpandStatus(prev => {


        const newStatus = { ...prev };


        // 找到被删除章节对应的大纲ID并更新其状态


        const outlineId = Object.keys(newStatus).find(id =>


          outlines.find(o => o.id === id && o.title === outlineTitle)


        );


        if (outlineId) {


          newStatus[outlineId] = false;


        }


        return newStatus;


      });


    } catch (error: unknown) {


      const apiError = error as ApiError;


      message.error(apiError.response?.data?.detail || '删除章节失败');


    }


  };


  // 显示已存在章节的展开规划


  const showExistingExpansionPreview = (


    outlineTitle: string,


    data: {


      chapter_count: number;


      chapters: Array<{ id: string; chapter_number: number; title: string }>;


      expansion_plans: Array<{


        sub_index: number;


        title: string;


        plot_summary: string;


        key_events: string[];


        character_focus: string[];


        emotional_tone: string;


        narrative_goal: string;


        conflict_type: string;


        estimated_words: number;


        scenes?: Array<{


          location: string;


          characters: string[];


          purpose: string;


        }> | null;


      }> | null;


    }


  ) => {


    modalApi.info({


      title: (


        <Space style={{ flexWrap: 'wrap' }}>


          <CheckCircleOutlined style={{ color: 'var(--color-success)' }} />


          <span>《{outlineTitle}》展开信息</span>


        </Space>


      ),


      width: isMobile ? '95%' : 900,


      centered: true,


      style: isMobile ? {


        top: 20,


        maxWidth: 'calc(100vw - 16px)',


        margin: '0 8px'


      } : undefined,


      styles: {


        body: {


          maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(80vh - 60px)',


          overflowY: 'auto',


          overflowX: 'hidden'


        }


      },


      footer: (


        <Space wrap style={{ width: '100%', justifyContent: isMobile ? 'center' : 'flex-end' }}>


          <Button


            danger


            icon={<DeleteOutlined />}


            onClick={() => {


              Modal.destroyAll();


              modalApi.confirm({


                title: '确认删除',


                icon: <ExclamationCircleOutlined />,


                centered: true,


                content: (


                  <div>


                    <p>此操作将删除大纲《{outlineTitle}》展开的所有 <strong>{data.chapter_count}</strong> 个章节。</p>


                    <p style={{ color: 'var(--color-primary)', marginTop: 8 }}>


                      📝 注意：大纲本身会保留，您可以重新展开


                    </p>


                    <p style={{ color: '#ff4d4f', marginTop: 8 }}>


                      ⚠️ 警告：章节内容将永久删除且无法恢复！


                    </p>


                  </div>


                ),


                okText: '确认删除',


                okType: 'danger',


                cancelText: '取消',


                onOk: () => handleDeleteExpandedChapters(outlineTitle, data.chapters || []),


              });


            }}


            block={isMobile}


            size={isMobile ? 'middle' : undefined}


          >


            删除所有展开的章节 ({data.chapter_count}章)


          </Button>


          <Button onClick={() => Modal.destroyAll()}>


            关闭


          </Button>


        </Space>


      ),


      content: (
        <Suspense fallback={outlineLazyFallback}>
          <LazyOutlineExistingExpansionContent
            outlineTitle={outlineTitle}
            data={data}
            isMobile={isMobile}
          />
        </Suspense>
      ),


    });


  };


  // 显示展开规划预览，并提供确认创建章节的选项


  const showExpansionPreview = (outlineId: string, response: OutlineExpansionResponse) => {


    // 缓存AI生成的规划数据


    const cachedPlans = response.chapter_plans;


    modalApi.confirm({


      title: (


        <Space>


          <CheckCircleOutlined style={{ color: 'var(--color-success)' }} />


          <span>展开规划预览</span>


        </Space>


      ),


      width: 900,


      centered: true,


      okText: '确认并创建章节',


      cancelText: '暂不创建',


      content: (
        <Suspense fallback={outlineLazyFallback}>
          <LazyOutlineExpansionPreviewContent
            response={response}
            isMobile={isMobile}
          />
        </Suspense>
      ),


      onOk: async () => {


        // 第二步：用户确认后，直接使用缓存的规划创建章节（避免重复调用AI）


        await handleConfirmCreateChapters(outlineId, cachedPlans);


      },


      onCancel: () => {


        message.info('已取消创建章节');


      }


    });


  };


  // 确认创建章节 - 使用缓存的规划数据，避免重复AI调用


  const handleConfirmCreateChapters = async (


    outlineId: string,


    cachedPlans: ChapterPlanItem[]


  ) => {


    try {


      setIsExpanding(true);


      // 使用新的API端点，直接传递缓存的规划数据


      const response = await outlineApi.createChaptersFromPlans(outlineId, cachedPlans);


      message.success(


        `成功创建${response.chapters_created}个章节！`,


        3


      );


      console.log('✅ 使用缓存的规划创建章节，避免了重复的AI调用');


      // 刷新大纲和章节列表


      refreshOutlines();


    } catch (error) {


      console.error('创建章节失败:', error);


      message.error('创建章节失败');


    } finally {


      setIsExpanding(false);


    }


  };


  // 批量展开所有大纲 - 使用SSE流式显示进度


  const handleBatchExpandOutlines = () => {


    if (!currentProject?.id || outlines.length === 0) {


      message.warning('没有可展开的大纲');


      return;


    }


    modalApi.confirm({


      title: (


        <Space>


          <AppstoreAddOutlined />


          <span>批量展开所有大纲</span>


        </Space>


      ),


      width: 600,


      centered: true,


      content: (
        <Suspense fallback={outlineLazyFallback}>
          <LazyOutlineBatchExpandConfigForm
            form={batchExpansionForm}
            outlineCount={outlines.length}
          />
        </Suspense>
      ),


      okText: '开始展开',


      cancelText: '取消',


      okButtonProps: { type: 'primary' },


      onOk: async () => {


        try {


          const values = await batchExpansionForm.validateFields();


          // 显示SSE进度Modal


          setSSEProgress(0);


          setSSEMessage('正在准备批量展开...');


          setSSEModalVisible(true);


          setIsExpanding(true);


          // 准备请求数据


          const requestData = {


            project_id: currentProject.id,


            ...values,


            auto_create_chapters: false // 第一步：仅生成规划


          };


          const payload: Record<string, unknown> = { ...requestData };


          delete payload.project_id;


          const task = await backgroundTaskApi.createTask({


            task_type: 'outline_batch_expand',


            project_id: currentProject.id,


            payload,


          });


          message.success('批量展开任务已转为后台执行，可继续进行其他操作');


          startExpandTaskPolling(task.task_id, (result) => {


            if (!result) {


              message.error('批量展开完成但未返回规划数据');


              return;


            }


            const data = result as unknown as BatchOutlineExpansionResponse;


            console.log('批量展开完成，结果:', data);


            setCachedBatchExpansionResponse(data);


            setBatchPreviewData(data);


            setBatchPreviewVisible(true);


          });


        } catch (error) {


          console.error('批量展开失败:', error);


          message.error('批量展开失败');


          stopExpandTaskPolling();


          expandTaskIdRef.current = null;


          setSSEModalVisible(false);


          setIsExpanding(false);


        }


      },


    });


  };


  // 渲染批量展开预览 Modal 内容


  const handleBatchPreviewOk = async () => {


    setBatchPreviewVisible(false);


    await handleConfirmBatchCreateChapters();


  };


  // 处理批量预览取消


  const handleBatchPreviewCancel = () => {


    setBatchPreviewVisible(false);


    message.info('已取消创建章节，规划已保存');


  };


  // 确认批量创建章节 - 使用缓存的规划数据


  const handleConfirmBatchCreateChapters = async () => {


    try {


      setIsExpanding(true);


      // 使用缓存的规划数据，避免重复调用AI


      if (!cachedBatchExpansionResponse) {


        message.error('规划数据丢失，请重新展开');


        return;


      }


      console.log('✅ 使用缓存的批量规划数据创建章节，避免重复AI调用');


      // 逐个大纲创建章节


      let totalCreated = 0;


      const errors: string[] = [];


      for (const result of cachedBatchExpansionResponse.expansion_results) {


        try {


          // 使用create-chapters-from-plans接口，直接传递缓存的规划


          const response = await outlineApi.createChaptersFromPlans(


            result.outline_id,


            result.chapter_plans


          );


          totalCreated += response.chapters_created;


        } catch (error: unknown) {


          const apiError = error as ApiError;


          const err = error as Error;


          const errorMsg = apiError.response?.data?.detail || err.message || '未知错误';


          errors.push(`${result.outline_title}: ${errorMsg}`);


          console.error(`创建大纲 ${result.outline_title} 的章节失败:`, error);


        }


      }


      // 显示结果


      if (errors.length === 0) {


        message.success(


          `批量创建完成！共创建 ${totalCreated} 个章节`,


          3


        );


      } else {


        message.warning(


          `部分完成：成功创建 ${totalCreated} 个章节，${errors.length} 个失败`,


          5


        );


        console.error('失败详情:', errors);


      }


      // 清除缓存


      setCachedBatchExpansionResponse(null);


      // 刷新列表


      refreshOutlines();


    } catch (error) {


      console.error('批量创建章节失败:', error);


      message.error('批量创建章节失败');


    } finally {


      setIsExpanding(false);


    }


  };


  const handleOpenEditModalRef = useRef(handleOpenEditModal);


  const handleDeleteOutlineRef = useRef(handleDeleteOutline);


  const handleExpandOutlineRef = useRef(handleExpandOutline);


  useEffect(() => {


    handleOpenEditModalRef.current = handleOpenEditModal;


  }, [handleOpenEditModal]);


  useEffect(() => {


    handleDeleteOutlineRef.current = handleDeleteOutline;


  }, [handleDeleteOutline]);


  useEffect(() => {


    handleExpandOutlineRef.current = handleExpandOutline;


  }, [handleExpandOutline]);


  const openEditModalFromList = useCallback((id: string) => {


    handleOpenEditModalRef.current(id);


  }, []);


  const deleteOutlineFromList = useCallback((id: string) => {


    return handleDeleteOutlineRef.current(id);


  }, []);


  const expandOutlineFromList = useCallback((outlineId: string, outlineTitle: string) => {


    return handleExpandOutlineRef.current(outlineId, outlineTitle);


  }, []);


  const toggleScenesExpand = useCallback((id: string) => {


    setScenesExpandStatus((prev) => ({


      ...prev,


      [id]: !prev[id],


    }));


  }, []);


  const outlineMode = currentProject?.outline_mode;


  const outlineListItems = useMemo(() => (


    visibleOutlines.map((item) => {


      const parsedOutlineViewData = parsedOutlineViewDataById.get(item.id);


      const structureData = parsedOutlineViewData?.structureData ?? {};


      const characterNames = parsedOutlineViewData?.characterNames ?? [];


      const organizationNames = parsedOutlineViewData?.organizationNames ?? [];


      const outlineContentPreview = getPreviewText(item.content, isMobile ? 180 : 260);


      const isOutlineExpanded = outlineExpandStatus[item.id] ?? false;


      return (


        <List.Item


          key={item.id}


          style={outlineItemStyle}


        >


          <Card


            style={{


              width: '100%',


              borderRadius: isMobile ? 6 : 8,


              border: '1px solid #f0f0f0',


              boxShadow: '0 1px 2px rgba(0, 0, 0, 0.03)',


              transition: 'all 0.3s ease'


            }}


            bodyStyle={{


              padding: isMobile ? '10px 12px' : 16


            }}


            onMouseEnter={(e) => {


              if (!isMobile) {


                e.currentTarget.style.boxShadow = '0 4px 12px rgba(0, 0, 0, 0.08)';


                e.currentTarget.style.borderColor = 'var(--color-primary)';


              }


            }}


            onMouseLeave={(e) => {


              if (!isMobile) {


                e.currentTarget.style.boxShadow = '0 1px 2px rgba(0, 0, 0, 0.03)';


                e.currentTarget.style.borderColor = '#f0f0f0';


              }


            }}


          >


            <List.Item.Meta


              style={{ width: '100%' }}


              title={


                <Space size="small" style={{ fontSize: isMobile ? 13 : 16, flexWrap: 'wrap', lineHeight: isMobile ? '1.4' : '1.5' }}>


                  <span style={{ color: 'var(--color-primary)', fontWeight: 'bold', fontSize: isMobile ? 13 : 16 }}>


                    {outlineMode === 'one-to-one'


                      ? `第${item.order_index || '?'}章`


                      : `第${item.order_index || '?'}卷`


                    }


                  </span>


                  <span style={{ fontSize: isMobile ? 13 : 16 }}>{item.title}</span>


                  {/* ✅ 新增：展开状态标识 - 仅在一对多模式显示 */}


                  {outlineMode === 'one-to-many' && (


                    isOutlineExpanded ? (


                      <Tag color="success" icon={<CheckCircleOutlined />} style={{ fontSize: isMobile ? 11 : 12 }}>已展开</Tag>


                    ) : (


                      <Tag color="default" style={{ fontSize: isMobile ? 11 : 12 }}>未展开</Tag>


                    )


                  )}


                </Space>


              }


              description={


                <div style={{ fontSize: isMobile ? 12 : 14, lineHeight: isMobile ? '1.5' : '1.6' }}>


                  {/* 大纲内容 */}


                  <div style={{


                    marginBottom: isMobile ? 10 : 12,


                    padding: isMobile ? '8px 10px' : '10px 12px',


                    background: 'linear-gradient(135deg, #fafafa 0%, #f5f5f5 100%)',


                    borderLeft: '3px solid #8c8c8c',


                    borderRadius: isMobile ? 4 : 6,


                    fontSize: isMobile ? 12 : 13,


                    color: '#262626',


                    lineHeight: '1.6'


                  }}>


                    <div style={{


                      fontWeight: 600,


                      color: '#595959',


                      marginBottom: isMobile ? 4 : 6,


                      fontSize: isMobile ? 12 : 13


                    }}>


                      📝 大纲内容


                    </div>


                    <div


                      title={item.content}


                      style={{


                        padding: isMobile ? '6px 8px' : '6px 10px',


                        background: '#ffffff',


                        border: '1px solid #d9d9d9',


                        borderRadius: 4,


                        fontSize: isMobile ? 12 : 13,


                        color: '#262626',


                        lineHeight: '1.6'


                      }}


                    >


                      {outlineContentPreview}


                    </div>


                  </div>


                  {/* ✨ 涉及角色展示 - 优化版（支持角色/组织分类显示） */}


                  {characterNames.length > 0 && (


                    <div style={{


                      marginTop: isMobile ? 10 : 12,


                      padding: isMobile ? '8px 10px' : '10px 12px',


                      background: 'linear-gradient(135deg, #f5f3ff 0%, #faf5ff 100%)',


                      borderLeft: '3px solid #9333ea',


                      borderRadius: isMobile ? 4 : 6


                    }}>


                      <div style={{


                        display: 'flex',


                        alignItems: 'center',


                        gap: isMobile ? 6 : 8,


                        marginBottom: isMobile ? 6 : 8


                      }}>


                        <span style={{


                          fontSize: isMobile ? 12 : 13,


                          fontWeight: 600,


                          color: '#7c3aed',


                          display: 'flex',


                          alignItems: 'center',


                          gap: 4


                        }}>


                          👥 涉及角色


                          <Tag


                            color="purple"


                            style={{


                              margin: 0,


                              fontSize: 10,


                              borderRadius: 10,


                              padding: '0 6px'


                            }}


                          >


                            {characterNames.length}


                          </Tag>


                        </span>


                      </div>


                      <Space wrap size={[4, 4]}>


                        {characterNames.map((name, idx) => (


                          <Tag


                            key={idx}


                            color="purple"


                            style={{


                              margin: 0,


                              borderRadius: 4,


                              padding: isMobile ? '2px 8px' : '3px 10px',


                              fontSize: isMobile ? 11 : 12,


                              fontWeight: 500,


                              border: '1px solid #e9d5ff',


                              background: '#ffffff',


                              color: '#7c3aed',


                              whiteSpace: 'normal',


                              wordBreak: 'break-word',


                              height: 'auto',


                              lineHeight: '1.5'


                            }}


                          >


                            {name}


                          </Tag>


                        ))}


                      </Space>


                    </div>


                  )}


                  {/* 🏛️ 涉及组织展示 */}


                  {organizationNames.length > 0 && (


                    <div style={{


                      marginTop: isMobile ? 10 : 12,


                      padding: isMobile ? '8px 10px' : '10px 12px',


                      background: 'linear-gradient(135deg, #fff7ed 0%, #fffbeb 100%)',


                      borderLeft: '3px solid #ea580c',


                      borderRadius: isMobile ? 4 : 6


                    }}>


                      <div style={{


                        display: 'flex',


                        alignItems: 'center',


                        gap: isMobile ? 6 : 8,


                        marginBottom: isMobile ? 6 : 8


                      }}>


                        <span style={{


                          fontSize: isMobile ? 12 : 13,


                          fontWeight: 600,


                          color: '#ea580c',


                          display: 'flex',


                          alignItems: 'center',


                          gap: 4


                        }}>


                          🏛️ 涉及组织


                          <Tag


                            color="orange"


                            style={{


                              margin: 0,


                              fontSize: 10,


                              borderRadius: 10,


                              padding: '0 6px'


                            }}


                          >


                            {organizationNames.length}


                          </Tag>


                        </span>


                      </div>


                      <Space wrap size={[4, 4]}>


                        {organizationNames.map((name, idx) => (


                          <Tag


                            key={idx}


                            color="orange"


                            style={{


                              margin: 0,


                              borderRadius: 4,


                              padding: isMobile ? '2px 8px' : '3px 10px',


                              fontSize: isMobile ? 11 : 12,


                              fontWeight: 500,


                              border: '1px solid #fed7aa',


                              background: '#ffffff',


                              color: '#ea580c',


                              whiteSpace: 'normal',


                              wordBreak: 'break-word',


                              height: 'auto',


                              lineHeight: '1.5'


                            }}


                          >


                            {name}


                          </Tag>


                        ))}


                      </Space>


                    </div>


                  )}


                  {/* ✨ 场景信息展示 - 优化版（支持折叠，最多显示3个） */}


                  {structureData.scenes && structureData.scenes.length > 0 ? (() => {


                    const isExpanded = scenesExpandStatus[item.id] || false;


                    const maxVisibleScenes = 4;


                    const hasMoreScenes = structureData.scenes!.length > maxVisibleScenes;


                    const visibleScenes = isExpanded ? structureData.scenes : structureData.scenes!.slice(0, maxVisibleScenes);


                    return (


                      <div style={{


                        marginTop: isMobile ? 10 : 12,


                        padding: isMobile ? '8px 10px' : '10px 12px',


                        background: 'linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 100%)',


                        borderLeft: '3px solid #0ea5e9',


                        borderRadius: isMobile ? 4 : 6


                      }}>


                        <div style={{


                          display: 'flex',


                          alignItems: 'center',


                          justifyContent: 'space-between',


                          marginBottom: isMobile ? 6 : 8,


                          flexWrap: isMobile ? 'wrap' : 'nowrap',


                          gap: isMobile ? 4 : 0


                        }}>


                          <span style={{


                            fontSize: isMobile ? 12 : 13,


                            fontWeight: 600,


                            color: '#0284c7',


                            display: 'flex',


                            alignItems: 'center',


                            gap: 4


                          }}>


                            🎬 场景设定


                            <Tag


                              color="cyan"


                              style={{


                                margin: 0,


                                fontSize: 10,


                                borderRadius: 10,


                                padding: '0 6px'


                              }}


                            >


                              {structureData.scenes!.length}


                            </Tag>


                          </span>


                          {hasMoreScenes && (


                            <Button


                              type="text"


                              size="small"


                              onClick={() => setScenesExpandStatus(prev => ({


                                ...prev,


                                [item.id]: !isExpanded


                              }))}


                              style={{


                                fontSize: isMobile ? 10 : 11,


                                height: isMobile ? 20 : 22,


                                padding: isMobile ? '0 6px' : '0 8px',


                                color: '#0284c7'


                              }}


                            >


                              {isExpanded ? '收起 ▲' : `展开 (${structureData.scenes!.length - maxVisibleScenes}+) ▼`}


                            </Button>


                          )}


                        </div>


                        {/* 使用grid布局，移动端一列，桌面端两列 */}


                        <div style={{


                          display: 'grid',


                          gridTemplateColumns: isMobile ? '1fr' : 'repeat(auto-fill, minmax(280px, 1fr))',


                          gap: isMobile ? 6 : 8,


                          width: '100%',


                          minWidth: 0  // 防止grid子元素溢出


                        }}>


                          {visibleScenes!.map((scene, idx) => {


                          // 判断是字符串还是对象


                          if (typeof scene === 'string') {


                            // 字符串格式：简洁卡片


                            return (


                              <div


                                key={idx}


                                style={{


                                  padding: isMobile ? '6px 8px' : '8px 10px',


                                  background: '#ffffff',


                                  border: '1px solid #bae6fd',


                                  borderRadius: isMobile ? 4 : 6,


                                  fontSize: isMobile ? 11 : 12,


                                  color: '#0c4a6e',


                                  display: 'flex',


                                  alignItems: 'flex-start',


                                  gap: isMobile ? 6 : 8,


                                  transition: 'all 0.2s ease',


                                  cursor: 'default',


                                  width: '100%',


                                  minWidth: 0,


                                  boxSizing: 'border-box'


                                }}


                                onMouseEnter={(e) => {


                                  if (!isMobile) {


                                    e.currentTarget.style.borderColor = '#0ea5e9';


                                    e.currentTarget.style.boxShadow = '0 2px 8px rgba(14, 165, 233, 0.15)';


                                  }


                                }}


                                onMouseLeave={(e) => {


                                  if (!isMobile) {


                                    e.currentTarget.style.borderColor = '#bae6fd';


                                    e.currentTarget.style.boxShadow = 'none';


                                  }


                                }}


                              >


                                <Tag


                                  color="cyan"


                                  style={{


                                    margin: 0,


                                    fontSize: 10,


                                    borderRadius: 4,


                                    flexShrink: 0


                                  }}


                                >


                                  {idx + 1}


                                </Tag>


                                <span style={{


                                  flex: 1,


                                  lineHeight: '1.6',


                                  overflow: 'hidden',


                                  textOverflow: 'ellipsis',


                                  whiteSpace: 'nowrap'


                                }}>{scene}</span>


                              </div>


                            );


                          } else {


                            // 对象格式：详细卡片


                            return (


                              <div


                                key={idx}


                                style={{


                                  padding: isMobile ? '8px 10px' : '10px 12px',


                                  background: '#ffffff',


                                  border: '1px solid #bae6fd',


                                  borderRadius: isMobile ? 4 : 6,


                                  fontSize: isMobile ? 11 : 12,


                                  transition: 'all 0.2s ease',


                                  cursor: 'default',


                                  width: '100%',


                                  minWidth: 0,


                                  boxSizing: 'border-box'


                                }}


                                onMouseEnter={(e) => {


                                  if (!isMobile) {


                                    e.currentTarget.style.borderColor = '#0ea5e9';


                                    e.currentTarget.style.boxShadow = '0 2px 8px rgba(14, 165, 233, 0.15)';


                                  }


                                }}


                                onMouseLeave={(e) => {


                                  if (!isMobile) {


                                    e.currentTarget.style.borderColor = '#bae6fd';


                                    e.currentTarget.style.boxShadow = 'none';


                                  }


                                }}


                              >


                                <div style={{


                                  display: 'flex',


                                  alignItems: 'center',


                                  gap: isMobile ? 6 : 8,


                                  marginBottom: isMobile ? 4 : 6,


                                  flexWrap: 'wrap'


                                }}>


                                  <Tag


                                    color="cyan"


                                    style={{


                                      margin: 0,


                                      fontSize: 10,


                                      borderRadius: 4


                                    }}


                                  >


                                    场景{idx + 1}


                                  </Tag>


                                  <span style={{


                                    fontWeight: 600,


                                    color: '#0c4a6e',


                                    fontSize: isMobile ? 12 : 13,


                                    flex: 1,


                                    overflow: 'hidden',


                                    textOverflow: 'ellipsis',


                                    whiteSpace: 'nowrap'


                                  }}>


                                    📍 {scene.location}


                                  </span>


                                </div>


                                {scene.characters && scene.characters.length > 0 && (


                                  <div style={{


                                    fontSize: isMobile ? 10 : 11,


                                    color: '#64748b',


                                    marginBottom: 4,


                                    paddingLeft: isMobile ? 2 : 4,


                                    overflow: 'hidden',


                                    textOverflow: 'ellipsis',


                                    whiteSpace: 'nowrap'


                                  }}>


                                    <span style={{ fontWeight: 500 }}>👤 角色：</span>


                                    {scene.characters.join(' · ')}


                                  </div>


                                )}


                                {scene.purpose && (


                                  <div style={{


                                    fontSize: isMobile ? 10 : 11,


                                    color: '#64748b',


                                    paddingLeft: isMobile ? 2 : 4,


                                    lineHeight: '1.5',


                                    overflow: 'hidden',


                                    textOverflow: 'ellipsis',


                                    whiteSpace: 'nowrap'


                                  }}>


                                    <span style={{ fontWeight: 500 }}>🎯 目的：</span>


                                    {scene.purpose}


                                  </div>


                                )}


                              </div>


                            );


                          }


                          })}


                        </div>


                      </div>


                    );


                  })() : null}


                {/* ✨ 关键事件展示 */}


                {structureData.key_events && structureData.key_events.length > 0 && (


                  <div style={{


                    marginTop: 12,


                    padding: '10px 12px',


                    background: 'linear-gradient(135deg, #fff7ed 0%, #ffedd5 100%)',


                    borderLeft: '3px solid #f97316',


                    borderRadius: 6


                  }}>


                    <div style={{


                      display: 'flex',


                      alignItems: 'center',


                      gap: 8,


                      marginBottom: 8


                    }}>


                      <span style={{


                        fontSize: 13,


                        fontWeight: 600,


                        color: '#ea580c',


                        display: 'flex',


                        alignItems: 'center',


                        gap: 4


                      }}>


                        ⚡ 关键事件


                        <Tag


                          color="orange"


                          style={{


                            margin: 0,


                            fontSize: 11,


                            borderRadius: 10,


                            padding: '0 6px'


                          }}


                        >


                          {structureData.key_events.length}


                        </Tag>


                      </span>


                    </div>


                    <Space direction="vertical" size={6} style={{ width: '100%' }}>


                      {structureData.key_events.map((event, idx) => (


                        <div


                          key={idx}


                          style={{


                            padding: '6px 10px',


                            background: '#ffffff',


                            border: '1px solid #fed7aa',


                            borderRadius: 4,


                            fontSize: 12,


                            color: '#9a3412',


                            display: 'flex',


                            alignItems: 'flex-start',


                            gap: 8


                          }}


                        >


                          <Tag


                            color="orange"


                            style={{


                              margin: 0,


                              fontSize: 11,


                              borderRadius: 4,


                              flexShrink: 0


                            }}


                          >


                            {idx + 1}


                          </Tag>


                          <span style={{


                            flex: 1,


                            lineHeight: '1.6',


                            overflow: 'hidden',


                            textOverflow: 'ellipsis',


                            whiteSpace: 'nowrap'


                          }}>{event}</span>


                        </div>


                      ))}


                    </Space>


                  </div>


                )}


                {/* ✨ 情节要点展示 (key_points) */}


                {structureData.key_points && structureData.key_points.length > 0 && (


                  <div style={{


                    marginTop: 12,


                    padding: '10px 12px',


                    background: 'linear-gradient(135deg, #f0fdf4 0%, #dcfce7 100%)',


                    borderLeft: '3px solid #22c55e',


                    borderRadius: 6


                  }}>


                    <div style={{


                      display: 'flex',


                      alignItems: 'center',


                      gap: 8,


                      marginBottom: 8


                    }}>


                      <span style={{


                        fontSize: 13,


                        fontWeight: 600,


                        color: '#15803d',


                        display: 'flex',


                        alignItems: 'center',


                        gap: 4


                      }}>


                        💡 情节要点


                        <Tag


                          color="green"


                          style={{


                            margin: 0,


                            fontSize: 11,


                            borderRadius: 10,


                            padding: '0 6px'


                          }}


                        >


                          {structureData.key_points.length}


                        </Tag>


                      </span>


                    </div>


                    {/* 使用grid布局，移动端一列，桌面端两列 */}


                    <div style={{


                      display: 'grid',


                      gridTemplateColumns: isMobile ? '1fr' : 'repeat(auto-fill, minmax(280px, 1fr))',


                      gap: isMobile ? 6 : 8,


                      width: '100%',


                      minWidth: 0


                    }}>


                      {structureData.key_points.map((point, idx) => (


                        <div


                          key={idx}


                          style={{


                            padding: isMobile ? '6px 8px' : '8px 10px',


                            background: '#ffffff',


                            border: '1px solid #bbf7d0',


                            borderRadius: isMobile ? 4 : 6,


                            fontSize: isMobile ? 11 : 12,


                            color: '#166534',


                            display: 'flex',


                            alignItems: 'flex-start',


                            gap: isMobile ? 6 : 8,


                            transition: 'all 0.2s ease',


                            cursor: 'default',


                            width: '100%',


                            minWidth: 0,


                            boxSizing: 'border-box'


                          }}


                          onMouseEnter={(e) => {


                            if (!isMobile) {


                              e.currentTarget.style.borderColor = '#22c55e';


                              e.currentTarget.style.boxShadow = '0 2px 8px rgba(34, 197, 94, 0.15)';


                            }


                          }}


                          onMouseLeave={(e) => {


                            if (!isMobile) {


                              e.currentTarget.style.borderColor = '#bbf7d0';


                              e.currentTarget.style.boxShadow = 'none';


                            }


                          }}


                        >


                          <Tag


                            color="green"


                            style={{


                              margin: 0,


                              fontSize: 10,


                              borderRadius: 4,


                              flexShrink: 0


                            }}


                          >


                            {idx + 1}


                          </Tag>


                          <span style={{


                            flex: 1,


                            lineHeight: '1.6',


                            overflow: 'hidden',


                            textOverflow: 'ellipsis',


                            whiteSpace: 'nowrap'


                          }}>{point}</span>


                        </div>


                      ))}


                    </div>


                  </div>


                )}


                {/* ✨ 情感基调展示 (emotion) */}


                {structureData.emotion && (


                  <div style={{


                    marginTop: 12,


                    padding: '10px 12px',


                    background: 'linear-gradient(135deg, #fef3c7 0%, #fde68a 100%)',


                    borderLeft: '3px solid #f59e0b',


                    borderRadius: 6,


                    display: 'flex',


                    alignItems: 'center',


                    gap: 8


                  }}>


                    <span style={{


                      fontSize: 13,


                      fontWeight: 600,


                      color: '#b45309'


                    }}>


                      💫 情感基调：


                    </span>


                    <Tag


                      color="gold"


                      style={{


                        margin: 0,


                        fontSize: 12,


                        padding: '2px 12px',


                        borderRadius: 12,


                        background: '#ffffff',


                        border: '1px solid #fbbf24',


                        color: '#b45309'


                      }}


                    >


                      {structureData.emotion}


                    </Tag>


                  </div>


                )}


                {/* ✨ 叙事目标展示 (goal) */}


                {structureData.goal && (


                  <div style={{


                    marginTop: 12,


                    padding: '10px 12px',


                    background: 'linear-gradient(135deg, #dbeafe 0%, #bfdbfe 100%)',


                    borderLeft: '3px solid #3b82f6',


                    borderRadius: 6


                  }}>


                    <div style={{


                      fontSize: 13,


                      fontWeight: 600,


                      color: '#1e40af',


                      marginBottom: 6


                    }}>


                      🎯 叙事目标


                    </div>


                    <div style={{


                      fontSize: 12,


                      color: '#1e3a8a',


                      lineHeight: '1.6',


                      padding: '6px 10px',


                      background: '#ffffff',


                      border: '1px solid #93c5fd',


                      borderRadius: 4,


                      overflow: 'hidden',


                      textOverflow: 'ellipsis',


                      whiteSpace: 'nowrap'


                    }}>


                      {structureData.goal}


                    </div>


                  </div>


                )}


              </div>


            }


          />


            {/* 操作按钮区域 - 在卡片内部 */}


            <div style={{


              marginTop: 16,


              paddingTop: 12,


              borderTop: '1px solid #f0f0f0',


              display: 'flex',


              justifyContent: 'flex-end',


              gap: 8


            }}>


              {outlineMode === 'one-to-many' && (


                <Button


                  icon={<BranchesOutlined />}


                  onClick={() => expandOutlineFromList(item.id, item.title)}


                  loading={isExpanding}


                  size={isMobile ? 'middle' : 'small'}


                >


                  展开


                </Button>


              )}


              <Button


                icon={<EditOutlined />}


                onClick={() => openEditModalFromList(item.id)}


                size={isMobile ? 'middle' : 'small'}


              >


                编辑


              </Button>


              <Popconfirm


                title="确定删除这条大纲吗？"


                onConfirm={() => deleteOutlineFromList(item.id)}


                okText="确定"


                cancelText="取消"


              >


                <Button


                  danger


                  icon={<DeleteOutlined />}


                  size={isMobile ? 'middle' : 'small'}


                >


                  删除


                </Button>


              </Popconfirm>


            </div>


          </Card>


        </List.Item>


      );


    })


  ), [


    visibleOutlines,


    parsedOutlineViewDataById,


    outlineItemStyle,


    isMobile,


    outlineMode,


    outlineExpandStatus,


    scenesExpandStatus,


    isExpanding,


    openEditModalFromList,


    deleteOutlineFromList,


    expandOutlineFromList,


    toggleScenesExpand,


  ]);


  return (


    <>


      {/* 批量展开预览 Modal */}


      {batchPreviewVisible ? (


        <Suspense fallback={null}>


          <LazyOutlineBatchPreviewModal


            visible={batchPreviewVisible}


            data={batchPreviewData}


            onOk={handleBatchPreviewOk}


            onCancel={handleBatchPreviewCancel}


          />


        </Suspense>


      ) : null}


      {contextHolder}


      {/* SSE进度Modal - 使用统一组件 */}


      {sseModalVisible ? (


        <Suspense fallback={null}>


          <LazySSEProgressModal


            visible={sseModalVisible}


            progress={sseProgress}


            message={sseMessage}


            title="正在生成中..."


            blocking={false}


            onCancel={() => {


              if (isGenerating) {


                void handleCancelGenerateTask();


                return;


              }


              if (isExpanding) {


                void handleCancelExpandTask();


              }


            }}


            cancelButtonText="取消任务"


          />


        </Suspense>


      ) : null}


      <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>


        {/* 页面头部 */}


        <div style={{


          position: 'sticky',


          top: 0,


          zIndex: 10,


          backgroundColor: 'var(--color-bg-container)',


          padding: isMobile ? '12px 0' : '16px 0',


          marginBottom: isMobile ? 12 : 16,


          borderBottom: '1px solid #f0f0f0',


          display: 'flex',


          flexDirection: isMobile ? 'column' : 'row',


          gap: isMobile ? 12 : 0,


          justifyContent: 'space-between',


          alignItems: isMobile ? 'stretch' : 'center'


        }}>


          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>


            <h2 style={{ margin: 0, fontSize: isMobile ? 18 : 24 }}>


              <FileTextOutlined style={{ marginRight: 8 }} />


              故事大纲





            </h2>


            {currentProject?.outline_mode && (


              <Tag color={currentProject.outline_mode === 'one-to-one' ? 'blue' : 'green'} style={{ width: 'fit-content' }}>


                {currentProject.outline_mode === 'one-to-one' ? '传统模式 (1→1)' : '细化模式 (1→N)'}


              </Tag>


            )}


          </div>


          <Space size="small" wrap={isMobile}>


            <Button


              icon={<PlusOutlined />}


              onClick={showManualCreateOutlineModal}


              block={isMobile}


            >


              手动创建


            </Button>


            <Button


              type="primary"


              icon={<ThunderboltOutlined />}


              onClick={showGenerateModal}


              loading={isGenerating}


              block={isMobile}


            >


              {isMobile ? '智能生成/续写' : '智能生成/续写大纲'}


            </Button>


            {outlines.length > 0 && currentProject?.outline_mode === 'one-to-many' && (


              <Button


                icon={<AppstoreAddOutlined />}


                onClick={handleBatchExpandOutlines}


                loading={isExpanding}


                title="将所有大纲展开为多章，实现从大纲到章节的一对多关系"


                >


                {isMobile ? '批量展开' : '批量展开为多章'}


              </Button>


            )}


          </Space>


        </div>


        {/* 大纲列表 */}


        <div style={{ flex: 1, overflowY: 'auto' }}>


          {outlines.length === 0 ? (


            <Empty description="还没有大纲，开始创建吧！" />


          ) : (


            <>


              <List>


                {outlineListItems}


              </List>


              {visibleOutlineCount < sortedOutlines.length ? (


                <div style={{


                  padding: '8px 0 16px',


                  textAlign: 'center',


                  fontSize: 12,


                  color: '#8c8c8c'


                }}>


                  还有 {sortedOutlines.length - visibleOutlineCount} 个大纲正在加载...


                </div>


              ) : null}


            </>


          )}


        </div>


      </div>


    </>


  );


}


