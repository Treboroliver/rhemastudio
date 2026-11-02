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

export const lookupStrong = (strongId: string) =>
  invoke<LexiconEntryDto>('lookup_strong', { strongId })

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
