import js from '@eslint/js';
import ts from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import svelte from 'eslint-plugin-svelte';

export default [
	js.configs.recommended,
	...svelte.configs['flat/recommended'],
	{
		files: ['**/*.ts'],
		plugins: { '@typescript-eslint': ts },
		languageOptions: { parser: tsParser },
		rules: { ...ts.configs.recommended.rules }
	},
	{
		ignores: ['.svelte-kit/', 'build/', 'dist/', 'node_modules/']
	}
];
