import { spawn } from 'child_process'
import { resolve } from 'path'

const viteBin = resolve(process.cwd(), 'node_modules', 'vite', 'bin', 'vite.js')
const reportHtmlPath = resolve(process.cwd(), '../backend/static/reports/bundle-report.html')

const child = spawn(process.execPath, [viteBin, 'build'], {
  cwd: process.cwd(),
  stdio: 'inherit',
  env: {
    ...process.env,
    BUNDLE_REPORT: '1',
  },
})

child.on('error', (error) => {
  console.error('[build:analyze] Failed to start Vite build:', error)
  process.exit(1)
})

child.on('exit', (code) => {
  if (code === 0) {
    console.log(`[build:analyze] Report: ${reportHtmlPath}`)
  }
  process.exit(code ?? 1)
})
