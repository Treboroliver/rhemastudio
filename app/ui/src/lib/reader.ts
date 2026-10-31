export type UiNode =
  | { type: 'text'; value: string }
  | {
      type:
        | 'italic'
        | 'bold'
        | 'underline'
        | 'strikethrough'
        | 'caps'
        | 'superscript'
        | 'subscript'
        | 'wordsOfJesus'
        | 'otQuote'
        | 'note'
      children: UiNode[]
    }
  | { type: 'link'; children: UiNode[]; hint?: string }
  | { type: 'verseLink'; book: number; chapter: number; verse: number; children: UiNode[] }
  | { type: 'entryLink'; lang: 'G' | 'H'; num: number; children: UiNode[] }
  | { type: 'heading'; level: number; children: UiNode[] }
  | { type: 'strong'; lang: 'G' | 'H'; num: number; children: UiNode[] }
  | { type: 'paragraph' }
  | { type: 'lineBreak' }

/** Há texto visível (ou quebra de linha) no conteúdo? */
export function hasVisibleContent(nodes: UiNode[]): boolean {
  return nodes.some((node) => {
    if (node.type === 'text') return node.value.trim().length > 0
    if (node.type === 'paragraph' || node.type === 'lineBreak') return false
    return hasVisibleContent(node.children)
  })
}

/** Divide o conteúdo nos parágrafos de nível superior, descartando os vazios. */
export function splitParagraphs(nodes: UiNode[]): UiNode[][] {
  const groups: UiNode[][] = []
  let current: UiNode[] = []
  for (const node of nodes) {
    if (node.type === 'paragraph') {
      if (hasVisibleContent(current)) groups.push(current)
      current = []
    } else {
      current.push(node)
    }
  }
  if (hasVisibleContent(current)) groups.push(current)
  return groups
}

/** Ações de clique do conteúdo renderizado; sem uma ação, o nó aparece como texto simples. */
export interface NodeActions {
  /** Palavra com um ou mais Strong (do mais externo ao mais interno) e o elemento clicado. */
  onStrong?: (ids: string[], element: HTMLElement) => void
  onVerse?: (target: { book: number; chapter: number; verse: number }) => void
  /** Chave de verbete, como `G25`. */
  onEntry?: (id: string) => void
}

/**
 * Strong consecutivos sobre a mesma palavra chegam aninhados. Devolve todos os números e o
 * conteúdo da palavra, para renderizar um único botão por palavra.
 */
export function strongChain(node: Extract<UiNode, { type: 'strong' }>): {
  ids: string[]
  children: UiNode[]
} {
  const ids: string[] = []
  let current: Extract<UiNode, { type: 'strong' }> = node
  for (;;) {
    const id = `${current.lang}${current.num}`
    if (!ids.includes(id)) ids.push(id)
    const only = current.children.length === 1 ? current.children[0] : null
    if (!only || only.type !== 'strong') return { ids, children: current.children }
    current = only
  }
}

export interface VerseDto {
  number: number
  nodes: UiNode[]
}

export interface ChapterDto {
  module_id: string
  book: number
  chapter: number
  incomplete: boolean
  verses: VerseDto[]
}

export interface AlignedVerseDto {
  number: number
  columns: (UiNode[] | null)[]
}

export interface ChapterColumnDto {
  module_id: string
  error: ApiError | null
  incomplete: boolean
}

export interface AlignedChaptersDto {
  columns: ChapterColumnDto[]
  rows: AlignedVerseDto[]
}

export interface ApiError {
  code: string
  message: string
  book?: number
  max?: number
}

export interface BookInfo {
  number: number
  name: string
  abbreviation: string
  chapters: number
  testament: 'old' | 'new'
}

export interface RefPoint {
  book: number
  chapter: number
  verse: number
}

export interface RangeDto {
  start: RefPoint
  end: RefPoint
  label: string
  short_label: string
  whole: boolean
}

export interface RefErrorDto {
  code: string
  book?: number
  book_name?: string
  chapter?: number
  verse?: number
  max?: number
  text?: string
}

export interface SuggestionDto {
  book: number
  name: string
  abbreviation: string
  range?: RangeDto
}

export interface ResolveDto {
  ranges: RangeDto[]
  errors: RefErrorDto[]
  suggestions: SuggestionDto[]
}

/** Posição de leitura. `verse` é o versículo selecionado, quando há um. */
export interface Passage {
  book: number
  chapter: number
  verse: number | null
}

export const HISTORY_LIMIT = 100

export function samePassage(a: Passage | null, b: Passage | null): boolean {
  return !!a && !!b && a.book === b.book && a.chapter === b.chapter && a.verse === b.verse
}

type BookChapters = Pick<BookInfo, 'number' | 'chapters'>

function byNumber(books: BookChapters[], number: number): BookChapters | undefined {
  return books.find((book) => book.number === number)
}

/** Próximo capítulo, passando ao livro seguinte no fim do livro. */
export function nextChapter(position: Passage, books: BookChapters[]): Passage | null {
  const current = byNumber(books, position.book)
  if (!current) return null
  if (position.chapter < current.chapters) {
    return { book: position.book, chapter: position.chapter + 1, verse: null }
  }
  const next = byNumber(books, position.book + 1)
  return next ? { book: next.number, chapter: 1, verse: null } : null
}

/** Capítulo anterior, voltando ao último capítulo do livro anterior no início do livro. */
export function previousChapter(position: Passage, books: BookChapters[]): Passage | null {
  if (position.chapter > 1) {
    return { book: position.book, chapter: position.chapter - 1, verse: null }
  }
  const previous = byNumber(books, position.book - 1)
  return previous ? { book: previous.number, chapter: previous.chapters, verse: null } : null
}

/** Histórico de passagens com voltar e avançar. */
export class History {
  private entries: Passage[] = []
  private index = -1

  constructor(private readonly limit = HISTORY_LIMIT) {}

  get current(): Passage | null {
    return this.entries[this.index] ?? null
  }

  get size(): number {
    return this.entries.length
  }

  get canBack(): boolean {
    return this.index > 0
  }

  get canForward(): boolean {
    return this.index >= 0 && this.index < this.entries.length - 1
  }

  push(passage: Passage): void {
    if (samePassage(this.current, passage)) return
    this.entries = this.entries.slice(0, this.index + 1)
    this.entries.push({ ...passage })
    if (this.entries.length > this.limit) this.entries.shift()
    this.index = this.entries.length - 1
  }

  /** Atualiza o versículo da posição atual sem criar nova entrada. */
  replaceCurrent(passage: Passage): void {
    if (this.index >= 0) this.entries[this.index] = { ...passage }
  }

  back(): Passage | null {
    if (!this.canBack) return null
    this.index -= 1
    return this.current
  }

  forward(): Passage | null {
    if (!this.canForward) return null
    this.index += 1
    return this.current
  }
}

/** Mostra o intervalo com travessão entre números, como em "João 3:16–18". */
export function prettyRange(label: string): string {
  return label.replace(/(\d)-(\d)/g, '$1–$2')
}

/** Converte uma faixa em passagem. Capítulos inteiros não selecionam versículo. */
export function passageFromRange(range: RangeDto): Passage {
  return {
    book: range.start.book,
    chapter: range.start.chapter,
    verse: range.whole ? null : range.start.verse
  }
}

export function asApiError(cause: unknown): ApiError {
  if (cause && typeof cause === 'object' && 'code' in cause && typeof cause.code === 'string') {
    const error = cause as ApiError
    return { ...error, message: typeof error.message === 'string' ? error.message : '' }
  }
  const message = typeof cause === 'string' ? cause : cause instanceof Error ? cause.message : ''
  return { code: 'internal', message }
}
