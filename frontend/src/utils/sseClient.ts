/* eslint-disable @typescript-eslint/no-explicit-any */
export interface SSEMessage {
  type: 'progress' | 'chunk' | 'result' | 'error' | 'done';
  message?: string;
  progress?: number;
  word_count?: number;
  status?: 'processing' | 'success' | 'error' | 'warning';
  content?: string;
  data?: any;
  error?: string;
  code?: number;
}

export interface SSEClientOptions {
  onProgress?: (message: string, progress: number, status: string, wordCount?: number) => void;
  onChunk?: (content: string) => void;
  onResult?: (data: any) => void;
  onError?: (error: string, code?: number) => void;
  onCancelled?: (message: string) => void;
  onTaskCreated?: (taskId: string) => void;
  onComplete?: () => void;
  onConnectionError?: (error: Event) => void;
  onHeartbeat?: () => void;
  inactivityTimeoutMs?: number;
  signal?: AbortSignal;
}

const DEFAULT_SSE_INACTIVITY_TIMEOUT_MS = 45000;

const parseSSEBlock = (block: string): { isHeartbeat: boolean; data: string | null } => {
  const lines = block.split(/\r?\n/);
  const dataLines: string[] = [];
  let isHeartbeat = false;

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();
    if (!line) {
      continue;
    }

    if (line.startsWith(':')) {
      isHeartbeat = true;
      continue;
    }

    if (line.startsWith('data:')) {
      dataLines.push(line.slice(5).trimStart());
    }
  }

  return {
    isHeartbeat,
    data: dataLines.length > 0 ? dataLines.join('\n') : null,
  };
};

export class SSEClient {
  private eventSource: EventSource | null = null;
  private url: string;
  private options: SSEClientOptions;
  private accumulatedContent: string = '';

  constructor(url: string, options: SSEClientOptions = {}) {
    this.url = url;
    this.options = options;
  }

  connect(): Promise<any> {
    return new Promise((resolve, reject) => {
      try {
        this.eventSource = new EventSource(this.url);

        this.eventSource.onmessage = (event) => {
          try {
            const message: SSEMessage = JSON.parse(event.data);
            this.handleMessage(message, resolve, reject);
          } catch (error) {
            console.error('解析SSE消息失败:', error);
          }
        };

        this.eventSource.onerror = (error) => {
          console.error('SSE连接错误:', error);
          if (this.options.onConnectionError) {
            this.options.onConnectionError(error);
          }
          this.close();
          reject(new Error('SSE连接失败'));
        };

      } catch (error) {
        reject(error);
      }
    });
  }

  private handleMessage(message: SSEMessage, resolve: (value: any) => void, reject: (reason?: any) => void) {
    switch (message.type) {
      case 'progress':
        if (this.options.onProgress && message.progress !== undefined) {
          this.options.onProgress(
            message.message || '',
            message.progress,
            message.status || 'processing',
            message.word_count
          );
        }
        break;

      case 'chunk':
        if (message.content) {
          this.accumulatedContent += message.content;
          if (this.options.onChunk) {
            this.options.onChunk(message.content);
          }
        }
        break;

      case 'result':
        if (this.options.onResult && message.data) {
          this.options.onResult(message.data);
        }
        break;

      case 'error':
        if (this.options.onError) {
          this.options.onError(message.error || '未知错误', message.code);
        }
        this.close();
        reject(new Error(message.error || '未知错误'));
        break;

      case 'done':
        if (this.options.onComplete) {
          this.options.onComplete();
        }
        this.close();
        if (!this.options.onResult && this.accumulatedContent) {
          resolve({ content: this.accumulatedContent });
        } else {
          resolve(true);
        }
        break;
    }
  }

  close() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
  }

  getAccumulatedContent(): string {
    return this.accumulatedContent;
  }
}

export class SSEPostClient {
  private url: string;
  private data: any;
  private options: SSEClientOptions;
  private abortController: AbortController | null = null;
  private externalAbortCleanup: (() => void) | null = null;
  private accumulatedContent: string = '';
  private resultData: any = null;
  private settled = false;
  private inactivityTimer: number | null = null;

  constructor(url: string, data: any, options: SSEClientOptions = {}) {
    this.url = url;
    this.data = data;
    this.options = options;
  }

  async connect(): Promise<any> {
    return new Promise((resolve, reject) => {
      void this.connectInternal(resolve, reject);
    });
  }

  private resolveOnce(resolve: (value: any) => void, value: any) {
    if (this.settled) {
      return;
    }
    this.settled = true;
    this.clearInactivityTimer();
    resolve(value);
  }

  private rejectOnce(reject: (reason?: any) => void, reason: any) {
    if (this.settled) {
      return;
    }
    this.settled = true;
    this.clearInactivityTimer();
    reject(reason);
  }

  private getInactivityTimeoutMs(): number {
    return this.options.inactivityTimeoutMs ?? DEFAULT_SSE_INACTIVITY_TIMEOUT_MS;
  }

  private clearInactivityTimer() {
    if (this.inactivityTimer !== null) {
      window.clearTimeout(this.inactivityTimer);
      this.inactivityTimer = null;
    }
  }

  private markActivity(reject: (reason?: any) => void) {
    this.clearInactivityTimer();

    const timeoutMs = this.getInactivityTimeoutMs();
    if (timeoutMs <= 0 || this.settled) {
      return;
    }

    this.inactivityTimer = window.setTimeout(() => {
      const timeoutError = new Error('SSE connection timed out due to inactivity');
      if (this.options.onError) {
        this.options.onError(timeoutError.message, 408);
      }
      this.abort();
      this.rejectOnce(reject, timeoutError);
    }, timeoutMs);
  }

  private handleHeartbeat(reject: (reason?: any) => void) {
    this.markActivity(reject);
    if (this.options.onHeartbeat) {
      this.options.onHeartbeat();
    }
  }

  private finalizeStream(resolve: (value: any) => void, reject: (reason?: any) => void) {
    if (this.settled) {
      return;
    }

    if (this.resultData) {
      this.resolveOnce(resolve, this.resultData);
      return;
    }

    if (this.accumulatedContent) {
      this.resolveOnce(resolve, { content: this.accumulatedContent });
      return;
    }

    const streamClosedError = new Error('SSE stream closed before completion');
    if (this.options.onError) {
      this.options.onError(streamClosedError.message);
    }
    this.rejectOnce(reject, streamClosedError);
  }

  private async connectInternal(resolve: (value: any) => void, reject: (reason?: any) => void) {
    try {
      this.abortController = new AbortController();

      if (this.options.signal) {
        if (this.options.signal.aborted) {
          this.abortController.abort();
        } else {
          const forwardAbort = () => {
            this.abortController?.abort();
          };
          this.options.signal.addEventListener('abort', forwardAbort, { once: true });
          this.externalAbortCleanup = () => {
            this.options.signal?.removeEventListener('abort', forwardAbort);
          };
        }
      }

      const response = await fetch(this.url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(this.data),
        signal: this.abortController.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const reader = response.body?.getReader();
      const decoder = new TextDecoder();

      if (!reader) {
        throw new Error('Unable to read response stream');
      }

      this.markActivity(reject);

      let buffer = '';
      while (true) {
        const { done, value } = await reader.read();

        if (done) {
          break;
        }

        this.markActivity(reject);
        buffer += decoder.decode(value, { stream: true });

        const blocks = buffer.split(/\r?\n\r?\n/);
        buffer = blocks.pop() || '';

        for (const block of blocks) {
          if (!block.trim()) {
            continue;
          }

          const parsedBlock = parseSSEBlock(block);

          if (parsedBlock.isHeartbeat) {
            this.handleHeartbeat(reject);
          }

          if (!parsedBlock.data) {
            continue;
          }

          try {
            const message: SSEMessage = JSON.parse(parsedBlock.data);
            this.markActivity(reject);
            await this.handleMessage(message, resolve, reject);
          } catch (error) {
            console.error('Failed to parse SSE message:', error, block);
          }
        }
      }

      const rest = decoder.decode();
      if (rest) {
        buffer += rest;
      }

      const finalBlock = buffer.trim();
      if (finalBlock) {
        const parsedBlock = parseSSEBlock(finalBlock);
        if (parsedBlock.isHeartbeat) {
          this.handleHeartbeat(reject);
        }
        if (parsedBlock.data) {
          try {
            const message: SSEMessage = JSON.parse(parsedBlock.data);
            this.markActivity(reject);
            await this.handleMessage(message, resolve, reject);
          } catch (error) {
            console.error('Failed to parse trailing SSE message:', error, finalBlock);
          }
        }
      }

      this.finalizeStream(resolve, reject);
    } catch (error: any) {
      if (error?.name === 'AbortError') {
        this.clearInactivityTimer();
        this.rejectOnce(reject, error);
        return;
      }

      console.error('SSE POST request failed:', error);
      if (this.options.onError) {
        this.options.onError(error.message || 'Request failed');
      }
      this.rejectOnce(reject, error);
    } finally {
      this.clearInactivityTimer();
      this.externalAbortCleanup?.();
      this.externalAbortCleanup = null;
    }
  }

  private async handleMessage(message: SSEMessage, resolve: (value: any) => void, reject: (reason?: any) => void) {
    switch (message.type) {
      case 'progress':
        if (this.options.onProgress && message.progress !== undefined) {
          this.options.onProgress(
            message.message || '',
            message.progress,
            message.status || 'processing',
            message.word_count
          );
        }
        break;

      case 'chunk':
        if (message.content) {
          this.accumulatedContent += message.content;
          if (this.options.onChunk) {
            this.options.onChunk(message.content);
          }
        }
        break;

      case 'result':
        if (this.options.onResult && message.data) {
          this.options.onResult(message.data);
        }
        this.resultData = message.data;
        break;

      case 'error':
        if (this.options.onError) {
          this.options.onError(message.error || 'Unknown SSE error', message.code);
        }
        this.rejectOnce(reject, new Error(message.error || 'Unknown SSE error'));
        break;

      case 'done':
        if (this.options.onComplete) {
          this.options.onComplete();
        }
        if (this.resultData) {
          this.resolveOnce(resolve, this.resultData);
        } else if (this.accumulatedContent) {
          this.resolveOnce(resolve, { content: this.accumulatedContent });
        } else {
          this.resolveOnce(resolve, true);
        }
        break;
    }
  }

  abort() {
    this.clearInactivityTimer();
    this.externalAbortCleanup?.();
    this.externalAbortCleanup = null;
    if (this.abortController && !this.abortController.signal.aborted) {
      this.abortController.abort();
    }
  }

  getAccumulatedContent(): string {
    return this.accumulatedContent;
  }
}

export async function ssePost<T = any>(
  url: string,
  data: any,
  options: SSEClientOptions = {}
): Promise<T> {
  const client = new SSEPostClient(url, data, options);
  try {
    return await client.connect();
  } finally {
    client.abort();
  }
}
