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
  seq: number
}

export const lookupStrong = (strongId: string) =>
  invoke<LexiconEntryDto>('lookup_strong', { strongId })

export const lexEntry = (moduleId: string, key: string) =>
  invoke<LexiconEntryDto>('lex_entry', { moduleId, key })

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
