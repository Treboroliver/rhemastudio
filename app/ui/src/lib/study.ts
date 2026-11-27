import { invoke } from '@tauri-apps/api/core'
import type { UiNode } from './reader'

export interface LexiconEntryDto {
  key: string
  title: string
  original_word: string | null
  module_id: string
  module_title: string
  degraded?: boolean
  nodes: UiNode[]
}

export type CommentaryScope = 'verse' | 'range' | 'chapter' | 'chapterIntro' | 'bookIntro'

export interface CommentaryHeaderDto {
  topic_id: number
  scope: CommentaryScope
  label: string
  chapter: number
  from_verse: number
  to_verse: number
  size_bytes: number | null
  content_type: string
  supported: boolean
}

export interface CommentaryModuleDto {
  module_id: string
  module_title: string
  error: { code: string; message: string } | null
  entries: CommentaryHeaderDto[]
}

export interface CommentaryIndexDto {
  book: number
  chapter: number
  verse: number
  modules: CommentaryModuleDto[]
}

export interface CommentaryPageDto {
  module_id: string
  topic_id: number
  offset: number
  next_offset: number
  has_more: boolean
  degraded?: boolean
  nodes: UiNode[]
}

export interface LexIndexStatusDto {
  state: 'ready' | 'rebuild_needed' | 'building' | 'empty' | 'error'
  stage: string | null
  module_index: number
  module_count: number
  percent: number
  module_id: string | null
  entries_processed: number
  message: string | null
}

export interface LexProgressEvent {
  state: string
  stage: string
  module_index: number
  module_count: number
  percent: number
  module_id: string | null
  entries_processed: number
}

export interface LexSearchHitDto {
  module_id: string
  module_title: string
  kind: string
  key: string
  topic_id: number | null
  field: string
  score: number
  provenance: string | null
  dice: number | null
}

export interface LexSearchGroupDto {
  label: string
  module_id: string
  kind: string
  is_suggestions: boolean
  is_linked: boolean
  hits: LexSearchHitDto[]
}

export interface LexSearchDto {
  query: string
  total: number
  shown: number
  interpretations: string[]
  groups: LexSearchGroupDto[]
  notes: string[]
}

export interface LexEntryRequest {
  moduleId: string
  key: string
  topicId?: number | null
  seq: number
}

export interface LexRelatedItemDto {
  module_id: string
  module_title: string
  kind: string
  key: string
  topic_id: number | null
  title: string
  reason: string
  error: string | null
}

export interface LexRelatedDto {
  primary: LexiconEntryDto
  related: LexRelatedItemDto[]
}

export interface OccurrenceBibleDto {
  module_id: string
  title: string
  abbreviation: string | null
  lang: string
  total_verses: number
  total_occurrences: number
}

export interface BookOccurrenceCount {
  book: number
  verses: number
  occurrences: number
}

export interface VerseOccurrence {
  book: number
  chapter: number
  verse: number
  n: number
}

export interface StrongOccurrencesDto {
  strong: string
  selected_module_id: string | null
  bibles: OccurrenceBibleDto[]
  total_verses: number
  total_occurrences: number
  by_book: BookOccurrenceCount[]
  page: number
  page_size: number
  total_pages: number
  items: VerseOccurrence[]
}

export interface TranslationCount {
  word: string
  n: number
  percent: number
}

export interface StrongTranslationsDto {
  strong: string
  module_id: string | null
  total: number
  items: TranslationCount[]
}

export const strongOccurrences = (
  strong: string,
  moduleId?: string | null,
  page?: number,
  pageSize?: number
) =>
  invoke<StrongOccurrencesDto>('strong_occurrences', {
    strong,
    moduleId: moduleId ?? null,
    page: page ?? 1,
    pageSize: pageSize ?? 50
  })

export const strongTranslations = (
  strong: string,
  moduleId?: string | null,
  limit?: number
) =>
  invoke<StrongTranslationsDto>('strong_translations', {
    strong,
    moduleId: moduleId ?? null,
    limit: limit ?? 15
  })

export interface MorphDecodeDto {
  code: string
  found: boolean
  explanation: string
  module_id: string | null
  module_title: string | null
}

export const morphDecode = (code: string, lang?: string | null) =>
  invoke<MorphDecodeDto>('morph_decode', { code, lang: lang ?? null })

export const lookupStrong = (strongId: string) =>
  invoke<LexiconEntryDto>('lookup_strong', { strongId })

export const lexEntry = (moduleId: string, key: string, topicId?: number | null) =>
  invoke<LexiconEntryDto>('lex_entry', { moduleId, key, topicId: topicId ?? null })

export const lexRelated = (params: {
  strongId?: string | null
  moduleId?: string | null
  key?: string | null
  topicId?: number | null
}) =>
  invoke<LexRelatedDto>('lex_related', {
    strongId: params.strongId ?? null,
    moduleId: params.moduleId ?? null,
    key: params.key ?? null,
    topicId: params.topicId ?? null
  })

export const lexSearch = (query: string, limit?: number) =>
  invoke<LexSearchDto>('lex_search', { query, limit })

export const lexIndexStatus = () =>
  invoke<LexIndexStatusDto>('lex_index_status')

export const lexIndexBuild = () =>
  invoke<LexIndexStatusDto>('lex_index_build')

export function formatLexField(
  t: (key: string, values?: Record<string, string | number>) => string,
  field: string
): string {
  const known = [
    'key',
    'subentrada',
    'dominio',
    'ligacao',
    'lemma',
    'lemma_prefixo',
    'translit',
    'key_norm',
    'refs',
    'reverse_index',
    'body'
  ]
  return known.includes(field) ? t(`lexField_${field}`) : field
}

export const getCommentaryIndex = (book: number, chapter: number, verse: number) =>
  invoke<CommentaryIndexDto>('get_commentary_index', { book, chapter, verse })

export const getCommentaryEntry = (moduleId: string, topicId: number, offset: number) =>
  invoke<CommentaryPageDto>('get_commentary_entry', { moduleId, topicId, offset })

/** Uma palavra clicada no texto bíblico: um ou mais números de Strong, do mais externo ao mais interno. */
export interface StrongRequest {
  ids: string[]
  morph?: string | null
  seq: number
}

export type StudyTab = 'lexicon' | 'commentary' | 'sermon'

/** Atraso para seguir o versículo selecionado sem consultar a cada tecla de seta. */
export const COMMENTARY_FOLLOW_DELAY_MS = 150

export function formatSize(bytes: number | null): string {
  if (bytes === null) return ''
  if (bytes < 1024) return `${bytes} B`
  return `${Math.round(bytes / 1024)} KB`
}
