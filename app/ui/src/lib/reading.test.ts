import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ptBr from '../i18n/pt-BR.json'
import en from '../i18n/en.json'
import BiblePane from './BiblePane.svelte'
import CommandBar from './CommandBar.svelte'
import NodeView from './NodeView.svelte'
import type { ModuleInfo } from './modules'
import type { ChapterDto, ResolveDto, UiNode } from './reader'

const table = ptBr as Record<string, string>
const t = (key: string, values: Record<string, string | number> = {}) =>
  (table[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))

afterEach(cleanup)

const text = (value: string): UiNode => ({ type: 'text', value })

describe('i18n', () => {
  it('pt-BR e en têm as mesmas chaves', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ptBr).sort())
  })
})

describe('NodeView', () => {
  it('renderiza cada tipo de nó sem HTML injetado', () => {
    const nodes: UiNode[] = [
      { type: 'italic', children: [text('adicionada')] },
      { type: 'wordsOfJesus', children: [text('fala')] },
      { type: 'heading', level: 1, children: [text('Título de exemplo')] },
      { type: 'strong', lang: 'G', num: 25, children: [text('palavra')] },
      { type: 'text', value: '<b>não é marcação</b>' },
      { type: 'lineBreak' },
      { type: 'paragraph' }
    ]
    const { container } = render(NodeView, { nodes, t })
    expect(container.querySelector('em.node-italic')?.textContent).toBe('adicionada')
    expect(container.querySelector('.words-of-jesus')?.textContent).toBe('fala')
    expect(container.querySelector('.section-title')?.textContent).toBe('Título de exemplo')
    expect(container.querySelector('.strong-word')?.getAttribute('data-strong')).toBe('G25')
    expect(container.querySelector('b')).toBeNull()
    expect(container.textContent).toContain('<b>não é marcação</b>')
    expect(container.querySelector('br')).not.toBeNull()
  })

  it('nota de rodapé abre ao clicar, fecha com Esc', async () => {
    const nodes: UiNode[] = [{ type: 'note', children: [text('Nota de exemplo')] }]
    render(NodeView, { nodes, t })
    expect(screen.queryByText('Nota de exemplo')).toBeNull()
    const marker = screen.getByRole('button', { name: 'Nota de rodapé' })
    await fireEvent.click(marker)
    expect(screen.getByText('Nota de exemplo')).toBeTruthy()
    await fireEvent.keyDown(marker, { key: 'Escape' })
    expect(screen.queryByText('Nota de exemplo')).toBeNull()
    marker.focus()
    expect(await screen.findByText('Nota de exemplo')).toBeTruthy()
  })
})

const chapter: ChapterDto = {
  module_id: 'x',
  book: 43,
  chapter: 3,
  incomplete: false,
  verses: [1, 2, 3].map((number) => ({ number, nodes: [text(`Texto de exemplo ${number}`)] }))
}

const long: ChapterDto = {
  module_id: 'x',
  book: 19,
  chapter: 119,
  incomplete: false,
  verses: Array.from({ length: 176 }, (_, i) => ({
    number: i + 1,
    nodes: [text(`Texto de exemplo ${i + 1}`)]
  }))
}

function pane(overrides: Record<string, unknown> = {}) {
  const props = {
    t,
    hasActiveBible: true,
    bibleTitle: 'Bíblia de exemplo',
    title: 'João 3',
    bookName: 'João',
    chapter,
    loading: false,
    error: null,
    selectedVerse: null,
    canPrevious: true,
    canNext: true,
    onSelectVerse: vi.fn(),
    onPrevious: vi.fn(),
    onNext: vi.fn(),
    ...overrides
  }
  const view = render(BiblePane, props)
  return Object.assign(props, { rerender: view.rerender })
}

describe('BiblePane em lotes (176 versículos)', () => {
  const scrolled: string[] = []
  const original = HTMLElement.prototype.scrollIntoView
  beforeEach(() => {
    scrolled.length = 0
    HTMLElement.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this.dataset.verse ?? '')
    }
  })
  afterEach(() => {
    HTMLElement.prototype.scrollIntoView = original
  })

  it('vai direto a um versículo não montado e rola até ele', async () => {
    pane({ chapter: long, title: 'Salmos 119', selectedVerse: 150 })
    const target = await screen.findByText('Texto de exemplo 150')
    expect(target.closest('[data-verse]')?.getAttribute('aria-selected')).toBe('true')
    await vi.waitFor(() => expect(scrolled).toContain('150'))
  })

  it('muda a seleção para um versículo distante (barra de comandos ou histórico)', async () => {
    const props = pane({ chapter: long, selectedVerse: 3 })
    expect(screen.queryByText('Texto de exemplo 150')).toBeNull()
    await props.rerender({ selectedVerse: 150 })
    await vi.waitFor(() => expect(scrolled).toContain('150'))
    expect(screen.getByText('Texto de exemplo 150')).toBeTruthy()
    await props.rerender({ selectedVerse: 40 })
    await vi.waitFor(() => expect(scrolled).toContain('40'))
  })

  it('setas e End funcionam em versículos ainda não montados', async () => {
    const props = pane({ chapter: long, selectedVerse: 20 })
    const option = document.querySelector<HTMLElement>('[data-verse="20"]') as HTMLElement
    await fireEvent.keyDown(option, { key: 'ArrowDown' })
    expect(props.onSelectVerse).toHaveBeenCalledWith(21)
    await props.rerender({ selectedVerse: 21 })
    expect(document.querySelector('[data-verse="21"]')).not.toBeNull()
    await fireEvent.keyDown(document.querySelector('[data-verse="21"]') as HTMLElement, {
      key: 'End'
    })
    expect(props.onSelectVerse).toHaveBeenCalledWith(176)
    await props.rerender({ selectedVerse: 176 })
    expect(document.querySelector('[data-verse="176"]')).not.toBeNull()
    expect(document.activeElement?.getAttribute('data-verse')).toBe('176')
  })

  it('monta todo o texto para leitores de tela', async () => {
    pane({ chapter: long })
    await vi.waitFor(
      () => expect(screen.getAllByRole('option')).toHaveLength(176),
      { timeout: 5000 }
    )
    const last = screen.getAllByRole('option')[175]
    expect(last.getAttribute('aria-posinset')).toBe('176')
    expect(last.getAttribute('aria-setsize')).toBe('176')
  })

  it('avisa quando o capítulo está parcialmente ausente', () => {
    pane({ chapter: { ...chapter, incomplete: true } })
    expect(screen.getByText('Alguns versículos não existem nesta Bíblia')).toBeTruthy()
    expect(screen.getAllByRole('option')).toHaveLength(3)
  })
})

describe('BiblePane', () => {
  it('mostra título, versículos e seleciona por clique e teclado', async () => {
    const props = pane()
    expect(screen.getByRole('heading', { name: 'João 3' })).toBeTruthy()
    const options = screen.getAllByRole('option')
    expect(options).toHaveLength(3)
    await fireEvent.click(options[1])
    expect(props.onSelectVerse).toHaveBeenCalledWith(2)
    await fireEvent.keyDown(options[0], { key: 'ArrowDown' })
    expect(props.onSelectVerse).toHaveBeenCalledWith(2)
    await fireEvent.click(screen.getByRole('button', { name: 'Próximo capítulo' }))
    expect(props.onNext).toHaveBeenCalled()
  })

  it('trata os estados sem Bíblia, carregando, erro e vazio', () => {
    pane({ hasActiveBible: false, chapter: null, title: null })
    expect(screen.getByText('Selecione uma Bíblia para começar')).toBeTruthy()
    cleanup()
    pane({ loading: true })
    expect(screen.getByText('Carregando capítulo…')).toBeTruthy()
    cleanup()
    pane({
      chapter: null,
      bookName: 'Gênesis',
      error: { code: 'book_not_in_module', message: '' }
    })
    expect(screen.getByRole('alert').textContent).toBe(
      'Esta Bíblia não contém Gênesis. Escolha outra na biblioteca.'
    )
    cleanup()
    pane({ chapter: { ...chapter, verses: [{ number: 1, nodes: [] }] } })
    expect(screen.getByText('Este capítulo está vazio nesta Bíblia.')).toBeTruthy()
  })
})

const bible = { id: 'b1', title: 'Bíblia de exemplo', abbreviation: 'EXE' } as ModuleInfo
const outcome = (value: Partial<ResolveDto>): ResolveDto => ({
  ranges: [],
  errors: [],
  suggestions: [],
  ...value
})
const point = { book: 43, chapter: 3, verse: 16 }

function bar(resolve: (query: string) => Promise<ResolveDto>, selectedBibleIds = ['b1']) {
  const props = {
    t,
    bibles: [bible],
    selectedBibleIds,
    resolve: vi.fn(resolve),
    onGo: vi.fn(),
    onSelectBible: vi.fn(),
    onClose: vi.fn(),
    delay: 0
  }
  render(CommandBar, props)
  return props
}

const type = (value: string) =>
  fireEvent.input(screen.getByRole('combobox'), { target: { value } })

describe('CommandBar', () => {
  it('adiciona a Bíblia correspondente à abreviação digitada', async () => {
    const props = bar(async () => outcome({}), [])
    await type('EXE')
    await vi.waitFor(() => expect(screen.getByText('Adicionar Bíblia paralela')).toBeTruthy())
    await fireEvent.keyDown(screen.getByRole('combobox'), { key: 'Enter' })
    expect(props.onSelectBible).toHaveBeenCalledWith(bible)
  })

  it('foca o campo, interpreta e vai com Enter', async () => {
    const props = bar(async () =>
      outcome({
        ranges: [
          {
            start: point,
            end: { ...point, verse: 18 },
            label: 'João 3:16-18',
            short_label: 'Jo 3:16-18',
            whole: false
          }
        ]
      })
    )
    const input = screen.getByRole('combobox')
    await vi.waitFor(() => expect(document.activeElement).toBe(input))
    await type('jo 3:16-18')
    await vi.waitFor(() => expect(screen.getAllByText('João 3:16–18').length).toBeGreaterThan(0))
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(props.onGo).toHaveBeenCalledWith({ book: 43, chapter: 3, verse: 16 })
    expect(props.onClose).toHaveBeenCalled()
  })

  it('mostra erro em linguagem simples e não navega', async () => {
    const props = bar(async () =>
      outcome({
        errors: [{ code: 'invalid_chapter', book: 43, book_name: 'João', chapter: 30, max: 21 }]
      })
    )
    await type('jo 30')
    expect((await screen.findByRole('alert')).textContent).toBe('João tem 21 capítulos')
    await fireEvent.keyDown(screen.getByRole('combobox'), { key: 'Enter' })
    expect(props.onGo).not.toHaveBeenCalled()
  })

  it('escolhe sugestões com as setas', async () => {
    const props = bar(async () =>
      outcome({
        suggestions: [
          { book: 43, name: 'João', abbreviation: 'Jo' },
          { book: 62, name: '1 João', abbreviation: '1Jo' }
        ]
      })
    )
    await type('jo')
    const input = screen.getByRole('combobox')
    await screen.findAllByRole('option')
    await fireEvent.keyDown(input, { key: 'ArrowDown' })
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(props.onGo).toHaveBeenCalledWith({ book: 62, chapter: 1, verse: null })
  })

  it('troca a Bíblia pela abreviação e fecha com Esc', async () => {
    const props = bar(async () => outcome({}))
    const input = screen.getByRole('combobox')
    await type('exe')
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(props.onSelectBible).toHaveBeenCalledWith(bible)
    await fireEvent.keyDown(input, { key: 'Escape' })
    expect(props.onClose).toHaveBeenCalledTimes(2)
  })
})
