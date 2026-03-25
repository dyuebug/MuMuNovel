import { mkdirSync, readFileSync, writeFileSync } from 'fs'
import { resolve } from 'path'
import { gzipSync } from 'zlib'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import type { PluginOption } from 'vite'
import type { OutputBundle, OutputChunk } from 'rollup'

const packageJson = JSON.parse(
  readFileSync(resolve(__dirname, 'package.json'), 'utf-8')
)

const normalizeModuleId = (id: string) => id.split('\\').join('/')
const bundleReportEnabled = process.env.BUNDLE_REPORT === '1'
const buildOutDir = process.env.VITE_OUT_DIR?.trim() || '../backend/static'
const bundleReportDir = resolve(__dirname, buildOutDir, 'reports')

const includesAny = (value: string, patterns: readonly string[]) =>
  patterns.some((pattern) => value.includes(pattern))

const formatBytes = (size: number) => {
  if (size < 1024) {
    return `${size} B`
  }

  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(2)} kB`
  }

  return `${(size / (1024 * 1024)).toFixed(2)} MB`
}

const escapeHtml = (value: string) => value
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&#39;')

const getChunkGzipSize = (chunk: OutputChunk) =>
  gzipSync(Buffer.from(chunk.code, 'utf-8')).length

const createManualChunkName = (id: string) => {
  const normalizedId = normalizeModuleId(id)

  if (normalizedId.endsWith('/src/utils/creationPresetsCore.ts')) {
    return 'creationPresetsCore'
  }

  if (normalizedId.endsWith('/src/utils/creationPresetsStory.ts')) {
    return 'creationPresetsStory'
  }

  if (normalizedId.endsWith('/src/utils/creationPresetsQuality.ts')) {
    return 'creationPresetsQuality'
  }

  if (normalizedId.endsWith('/src/utils/creationPresetsBatch.ts')) {
    return 'creationPresetsBatch'
  }

  if (normalizedId.endsWith('/src/utils/storyCreationPrompt.ts')) {
    return 'storyCreationPrompt'
  }

  if (normalizedId.endsWith('/src/utils/storyCreationPersistence.ts')) {
    return 'storyCreationPersistence'
  }

  if (normalizedId.endsWith('/src/utils/storyCreationDraft.ts')) {
    return 'storyCreationDraft'
  }

  if (!normalizedId.includes('/node_modules/')) {
    return undefined
  }

  if (includesAny(normalizedId, [
    '/@xyflow/react/',
    '/@xyflow/system/',
    '/dagre/',
    '/graphlib/',
    '/d3-',
  ])) {
    return 'vendor-graph'
  }

  if (includesAny(normalizedId, [
    '/react/',
    '/react-dom/',
    '/react-router/',
    '/react-router-dom/',
    '/scheduler/',
    '/history/',
  ])) {
    return 'vendor-react'
  }

  if (includesAny(normalizedId, [
    '/@ant-design/icons/',
    '/@ant-design/icons-svg/',
  ])) {
    return 'vendor-antd-icons'
  }

  if (normalizedId.includes('/canvas-confetti/')) {
    return 'vendor-effects'
  }

  if (normalizedId.includes('/react-diff-viewer-continued/')) {
    return 'vendor-diff'
  }

  if (normalizedId.includes('/@dnd-kit/')) {
    return 'vendor-dnd'
  }

  if (includesAny(normalizedId, [
    '/axios/',
    '/dayjs/',
    '/zustand/',
  ])) {
    return 'vendor-utils'
  }

  if (includesAny(normalizedId, [
    '/@ant-design/cssinjs/',
  ])) {
    return 'vendor-antd-style'
  }



  return undefined
}

const createBundleReportHtml = (report: {
  generatedAt: string
  version: string
  chunks: Array<{
    fileName: string
    name: string
    isEntry: boolean
    isDynamicEntry: boolean
    size: number
    gzipSize: number
    imports: string[]
    dynamicImports: string[]
    modules: Array<{ id: string, renderedLength: number }>
  }>
}) => {
  const maxSize = Math.max(...report.chunks.map((chunk) => chunk.size), 1)
  const rows = report.chunks.map((chunk) => {
    const topModules = chunk.modules
      .slice(0, 8)
      .map((module) => `<li><code>${escapeHtml(module.id)}</code> <strong>${formatBytes(module.renderedLength)}</strong></li>`)
      .join('')

    return `
      <tr>
        <td><code>${escapeHtml(chunk.fileName)}</code></td>
        <td>${escapeHtml(chunk.name || '(anonymous)')}</td>
        <td>${chunk.isEntry ? 'entry' : chunk.isDynamicEntry ? 'dynamic' : 'shared'}</td>
        <td>${formatBytes(chunk.size)}</td>
        <td>${formatBytes(chunk.gzipSize)}</td>
        <td>
          <div class="bar-track">
            <div class="bar-fill" style="width:${((chunk.size / maxSize) * 100).toFixed(2)}%"></div>
          </div>
        </td>
        <td>
          <details>
            <summary>???? (${chunk.modules.length})</summary>
            <ol>${topModules}</ol>
          </details>
        </td>
      </tr>
    `
  }).join('')

  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <title>Bundle Report</title>
  <style>
    :root { color-scheme: light dark; }
    body { font-family: Arial, sans-serif; margin: 24px; line-height: 1.5; }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #d9d9d9; padding: 10px; vertical-align: top; }
    th { background: #f5f5f5; text-align: left; }
    code { font-family: Consolas, monospace; font-size: 12px; }
    .bar-track { width: 240px; max-width: 100%; height: 10px; background: #f0f0f0; border-radius: 999px; overflow: hidden; }
    .bar-fill { height: 100%; background: linear-gradient(90deg, #1677ff, #69b1ff); }
    ol { margin: 8px 0 0 18px; padding: 0; }
    .meta { margin-bottom: 16px; color: #666; }
  </style>
</head>
<body>
  <h1>?? Bundle ????</h1>
  <div class="meta">?????${escapeHtml(report.generatedAt)} | ???${escapeHtml(report.version)}</div>
  <table>
    <thead>
      <tr>
        <th>Chunk</th>
        <th>Name</th>
        <th>Type</th>
        <th>Size</th>
        <th>Gzip</th>
        <th>??</th>
        <th>Top Modules</th>
      </tr>
    </thead>
    <tbody>${rows}</tbody>
  </table>
</body>
</html>`
}

const bundleReportPlugin = (): PluginOption => ({
  name: 'bundle-report-plugin',
  apply: 'build',
  generateBundle(_options, bundle: OutputBundle) {
    if (!bundleReportEnabled) {
      return
    }

    const chunks = Object.values(bundle)
      .filter((item): item is OutputChunk => item.type === 'chunk')
      .map((chunk) => ({
        fileName: chunk.fileName,
        name: chunk.name,
        isEntry: chunk.isEntry,
        isDynamicEntry: chunk.isDynamicEntry,
        size: Buffer.byteLength(chunk.code, 'utf-8'),
        gzipSize: getChunkGzipSize(chunk),
        imports: [...chunk.imports].sort(),
        dynamicImports: [...chunk.dynamicImports].sort(),
        modules: Object.entries(chunk.modules)
          .map(([id, meta]) => ({
            id: normalizeModuleId(id),
            renderedLength: meta.renderedLength ?? 0,
          }))
          .sort((left, right) => right.renderedLength - left.renderedLength),
      }))
      .sort((left, right) => right.size - left.size)

    const report = {
      generatedAt: new Date().toISOString(),
      version: packageJson.version,
      chunks,
    }

    mkdirSync(bundleReportDir, { recursive: true })
    const jsonPath = resolve(bundleReportDir, 'bundle-report.json')
    const htmlPath = resolve(bundleReportDir, 'bundle-report.html')
    writeFileSync(jsonPath, JSON.stringify(report, null, 2), 'utf-8')
    writeFileSync(htmlPath, createBundleReportHtml(report), 'utf-8')
    console.log(`[bundle-report] JSON: ${jsonPath}`)
    console.log(`[bundle-report] HTML: ${htmlPath}`)
  },
})

export default defineConfig({
  plugins: [react(), bundleReportPlugin()],
  define: {
    'import.meta.env.VITE_APP_VERSION': JSON.stringify(packageJson.version),
    'import.meta.env.VITE_BUILD_TIME': JSON.stringify(
      new Date().toISOString().split('T')[0]
    ),
  },
  build: {
    outDir: buildOutDir,
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks: createManualChunkName,
      },
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
      }
    }
  }
})
