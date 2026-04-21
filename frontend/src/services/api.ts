/**
 * @deprecated services/api.ts 仅保留历史导入路径与默认 api 转发。
 * 新运行时代码请优先使用 services/modularApi.ts 或对应 services/modules/*。
 *
 * 兼容策略：命名导出统一透传 modularApi.ts，避免兼容层与主入口的导出清单再次漂移。
 */
export * from './modularApi';
export { api as default } from './core/httpClient';