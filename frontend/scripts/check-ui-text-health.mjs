import { readFileSync, readdirSync } from 'fs'
import { resolve, join, relative, extname } from 'path'

const srcRoot = resolve(process.cwd(), 'src')
const textExtensions = new Set(['.ts', '.tsx', '.js', '.jsx'])
const skipDirs = new Set(['node_modules', 'dist', 'build', 'coverage', '.git'])
const latinMojibakePattern = /[\u00C2\u00C3\u00C5\u00C6\u00C7\u00C9\u00CB\u00D0\u00D1\u00D8\u00D9\u00DC\u00DE\u00DF\u00E0-\u00FF\u0153\u20AC]/
const quotedFragmentPattern = /'[^'\n]*'|"[^"\n]*"|`[^`\n]*`/g
const jsxTextPattern = />[^<>{}]+</g
const literalQuestionPattern = /'\?{2,}'|"\?{2,}"|`\?{2,}`/
const jsxQuestionPattern = />\s*\?{2,}\s*</

function walk(dir) {
  const entries = readdirSync(dir, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    if (skipDirs.has(entry.name)) {
      continue
    }
    const fullPath = join(dir, entry.name)
    if (entry.isDirectory()) {
      files.push(...walk(fullPath))
      continue
    }
    if (textExtensions.has(extname(entry.name).toLowerCase())) {
      files.push(fullPath)
    }
  }
  return files
}

function hasMojibakeInVisibleText(line) {
  const quotedFragments = line.match(quotedFragmentPattern) ?? []
  if (quotedFragments.some((fragment) => latinMojibakePattern.test(fragment))) {
    return true
  }
  const jsxFragments = line.match(jsxTextPattern) ?? []
  return jsxFragments.some((fragment) => latinMojibakePattern.test(fragment))
}

function detectLine(line) {
  const reasons = []
  if ([...line].some((char) => char === '\uFFFD' || (char.charCodeAt(0) >= 0x80 && char.charCodeAt(0) <= 0x9F))) {
    reasons.push('invalid-char')
  }
  if (hasMojibakeInVisibleText(line)) {
    reasons.push('mojibake')
  }
  if (literalQuestionPattern.test(line) || jsxQuestionPattern.test(line)) {
    reasons.push('question-placeholder')
  }
  return reasons
}

const findings = []
for (const filePath of walk(srcRoot)) {
  const content = readFileSync(filePath, 'utf8').replace(/\r\n/g, '\n')
  const lines = content.split('\n')
  lines.forEach((line, index) => {
    const reasons = detectLine(line)
    if (reasons.length > 0) {
      findings.push({
        file: relative(process.cwd(), filePath).replace(/\\/g, '/'),
        line: index + 1,
        reasons,
        content: line.trim(),
      })
    }
  })
}

if (findings.length > 0) {
  console.error('[validate:text] Frontend visible-text encoding validation failed:')
  for (const item of findings) {
    console.error(`- ${item.file}:${item.line} [${item.reasons.join(', ')}] ${item.content}`)
  }
  process.exit(1)
}

console.log('[validate:text] Frontend visible-text encoding validation passed.')
