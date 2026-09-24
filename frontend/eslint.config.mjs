import antfu from '@antfu/eslint-config'
import process from 'node:process'

export default antfu({
  lessOpinionated: true,
  stylistic: false,
  perfectionist: false,
  imports: false,
  unicorn: false,
  node: false,
  jsdoc: false,
  regexp: false,
  e18e: false,
  jsonc: false,
  ignores: ['docker-compose', 'kubernetes', '/service'],
  rules: {
    'no-console': process.env.NODE_ENV === 'production' ? 'error' : 'off',
    'no-alert': 'off',
    'no-empty': ['error', { allowEmptyCatch: true }],
    'no-useless-return': 'off',
    'no-control-regex': 'off',
    'ts/no-use-before-define': 'off',
    'ts/no-duplicate-enum-values': 'off',
    'unused-imports/no-unused-vars': ['error', { args: 'none', caughtErrors: 'none' }],
    'vue/html-indent': 'off',
    'vue/custom-event-name-casing': 'off',
    'vue/define-macros-order': 'off',
    'vue/no-unused-refs': 'off',
    'vue/space-infix-ops': 'off',
    'vue/block-order': 'off',
  },
})
