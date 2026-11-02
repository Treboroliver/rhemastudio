import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../App.svelte'
import type { AppSettings, ModuleInfo } from './modules'
import type { Block, SermonDto } from './sermon'
import { MAX_BLOCKS } from './sermon'
import { insertBlockIntoSermon, resetSermonSession } from './sermonSession'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => undefined) }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => undefined })
}))

const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value)) as T
const bookName: Record<number, string> = { 43: 'John', 44: 'Acts' }
const books = [
  { number: 43, name: 'Livro 43', abbreviation: 'L43', chapters: 21, testament: 'new' },
  { number: 44, name: 'Livro 44', abbreviation: 'L44', chapters: 28, testament: 'new' }
]

function module(id: string, kind: ModuleInfo['kind']): ModuleInfo {
  return {
    id,
    path: `C:\\synthetic\\${id}`,
    kind,
    state: 'Ready',
    title: `Módulo ${id}`,
    abbreviation: id.toUpperCase(),
    language: 'pt',
    language_name: 'Português',
    description: null,
    version: null,
    size: 1,
    has_strong: kind === 'Bible'
  }
}

const settings: AppSettings = {
  schema_version: 4,
  module_folders: [],
  default_bible_id: 'bible',
  selected_bible_ids: ['bible'],
  interface_language: 'pt-BR',
  theme: 'dark',
  show_strong_numbers: false,
  highlight_words_of_jesus: true,
  bible_font_size: 20,
  first_use_completed: true,
  last_passage: { book: 43, chapter: 3, verse: 16 },
  lexicon_module_ids: null,
  commentary_module_ids: null,
  study_panel_open: true
}

interface Db {
  sermons: Map<string, SermonDto>
  saves: SermonDto[]
  calls: [string, Record<string, unknown>][]
  installed: string[]
  snapshotFails: string | null
  counter: number
}
let db: Db

const sermon = (id: string, title: string, blocks: Block[] = [], base_text = ''): SermonDto => ({
  id,
  title,
  base_text,
  created_at: 1000,
  updated_at: 2000,
  archived: false,
  blocks
})
const text = (id: string, markdown: string): Block => ({ type: 'text', id, markdown })
const verseBlock = (id: string, moduleId: string): Block => ({
  type: 'verse',
  id,
  range: { start: { book: 'John', chapter: 3, verse: 16 }, end: { book: 'John', chapter: 3, verse: 16 } },
  module_id: moduleId,
  module_abbr: moduleId.toUpperCase(),
  snapshot: 'Texto salvo inventado'
})

function install(modules = [module('bible', 'Bible'), module('bible-2', 'Bible'), module('dict', 'Dictionary')]) {
  db = { sermons: new Map(), saves: [], calls: [], installed: modules.map((item) => item.id), snapshotFails: null, counter: 0 }
  invokeMock.mockImplementation(async (command: string, args: Record<string, unknown> = {}) => {
    db.calls.push([command, args])
    switch (command) {
      case 'settings_get':
        return settings
      case 'settings_set':
        return args.settings
      case 'set_last_passage':
        return { ...settings, last_passage: args.passage }
      case 'library_list':
        return modules
      case 'library_default_folders':
        return []
      case 'book_names':
        return books
      case 'get_chapters':
        return {
          columns: (args.moduleIds as string[]).map((module_id) => ({ module_id, error: null, incomplete: false })),
          rows: Array.from({ length: 20 }, (_, index) => ({
            number: index + 1,
            columns: (args.moduleIds as string[]).map(() =>
              index === 15
                ? [
                    { type: 'text', value: 'Texto 16 ' },
                    { type: 'strong', lang: 'G', num: 25, children: [{ type: 'text', value: 'palavra' }] }
                  ]
                : [{ type: 'text', value: `Texto ${index + 1}` }]
            )
          }))
        }
      case 'lookup_strong':
        return {
          key: args.strongId,
          title: String(args.strongId),
          original_word: null,
          module_id: 'dict',
          module_title: 'Módulo dict',
          nodes: [{ type: 'text', value: 'Verbete inventado' }]
        }
      case 'get_commentary_index':
        return { book: args.book, chapter: args.chapter, verse: args.verse, modules: [] }
      case 'resolve_reference': {
        const match = /^(\S+)\s+(\d+):(\d+)/.exec(String(args.text))
        if (!match) return { ranges: [], errors: [{ code: 'invalid_syntax' }], suggestions: [] }
        const chapter = Number(match[2])
        if (chapter > 21) {
          return { ranges: [], errors: [{ code: 'invalid_chapter', book: 43, book_name: 'Livro 43', max: 21 }], suggestions: [] }
        }
        const point = { book: 43, chapter, verse: Number(match[3]) }
        return { ranges: [{ start: point, end: point, label: 'x', short_label: 'x', whole: false }], errors: [], suggestions: [] }
      }
      case 'get_verse_snapshot': {
        if (db.snapshotFails) throw { code: db.snapshotFails, message: 'x' }
        const moduleId = String(args.moduleId)
        if (!db.installed.includes(moduleId)) throw { code: 'module_not_found', message: 'x' }
        const range = args.range as { start: { book: string | number; chapter: number; verse: number } }
        const number = typeof range.start.book === 'number' ? range.start.book : 43
        const point = { book: bookName[number], chapter: range.start.chapter, verse: range.start.verse }
        return {
          reference_label: `L${number} ${point.chapter}:${point.verse}`,
          module_abbr: moduleId.toUpperCase(),
          text: `Texto inventado ${moduleId} ${point.verse}`,
          range: { start: point, end: point }
        }
      }
      case 'get_lexicon_snapshot':
        return { strong_id: args.strongId, module_id: 'dict', title: 'palavra', text: 'Definição inventada' }
      case 'sermon_list':
        return [...db.sermons.values()].map((item) => ({ ...item, blocks: undefined, block_count: item.blocks.length }))
      case 'sermon_create': {
        db.counter += 1
        const created = sermon(`new-${db.counter}`, String(args.title))
        db.sermons.set(created.id, created)
        return clone(created)
      }
      case 'sermon_get': {
        const found = db.sermons.get(String(args.id))
        if (!found) throw { code: 'sermon_not_found', message: 'x' }
        return clone(found)
      }
      case 'sermon_save': {
        const payload = clone(args.sermon as SermonDto)
        db.saves.push(payload)
        db.sermons.set(payload.id, payload)
        return clone(payload)
      }
      default:
        throw { code: 'internal', message: command }
    }
  })
}

async function mountApp() {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  render(App)
  await screen.findByRole('listbox', { name: 'Versículos' })
}

beforeEach(() => {
  invokeMock.mockReset()
  resetSermonSession()
  install()
})
afterEach(() => {
  cleanup()
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
})

const insertButton = () => screen.getByRole('button', { name: 'Inserir no sermão', description: /Ctrl\+Enter/ })
const toast = () => document.querySelector<HTMLElement>('.sermon-toast')
const lastSave = () => db.saves[db.saves.length - 1]
const textareas = () => Array.from(document.querySelectorAll<HTMLTextAreaElement>('textarea.sermon-textarea'))

async function openEditor(title = 'Sermão A') {
  await fireEvent.click(screen.getByRole('tab', { name: 'Sermão' }))
  await fireEvent.click(await screen.findByText(title))
  await waitFor(() => expect(textareas().length).toBeGreaterThan(0))
}

const seed = (blocks: Block[] = [text('a', 'um'), text('b', 'dois'), text('c', 'três')], base = '') =>
  db.sermons.set('s1', sermon('s1', 'Sermão A', blocks, base))

describe('inserir versículo no sermão', () => {
  it('sem sermão ativo abre "Em qual sermão?", com foco no primeiro item, e insere no escolhido', async () => {
    seed([text('a', 'um')])
    await mountApp()
    await fireEvent.click(insertButton())
    const dialog = await screen.findByRole('dialog', { name: 'Em qual sermão?' })
    const first = within(dialog).getByRole('button', { name: 'Sermão A' })
    await waitFor(() => expect(document.activeElement).toBe(first))
    await fireEvent.click(first)
    expect(await screen.findByText('Versículo inserido em «Sermão A»')).toBeTruthy()
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(lastSave().blocks.map((block) => block.type)).toEqual(['text', 'verse'])
    expect(lastSave().base_text).toBe('L43 3:16')
    const verse = lastSave().blocks[1]
    expect(verse).toMatchObject({ module_id: 'bible', module_abbr: 'BIBLE', snapshot: 'Texto inventado bible 16' })
  })

  it('mostra só os 5 sermões mais recentes e oferece "Novo sermão"', async () => {
    for (let index = 1; index <= 7; index += 1) db.sermons.set(`s${index}`, sermon(`s${index}`, `Sermão ${index}`))
    await mountApp()
    await fireEvent.click(insertButton())
    const dialog = await screen.findByRole('dialog', { name: 'Em qual sermão?' })
    await waitFor(() => expect(within(dialog).getAllByRole('button')).toHaveLength(6))
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Novo sermão' }))
    await waitFor(() => expect(db.sermons.has('new-1')).toBe(true))
    await waitFor(() => expect(db.sermons.get('new-1')!.blocks).toHaveLength(1))
    expect(db.sermons.get('new-1')!.title).toBe('Sermão sem título')
  })

  it('Esc cancela a janela sem inserir', async () => {
    seed()
    await mountApp()
    await fireEvent.click(insertButton())
    const dialog = await screen.findByRole('dialog', { name: 'Em qual sermão?' })
    await fireEvent.keyDown(dialog, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(db.saves).toHaveLength(0)
  })

  it('Ctrl+Enter insere no fim do sermão ativo, não repete a pergunta e o Desfazer restaura', async () => {
    seed([text('a', 'um')])
    await mountApp()
    await fireEvent.keyDown(document.body, { key: 'Enter', ctrlKey: true })
    await fireEvent.click(await screen.findByRole('button', { name: 'Sermão A' }))
    await screen.findByText('Versículo inserido em «Sermão A»')
    await fireEvent.keyDown(document.body, { key: 'Enter', ctrlKey: true })
    await waitFor(() => expect(lastSave().blocks).toHaveLength(3))
    expect(screen.queryByRole('dialog')).toBeNull()
    await fireEvent.click(within(toast()!).getByRole('button', { name: 'Desfazer' }))
    await waitFor(() => expect(lastSave().blocks).toHaveLength(2))
    expect(lastSave().blocks.map((block) => block.type)).toEqual(['text', 'verse'])
  })

  it('o Desfazer devolve o texto base vazio', async () => {
    seed([text('a', 'um')])
    await mountApp()
    await fireEvent.click(insertButton())
    await fireEvent.click(await screen.findByRole('button', { name: 'Sermão A' }))
    await screen.findByText('Versículo inserido em «Sermão A»')
    await fireEvent.click(within(toast()!).getByRole('button', { name: 'Desfazer' }))
    await waitFor(() => expect(lastSave().blocks).toHaveLength(1))
    expect(lastSave().base_text).toBe('')
  })

  it('não preenche o texto base quando ele já existe', async () => {
    seed([], 'Já preenchido')
    await mountApp()
    await fireEvent.click(insertButton())
    await fireEvent.click(await screen.findByRole('button', { name: 'Sermão A' }))
    await screen.findByText('Versículo inserido em «Sermão A»')
    expect(lastSave().base_text).toBe('Já preenchido')
  })

  it('com o editor aberto insere depois do bloco focado, com o foco dentro do campo, sem trocar de aba, e grava sozinho', async () => {
    seed()
    await mountApp()
    await openEditor()
    textareas()[1].focus()
    await fireEvent.keyDown(textareas()[1], { key: 'Enter', ctrlKey: true })
    await screen.findByText('Versículo inserido em «Sermão A»')
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks.map((block) => block.id).slice(0, 2)).toEqual(['a', 'b'])
    expect(lastSave().blocks.map((block) => block.type)).toEqual(['text', 'text', 'verse', 'text'])
    expect(screen.getByRole('tab', { name: 'Sermão' }).getAttribute('aria-selected')).toBe('true')
    expect(document.querySelector('.sermon-snapshot')?.textContent).toBe('Texto inventado bible 16')
    expect(document.querySelectorAll('[data-block-id]')).toHaveLength(4)
  })

  it('com o editor aberto e sem bloco focado insere no fim e o Desfazer remove', async () => {
    seed()
    await mountApp()
    await openEditor()
    await fireEvent.click(insertButton())
    await screen.findByText('Versículo inserido em «Sermão A»')
    expect(Array.from(document.querySelectorAll('[data-block-id]')).pop()?.querySelector('.sermon-snapshot')).toBeTruthy()
    await fireEvent.click(within(toast()!).getByRole('button', { name: 'Desfazer' }))
    await waitFor(() => expect(document.querySelectorAll('[data-block-id]')).toHaveLength(3))
  })

  it('mostra erro simples quando a Bíblia não tem o livro', async () => {
    seed()
    await mountApp()
    db.snapshotFails = 'book_not_in_module'
    await fireEvent.click(insertButton())
    await fireEvent.click(await screen.findByRole('button', { name: 'Sermão A' }))
    expect(await screen.findByText('Esta Bíblia não contém este livro.')).toBeTruthy()
    expect(db.saves).toHaveLength(0)
  })

  it('insere o verbete aberto no léxico com o aviso e grava um bloco Léxico', async () => {
    seed([text('a', 'um')])
    await mountApp()
    await fireEvent.click(document.querySelector('[data-verse="16"] button.strong-word') as HTMLElement)
    const panel = document.getElementById('study-panel-lexicon') as HTMLElement
    await fireEvent.click(await within(panel).findByRole('button', { name: 'Inserir no sermão' }))
    await fireEvent.click(await screen.findByRole('button', { name: 'Sermão A' }))
    expect(await screen.findByText('Verbete inserido em «Sermão A»')).toBeTruthy()
    expect(lastSave().blocks[1]).toMatchObject({ type: 'lexicon', strong_id: 'G25', snapshot: 'Definição inventada' })
    expect(lastSave().base_text).toBe('')
  })
})

describe('menu "/" no editor', () => {
  async function typeSlash(value = '/') {
    const field = textareas()[0]
    field.focus()
    await fireEvent.input(field, { target: { value } })
    return field
  }

  it('abre com "/" numa linha vazia, filtra digitando e converte o bloco vazio', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const field = await typeSlash()
    const list = await screen.findByRole('listbox', { name: 'Inserir bloco' })
    expect(within(list).getAllByRole('option').map((option) => option.textContent)).toEqual([
      'Versículo…',
      'Título',
      'Nota',
      'Ilustração'
    ])
    await fireEvent.input(field, { target: { value: '/no' } })
    expect(within(await screen.findByRole('listbox', { name: 'Inserir bloco' })).getAllByRole('option')).toHaveLength(1)
    await fireEvent.keyDown(field, { key: 'Enter' })
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks).toHaveLength(1)
    expect(lastSave().blocks[0]).toMatchObject({ type: 'note', id: 'a' })
  })

  it('setas escolhem o item e Esc fecha o menu mantendo o texto', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const field = await typeSlash()
    await screen.findByRole('listbox', { name: 'Inserir bloco' })
    await fireEvent.keyDown(field, { key: 'ArrowDown' })
    expect(within(screen.getByRole('listbox', { name: 'Inserir bloco' })).getAllByRole('option')[1].getAttribute('aria-selected')).toBe('true')
    await fireEvent.keyDown(field, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('listbox', { name: 'Inserir bloco' })).toBeNull())
    expect(textareas()[0].value).toBe('/')
  })

  it('não abre fora do começo da linha', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    await typeSlash('texto /')
    expect(screen.queryByRole('listbox', { name: 'Inserir bloco' })).toBeNull()
  })

  it('escolher Título com texto na frente cria um bloco depois e remove a linha "/"', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const field = await typeSlash('antes\n/')
    await screen.findByRole('listbox', { name: 'Inserir bloco' })
    await fireEvent.keyDown(field, { key: 'ArrowDown' })
    await fireEvent.keyDown(field, { key: 'Enter' })
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks.map((block) => block.type)).toEqual(['text', 'heading'])
    expect(lastSave().blocks[0]).toMatchObject({ markdown: 'antes' })
  })

  async function openVerseBox() {
    const field = await typeSlash()
    await screen.findByRole('listbox', { name: 'Inserir bloco' })
    await fireEvent.keyDown(field, { key: 'Enter' })
    return screen.findByRole('textbox', { name: /Referência/ })
  }

  it('"Versículo…" interpreta a referência, mostra a prévia e Enter troca o bloco vazio', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const input = await openVerseBox()
    await waitFor(() => expect(document.activeElement).toBe(input))
    await fireEvent.input(input, { target: { value: 'Jo 3:16' } })
    expect(await screen.findByText('Texto inventado bible 16', {}, { timeout: 2000 })).toBeTruthy()
    await fireEvent.keyDown(input, { key: 'Enter' })
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks).toHaveLength(1)
    expect(lastSave().blocks[0]).toMatchObject({ type: 'verse', module_id: 'bible', snapshot: 'Texto inventado bible 16' })
    expect(lastSave().base_text).toBe('L43 3:16')
  })

  it('digitar "/Jo 3:16" direto abre a referência já preenchida', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    await typeSlash('/Jo 3:16')
    const input = await screen.findByRole('textbox', { name: /Referência/ })
    expect((input as HTMLInputElement).value).toBe('Jo 3:16')
    expect(await screen.findByText('Texto inventado bible 16', {}, { timeout: 2000 })).toBeTruthy()
  })

  it('mostra erro em linguagem simples e não insere com referência inválida', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const input = await openVerseBox()
    await fireEvent.input(input, { target: { value: 'Jo 99:1' } })
    expect(await screen.findByText('Livro 43 tem 21 capítulos', {}, { timeout: 2000 })).toBeTruthy()
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(db.saves.some((saved) => saved.blocks.some((block) => block.type === 'verse'))).toBe(false)
    expect(screen.getByRole('textbox', { name: /Referência/ })).toBeTruthy()
  })

  it('Tab alterna entre as Bíblias prontas e a prévia muda', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const input = await openVerseBox()
    await fireEvent.input(input, { target: { value: 'Jo 3:16' } })
    await screen.findByText('Texto inventado bible 16', {}, { timeout: 2000 })
    await fireEvent.keyDown(input, { key: 'Tab' })
    expect(await screen.findByText('Texto inventado bible-2 16', {}, { timeout: 2000 })).toBeTruthy()
    await fireEvent.keyDown(input, { key: 'Enter' })
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks[0]).toMatchObject({ module_id: 'bible-2' })
  })

  it('Esc cancela e devolve o foco ao campo', async () => {
    seed([text('a', '')])
    await mountApp()
    await openEditor()
    const input = await openVerseBox()
    await fireEvent.keyDown(input, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('textbox', { name: /Referência/ })).toBeNull())
    await waitFor(() => expect(document.activeElement).toBe(textareas()[0]))
    expect(db.saves).toHaveLength(0)
  })
})

describe('bloco Versículo', () => {
  it('Trocar tradução busca o texto da mesma faixa e mantém o bloco', async () => {
    seed([verseBlock('v', 'bible')])
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Sermão' }))
    await fireEvent.click(await screen.findByText('Sermão A'))
    await fireEvent.click(await screen.findByRole('button', { name: 'Trocar tradução' }))
    await fireEvent.click(screen.getByRole('menuitem', { name: /BIBLE-2/ }))
    await waitFor(() => expect(db.saves.length).toBeGreaterThan(0), { timeout: 3000 })
    expect(lastSave().blocks[0]).toMatchObject({
      id: 'v',
      module_id: 'bible-2',
      module_abbr: 'BIBLE-2',
      snapshot: 'Texto inventado bible-2 16'
    })
    const call = db.calls.filter(([command]) => command === 'get_verse_snapshot').pop()!
    expect(call[1].range).toMatchObject({ start: { book: 'John', chapter: 3, verse: 16 } })
  })

  it('módulo ausente mostra o texto salvo com a nota e os botões que não dependem do módulo', async () => {
    seed([verseBlock('v', 'sumiu')])
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Sermão' }))
    await fireEvent.click(await screen.findByText('Sermão A'))
    expect(await screen.findByText('Bíblia não instalada neste computador; mostrando o texto salvo.')).toBeTruthy()
    expect(screen.getByText('Texto salvo inventado')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Ir para a leitura' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Duplicar bloco 1' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Remover bloco 1' })).toBeTruthy()
  })

  it('Ir para a leitura navega para a passagem', async () => {
    seed([{ ...verseBlock('v', 'bible'), range: { start: { book: 'Acts', chapter: 2, verse: 4 }, end: { book: 'Acts', chapter: 2, verse: 4 } } } as Block])
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Sermão' }))
    await fireEvent.click(await screen.findByText('Sermão A'))
    await fireEvent.click(await screen.findByRole('button', { name: 'Ir para a leitura' }))
    await waitFor(() => {
      const passage = db.calls.filter(([command]) => command === 'set_last_passage').pop()
      expect(passage?.[1].passage).toMatchObject({ book: 44, chapter: 2, verse: 4 })
    })
  })

  it('clicar no título do bloco Léxico abre o verbete na aba Léxico', async () => {
    seed([{ type: 'lexicon', id: 'l', strong_id: 'G25', module_id: 'dict', title: 'palavra', snapshot: 'Definição inventada' }])
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Sermão' }))
    await fireEvent.click(await screen.findByText('Sermão A'))
    await fireEvent.click(await screen.findByRole('button', { name: 'G25 — palavra' }))
    await waitFor(() => expect(screen.getByRole('tab', { name: 'Léxico' }).getAttribute('aria-selected')).toBe('true'))
    expect(await screen.findByText('Verbete inventado')).toBeTruthy()
  })
})

describe('sermonSession', () => {
  it('recusa o bloco 1.001 sem gravar', async () => {
    seed(Array.from({ length: MAX_BLOCKS }, (_, index) => text(`b${index}`, 'x')))
    await expect(insertBlockIntoSermon('s1', text('extra', 'y'))).rejects.toMatchObject({ code: 'sermon_block_limit' })
    expect(db.saves).toHaveLength(0)
  })

  it('sem editor, insere no fim e o Desfazer remove só o bloco inserido', async () => {
    seed([text('a', 'um')])
    const result = await insertBlockIntoSermon('s1', text('n', 'novo'), 'Base')
    expect(lastSave().blocks.map((block) => block.id)).toEqual(['a', 'n'])
    expect(lastSave().base_text).toBe('Base')
    await result.undo()
    expect(lastSave().blocks.map((block) => block.id)).toEqual(['a'])
    expect(lastSave().base_text).toBe('')
  })
})
