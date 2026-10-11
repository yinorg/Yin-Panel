import type { Language } from '../../store/modules/app/helper'

export const defautSwatchesBackground = [
  '#00000000',
  '#000000',
  '#ffffff',
  '#18A058',
  '#2080F0',
  '#F0A020',
  'rgba(208, 48, 80, 1)',
  '#C418D1FF',
]

export const languageOptions: { label: string; key: Language; value: Language }[] = [
  { label: 'Follow browser', key: 'auto', value: 'auto' },
  { label: 'English', key: 'en-US', value: 'en-US' },
  { label: '简体中文', key: 'zh-CN', value: 'zh-CN' },
  { label: '繁體中文', key: 'zh-TW', value: 'zh-TW' },
  { label: '日本語', key: 'ja-JP', value: 'ja-JP' },
  { label: '한국어', key: 'ko-KR', value: 'ko-KR' },
  { label: 'Deutsch', key: 'de-DE', value: 'de-DE' },
  { label: 'Français', key: 'fr-FR', value: 'fr-FR' },
  { label: 'Español', key: 'es-ES', value: 'es-ES' },
  { label: 'Português (Brasil)', key: 'pt-BR', value: 'pt-BR' },
  { label: 'Русский', key: 'ru-RU', value: 'ru-RU' },
  { label: 'Bahasa Indonesia', key: 'id-ID', value: 'id-ID' },
  { label: 'Türkçe', key: 'tr-TR', value: 'tr-TR' },
  { label: 'Polski', key: 'pl-PL', value: 'pl-PL' },
  { label: 'Italiano', key: 'it-IT', value: 'it-IT' },
  { label: 'Nederlands', key: 'nl-NL', value: 'nl-NL' },
  { label: 'हिन्दी', key: 'hi-IN', value: 'hi-IN' },
  { label: 'ไทย', key: 'th-TH', value: 'th-TH' },
]
