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
  bible_font_size: number
  first_use_completed: boolean
  last_passage?: { book: number; chapter: number; verse?: number | null } | null
}

export function isReady(module: ModuleInfo): boolean {
  return module.state === 'Ready'
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
