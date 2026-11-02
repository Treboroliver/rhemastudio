import { invoke } from '@tauri-apps/api/core'

export const MAX_TITLE_CHARS = 200
export const MAX_BASE_TEXT_CHARS = 200
export const MAX_BLOCKS = 1000
export const MAX_BLOCK_BYTES = 200 * 1024
export const AUTOSAVE_DELAY_MS = 500
export const UNDO_WINDOW_MS = 8000
export const CLOSE_FLUSH_TIMEOUT_MS = 1000
export const DEFAULT_TITLE_KEY = 'sermonUntitled'

export interface VerseRefDto {
  book: string
  chapter: number
  verse: number
}

export type Block =
  | { type: 'heading'; id: string; level: number; text: string }
  | { type: 'text'; id: string; markdown: string }
  | { type: 'note'; id: string; markdown: string }
  | { type: 'illustration'; id: string; markdown: string }
  | {
      type: 'verse'
      id: string
      range: { start: VerseRefDto; end: VerseRefDto }
      module_id: string
      module_abbr: string
      snapshot: string
    }
  | { type: 'lexicon'; id: string; strong_id: string; module_id: string; title: string; snapshot: string }

export type EditableKind = 'heading' | 'text' | 'note' | 'illustration'
export const editableKinds: EditableKind[] = ['heading', 'text', 'note', 'illustration']

export interface SermonDto {
  id: string
  title: string
  base_text: string
  created_at: number
  updated_at: number
  archived: boolean
  blocks: Block[]
}

export interface SermonSummaryDto {
  id: string
  title: string
  base_text: string
  created_at: number
  updated_at: number
  archived: boolean
  block_count: number
}

export interface VersionInfoDto {
  id: number
  sermon_id: string
  created_at: number
  title: string
  block_count: number
}

export interface SermonApiError {
  code: string
  message: string
}

export function asSermonError(cause: unknown): SermonApiError {
  if (cause && typeof cause === 'object' && 'code' in cause) {
    const value = cause as { code?: unknown; message?: unknown }
    if (typeof value.code === 'string') {
      return { code: value.code, message: typeof value.message === 'string' ? value.message : value.code }
    }
  }
  return { code: 'unknown', message: cause instanceof Error ? cause.message : String(cause) }
}

export const sermonList = (includeArchived: boolean, query: string) =>
  invoke<SermonSummaryDto[]>('sermon_list', { includeArchived, query: query.trim() || null })
export const sermonCreate = (title: string, baseText = '') =>
  invoke<SermonDto>('sermon_create', { title, baseText })
export const sermonGet = (id: string) => invoke<SermonDto>('sermon_get', { id })
export const sermonSave = (sermon: SermonDto) => invoke<SermonDto>('sermon_save', { sermon })
export const sermonDuplicate = (id: string) => invoke<SermonDto>('sermon_duplicate', { id })
export const sermonArchive = (id: string, archived: boolean) =>
  invoke<void>('sermon_archive', { id, archived })
export const sermonDelete = (id: string) => invoke<void>('sermon_delete', { id })
export const sermonVersions = (id: string) => invoke<VersionInfoDto[]>('sermon_versions', { id })
export const sermonRestore = (id: string, versionId: number) =>
  invoke<SermonDto>('sermon_restore', { id, versionId })
export const sermonSaveExport = (id: string) => invoke<string | null>('sermon_save_export', { id })

let idCounter = 0
export function newBlockId(): string {
  const cryptoApi = globalThis.crypto
  if (cryptoApi && typeof cryptoApi.randomUUID === 'function') return cryptoApi.randomUUID()
  idCounter += 1
  return `b${Date.now().toString(36)}${idCounter.toString(36)}${Math.random().toString(36).slice(2, 8)}`
}

export function createBlock(kind: EditableKind): Block {
  const id = newBlockId()
  if (kind === 'heading') return { type: 'heading', id, level: 2, text: '' }
  return { type: kind, id, markdown: '' }
}

export function blockText(block: Block): string {
  if (block.type === 'heading') return block.text
  if (block.type === 'verse' || block.type === 'lexicon') return block.snapshot
  return block.markdown
}

export function isEditable(block: Block): block is Extract<Block, { type: EditableKind }> {
  return block.type !== 'verse' && block.type !== 'lexicon'
}

export function withText(block: Block, value: string): Block {
  if (block.type === 'heading') return { ...block, text: value }
  if (block.type === 'text' || block.type === 'note' || block.type === 'illustration') {
    return { ...block, markdown: value }
  }
  return block
}

/** Troca o tipo mantendo id e texto; só vale para blocos editáveis. */
export function convertBlock(block: Block, kind: EditableKind): Block {
  if (!isEditable(block) || block.type === kind) return block
  const value = blockText(block)
  if (kind === 'heading') return { type: 'heading', id: block.id, level: 2, text: value }
  return { type: kind, id: block.id, markdown: value }
}

export function blockBytes(block: Block): number {
  return new TextEncoder().encode(JSON.stringify(block)).length
}

export function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) return items
  const next = [...items]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved)
  return next
}

export function insertItem<T>(items: T[], index: number, item: T): T[] {
  const at = Math.max(0, Math.min(index, items.length))
  return [...items.slice(0, at), item, ...items.slice(at)]
}

export function duplicateBlock(block: Block): Block {
  return { ...(JSON.parse(JSON.stringify(block)) as Block), id: newBlockId() }
}

const bookVariants = [
  'Genesis', 'Exodus', 'Leviticus', 'Numbers', 'Deuteronomy', 'Joshua', 'Judges', 'Ruth',
  'FirstSamuel', 'SecondSamuel', 'FirstKings', 'SecondKings', 'FirstChronicles', 'SecondChronicles',
  'Ezra', 'Nehemiah', 'Esther', 'Job', 'Psalms', 'Proverbs', 'Ecclesiastes', 'SongOfSongs', 'Isaiah',
  'Jeremiah', 'Lamentations', 'Ezekiel', 'Daniel', 'Hosea', 'Joel', 'Amos', 'Obadiah', 'Jonah', 'Micah',
  'Nahum', 'Habakkuk', 'Zephaniah', 'Haggai', 'Zechariah', 'Malachi', 'Matthew', 'Mark', 'Luke', 'John',
  'Acts', 'Romans', 'FirstCorinthians', 'SecondCorinthians', 'Galatians', 'Ephesians', 'Philippians',
  'Colossians', 'FirstThessalonians', 'SecondThessalonians', 'FirstTimothy', 'SecondTimothy', 'Titus',
  'Philemon', 'Hebrews', 'James', 'FirstPeter', 'SecondPeter', 'FirstJohn', 'SecondJohn', 'ThirdJohn',
  'Jude', 'Revelation'
]

/** Referência legível ("Abr 1:2-3") a partir do intervalo salvo e da lista de livros da interface. */
export function referenceLabel(
  range: { start: VerseRefDto; end: VerseRefDto },
  books: { number: number; abbreviation: string }[]
): string {
  const name = (book: string) => {
    const number = bookVariants.indexOf(book) + 1
    return books.find((candidate) => candidate.number === number)?.abbreviation ?? book
  }
  const { start, end } = range
  const head = `${name(start.book)} ${start.chapter}:${start.verse}`
  if (start.book === end.book && start.chapter === end.chapter) {
    return start.verse === end.verse ? head : `${head}-${end.verse}`
  }
  if (start.book === end.book) return `${head} – ${end.chapter}:${end.verse}`
  return `${head} – ${name(end.book)} ${end.chapter}:${end.verse}`
}

export function formatDate(ms: number, language: string): string {
  try {
    return new Date(ms).toLocaleString(language, { dateStyle: 'medium', timeStyle: 'short' })
  } catch {
    return new Date(ms).toISOString()
  }
}

export type SaveStatus = 'saved' | 'saving' | 'error'

export interface SaveQueueOptions {
  save: () => Promise<void>
  onStatus: (status: SaveStatus) => void
  delay?: number
}

/**
 * Salvamento com debounce e nunca duas gravações ao mesmo tempo: alterações durante uma gravação
 * disparam outra ao terminar, sempre com o estado mais recente (`save` lê o estado na hora).
 * Em erro o estado continua pendente e a próxima alteração, `flush` ou nova tentativa regrava.
 */
export class SaveQueue {
  private dirty = false
  private running: Promise<void> | null = null
  private timer: ReturnType<typeof setTimeout> | undefined
  private readonly delay: number

  constructor(private readonly options: SaveQueueOptions) {
    this.delay = options.delay ?? AUTOSAVE_DELAY_MS
  }

  get pending(): boolean {
    return this.dirty || this.running !== null
  }

  touch(): void {
    this.dirty = true
    this.options.onStatus('saving')
    clearTimeout(this.timer)
    this.timer = setTimeout(() => void this.flush(), this.delay)
  }

  /** Grava já; resolve `true` se nada ficou pendente. */
  async flush(): Promise<boolean> {
    clearTimeout(this.timer)
    this.timer = undefined
    if (!this.running && this.dirty) {
      this.running = this.drain().finally(() => {
        this.running = null
      })
    }
    if (this.running) await this.running
    return !this.dirty
  }

  cancel(): void {
    clearTimeout(this.timer)
    this.timer = undefined
    this.dirty = false
  }

  private async drain(): Promise<void> {
    while (this.dirty) {
      this.dirty = false
      this.options.onStatus('saving')
      try {
        await this.options.save()
      } catch {
        this.dirty = true
        this.options.onStatus('error')
        return
      }
    }
    this.options.onStatus('saved')
  }
}
