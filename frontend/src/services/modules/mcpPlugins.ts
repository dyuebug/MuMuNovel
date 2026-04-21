import type {
  MCPPlugin,
  MCPPluginCreate,
  MCPPluginUpdate,
  MCPTestResult,
  MCPTool,
  MCPToolCallRequest,
  MCPToolCallResponse,
} from '../../types';
import { api } from '../core/httpClient';

interface MCPPluginSimpleCreate {
  config_json: string;
  enabled: boolean;
}

export const mcpPluginApi = {
  getPlugins: () =>
    api.get<unknown, MCPPlugin[]>('/mcp/plugins'),

  getPlugin: (id: string) =>
    api.get<unknown, MCPPlugin>(`/mcp/plugins/${id}`),

  createPlugin: (data: MCPPluginCreate) =>
    api.post<unknown, MCPPlugin>('/mcp/plugins', data),

  createPluginSimple: (data: MCPPluginSimpleCreate) =>
    api.post<unknown, MCPPlugin>('/mcp/plugins/simple', data),

  updatePlugin: (id: string, data: MCPPluginUpdate) =>
    api.put<unknown, MCPPlugin>(`/mcp/plugins/${id}`, data),

  deletePlugin: (id: string) =>
    api.delete<unknown, { message: string }>(`/mcp/plugins/${id}`),

  togglePlugin: (id: string, enabled: boolean) =>
    api.post<unknown, MCPPlugin>(`/mcp/plugins/${id}/toggle`, null, { params: { enabled } }),

  testPlugin: (id: string) =>
    api.post<unknown, MCPTestResult>(`/mcp/plugins/${id}/test`),

  getPluginTools: (id: string) =>
    api.get<unknown, { tools: MCPTool[] }>(`/mcp/plugins/${id}/tools`),

  callTool: (data: MCPToolCallRequest) =>
    api.post<unknown, MCPToolCallResponse>('/mcp/call', data),
};