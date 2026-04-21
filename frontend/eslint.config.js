import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist', 'test-results', 'playwright-report', '.playwright']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs['recommended-latest'],
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    ignores: ['src/services/api.ts'],
    rules: {
      'no-restricted-imports': ['error', {
        paths: [
          {
            name: '../services/api',
            message: '请改为从 ../services/modularApi 或对应 modules/* 导入；services/api.ts 仅保留兼容用途。',
          },
          {
            name: '../../services/api',
            message: '请改为从 ../../services/modularApi 或对应 modules/* 导入；services/api.ts 仅保留兼容用途。',
          },
          {
            name: './services/api',
            message: '请改为从 ./services/modularApi 或对应 modules/* 导入；services/api.ts 仅保留兼容用途。',
          },
          {
            name: '@/services/api',
            message: '请改为从 @/services/modularApi 或对应 modules/* 导入；services/api.ts 仅保留兼容用途。',
          },
          {
            name: 'src/services/api',
            message: '请改为从 src/services/modularApi 或对应 modules/* 导入；services/api.ts 仅保留兼容用途。',
          },
        ],
      }],
    },
  },
])