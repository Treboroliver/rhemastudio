export type ModuleKind = 'Bible' | 'Commentary' | 'Dictionary' | 'Book' | 'Devotional' | 'Unknown'
export type ModuleState =
  | 'Ready'
  | 'Protected'
  | { UnsupportedFormat: { message: string } }
  | { Corrupted: { message: string } }

export interface ModuleInfo {
  id: string
  path: string
  kind: ModuleKind
  state: ModuleState
  title: string
  abbreviation: string | null
  language: string | null
  language_name: string | null
  description: string | null
  version: string | null
  size: number
  has_strong: boolean
}

export interface AppSettings {
  schema_version: number
  module_folders: string[]
  default_bible_id: string | null
  selected_bible_ids: string[]
  interface_language: 'pt-BR' | 'en'
  theme: 'dark' | 'light' | 'auto'
  show_strong_numbers: boolean
  highlight_words_of_jesus: boolean
  bible_font_size: number
  first_use_completed: boolean
  last_passage?: { book: number; chapter: number; verse?: number | null } | null
  /** Léxicos em ordem de preferência; `null` usa o padrão (português primeiro). */
  lexicon_module_ids?: string[] | null
  /** Comentários ativos; `null` ativa todos os comentários prontos. */
  commentary_module_ids?: string[] | null
  study_panel_open?: boolean
  study_panel_expanded?: boolean
  show_cantillation?: boolean
  greek_font?: string
  hebrew_font?: string
}

export function isReady(module: ModuleInfo): boolean {
  return module.state === 'Ready'
}

export function readyCommentaries(modules: ModuleInfo[]): ModuleInfo[] {
  return modules.filter((module) => module.kind === 'Commentary' && isReady(module))
}

/** Comentários ativos: a lista salva sem o que saiu da biblioteca, ou todos os prontos. */
export function activeCommentaryIds(
  saved: string[] | null | undefined,
  modules: ModuleInfo[]
): string[] {
  const ready = readyCommentaries(modules).map((module) => module.id)
  return saved ? saved.filter((id) => ready.includes(id)) : ready
}

export function stateMessage(module: ModuleInfo): 'protected' | 'unsupported' | 'corrupted' | null {
  if (module.state === 'Protected') return 'protected'
  if (typeof module.state !== 'string' && 'UnsupportedFormat' in module.state) return 'unsupported'
  if (typeof module.state !== 'string' && 'Corrupted' in module.state) return 'corrupted'
  return null
}

export function groupFor(kind: ModuleKind): 'bibles' | 'commentaries' | 'dictionaries' | 'books' | null {
  if (kind === 'Bible') return 'bibles'
  if (kind === 'Commentary') return 'commentaries'
  if (kind === 'Dictionary') return 'dictionaries'
  return 'books'
}
