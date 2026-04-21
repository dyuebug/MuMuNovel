import { readFileSync } from 'fs'
import { resolve } from 'path'

const servicesRoot = resolve(process.cwd(), 'src', 'services')
const facadePath = resolve(servicesRoot, 'api.ts')
const modularApiPath = resolve(servicesRoot, 'modularApi.ts')

function normalizeContent(content) {
  return content.replace(/\r\n/g, '\n')
}

function stripComments(content) {
  return content
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '')
}

function parseExportStatements(content) {
  const statements = []
  const normalized = stripComments(normalizeContent(content))
  const exportFromPattern = /export\s+(\*|\{[\s\S]*?\})\s+from\s+['"]([^'"]+)['"];?/g

  for (const match of normalized.matchAll(exportFromPattern)) {
    const clause = match[1]
    const source = match[2]
    if (clause === '*') {
      statements.push({ type: 'star', source, raw: match[0].trim() })
      continue
    }

    const specifiers = clause
      .slice(1, -1)
      .split(',')
      .map((item) => item.trim())
      .filter((item) => item.length > 0)
      .map((item) => {
        const isType = item.startsWith('type ')
        const normalizedItem = isType ? item.slice(5).trim() : item
        const [imported, exported] = normalizedItem.split(/\s+as\s+/)
        return {
          kind: isType ? 'type' : 'value',
          imported: imported.trim(),
          exported: (exported ?? imported).trim(),
          raw: item,
        }
      })

    statements.push({
      type: 'named',
      source,
      raw: match[0].trim(),
      specifiers,
    })
  }

  return statements
}

function hasSpecifier(statement, expected) {
  return statement.type === 'named' && statement.specifiers.some((specifier) => (
    specifier.kind === expected.kind
    && specifier.imported === expected.imported
    && specifier.exported === expected.exported
  ))
}

const facadeContent = normalizeContent(readFileSync(facadePath, 'utf8'))
const modularApiContent = normalizeContent(readFileSync(modularApiPath, 'utf8'))

const facadeStatements = parseExportStatements(facadeContent)
const modularStatements = parseExportStatements(modularApiContent)
const errors = []

if (!facadeContent.includes('@deprecated')) {
  errors.push('services/api.ts 缺少 @deprecated 兼容层说明。')
}

if (facadeStatements.length !== 2) {
  errors.push(`services/api.ts 导出语句数量不符合预期：期望 2 条，实际 ${facadeStatements.length} 条。`)
}

const facadeStarExport = facadeStatements.find((statement) => statement.type === 'star')
if (!facadeStarExport || facadeStarExport.source !== './modularApi') {
  errors.push('services/api.ts 必须通过 export * from ./modularApi 透传全部命名导出。')
}

const facadeDefaultExport = facadeStatements.find((statement) => (
  statement.type === 'named'
  && statement.source === './core/httpClient'
  && hasSpecifier(statement, { kind: 'value', imported: 'api', exported: 'default' })
))
if (!facadeDefaultExport) {
  errors.push('services/api.ts 必须保留 export { api as default } from ./core/httpClient。')
}

for (const statement of facadeStatements) {
  if (statement.source.startsWith('./modules/')) {
    errors.push(`services/api.ts 不应直接从模块层导出实现：${statement.source}`)
  }
}

const coreContractExport = modularStatements.find((statement) => statement.type === 'named' && statement.source === './core/httpClient')
if (!coreContractExport) {
  errors.push('services/modularApi.ts 缺少来自 ./core/httpClient 的核心导出。')
} else {
  const requiredCoreSpecifiers = [
    { kind: 'value', imported: 'api', exported: 'api' },
    { kind: 'value', imported: 'getAxiosErrorStatus', exported: 'getAxiosErrorStatus' },
    { kind: 'value', imported: 'silentRequestConfig', exported: 'silentRequestConfig' },
    { kind: 'type', imported: 'RequestConfigWithToastControl', exported: 'RequestConfigWithToastControl' },
  ]

  for (const expected of requiredCoreSpecifiers) {
    if (!hasSpecifier(coreContractExport, expected)) {
      errors.push(`services/modularApi.ts 缺少核心导出：${expected.exported}`)
    }
  }
}

const invalidModularSources = modularStatements
  .map((statement) => statement.source)
  .filter((source) => source !== './core/httpClient' && !source.startsWith('./modules/'))

for (const source of invalidModularSources) {
  errors.push(`services/modularApi.ts 出现了约定外的导出来源：${source}`)
}

const hasModuleExports = modularStatements.some((statement) => statement.source.startsWith('./modules/'))
if (!hasModuleExports) {
  errors.push('services/modularApi.ts 至少应聚合一个 modules/* 导出。')
}

if (errors.length > 0) {
  console.error('[validate:services] 服务层兼容门面语义校验失败：')
  for (const error of errors) {
    console.error(`- ${error}`)
  }
  process.exit(1)
}

console.log('[validate:services] 服务层兼容门面语义校验通过。')