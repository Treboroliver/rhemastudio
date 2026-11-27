import { describe, expect, it, vi, afterEach } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import LexiconTab from './LexiconTab.svelte'
import ptBR from '../i18n/pt-BR.json'
import type { LexRelatedDto, StrongOccurrencesDto, StrongTranslationsDto } from './study'

const labels: Record<string, string> = ptBR
const t = (key: string, values?: Record<string, string | number>) => {
  let text = labels[key] ?? key
  if (values) {
    for (const [k, v] of Object.entries(values)) {
      text = text.replaceAll(`{${k}}`, String(v))
    }
  }
  return text
}

const books = [
  { number: 40, abbreviation: 'Mt' },
  { number: 43, abbreviation: 'Jo' }
]

const sampleRelated: LexRelatedDto = {
  primary: {
    key: 'G25',
    title: 'G25',
    original_word: 'αβγδ',
    module_id: 'dict-1',
    module_title: 'Léxico Sintético',
    nodes: [{ type: 'text', value: 'verbete sintetico' }]
  },
  related: []
}

const page1Dto: StrongOccurrencesDto = {
  strong: 'G25',
  selected_module_id: 'bible-pt-1',
  bibles: [
    {
      module_id: 'bible-pt-1',
      title: 'Sintética PT 1',
      abbreviation: 'SPT1',
      lang: 'por',
      total_verses: 3,
      total_occurrences: 4
    },
    {
      module_id: 'bible-pt-2',
      title: 'Sintética PT 2',
      abbreviation: 'SPT2',
      lang: 'por',
      total_verses: 2,
      total_occurrences: 2
    }
  ],
  total_verses: 3,
  total_occurrences: 4,
  by_book: [
    { book: 40, verses: 1, occurrences: 1 },
    { book: 43, verses: 2, occurrences: 3 }
  ],
  page: 1,
  page_size: 2,
  total_pages: 2,
  items: [
    { book: 40, chapter: 1, verse: 1, n: 1 },
    { book: 43, chapter: 3, verse: 16, n: 2 }
  ]
}

const page2Dto: StrongOccurrencesDto = {
  ...page1Dto,
  page: 2,
  items: [{ book: 43, chapter: 3, verse: 17, n: 1 }]
}

const bible2Dto: StrongOccurrencesDto = {
  ...page1Dto,
  selected_module_id: 'bible-pt-2',
  total_verses: 2,
  total_occurrences: 2,
  total_pages: 1,
  by_book: [{ book: 43, verses: 2, occurrences: 2 }],
  items: [{ book: 43, chapter: 3, verse: 16, n: 1 }]
}

const transDto: StrongTranslationsDto = {
  strong: 'G25',
  module_id: 'bible-pt-1',
  total: 4,
  items: [
    { word: 'luz', n: 3, percent: 75.0 },
    { word: 'brilha', n: 1, percent: 25.0 }
  ]
}

describe('Seção Ocorrências na aba Léxico (E2a)', () => {
  afterEach(() => {
    cleanup()
  })

  it('exibe seletor de Bíblia, traduções, contagem por livro, paginação e navega ao clicar', async () => {
    const onVerse = vi.fn()
    const occurrencesFn = vi.fn(async (_strong: string, moduleId?: string | null, page?: number) => {
      if (moduleId === 'bible-pt-2') return bible2Dto
      return page === 2 ? page2Dto : page1Dto
    })
    const translationsFn = vi.fn(async () => transDto)

    render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['G25'], seq: 1 },
        onVerse,
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue(sampleRelated),
        occurrencesFn,
        translationsFn
      }
    })

    // Resumo, traduções e referência com contagem (2×)
    expect(await screen.findByText('3 versículos · 4 ocorrências')).toBeTruthy()
    expect(screen.getByText('luz')).toBeTruthy()
    expect(screen.getByText('Página 1 de 2')).toBeTruthy()

    // Clicar numa referência navega levando a Bíblia selecionada
    const refBtn = screen.getByRole('button', { name: 'Jo 3:16 (2×)' })
    await fireEvent.click(refBtn)
    expect(onVerse).toHaveBeenCalledWith({
      book: 43,
      chapter: 3,
      verse: 16,
      moduleId: 'bible-pt-1'
    })

    // Paginação
    await fireEvent.click(screen.getByRole('button', { name: 'Próxima página' }))
    expect(await screen.findByText('Página 2 de 2')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Jo 3:17' })).toBeTruthy()

    // Trocar Bíblia no seletor
    const select = screen.getByLabelText('Bíblia com Strong') as HTMLSelectElement
    await fireEvent.change(select, { target: { value: 'bible-pt-2' } })
    expect(await screen.findByText('2 versículos · 2 ocorrências')).toBeTruthy()

    // Recolher seção
    const toggle = screen.getByRole('button', { name: /Ocorrências/ })
    await fireEvent.click(toggle)
    expect(toggle.getAttribute('aria-expanded')).toBe('false')
  })

  it('mostra estados: índice em construção, Strong sem ocorrências e Bíblia sem Strong', async () => {
    // 1) Índice em construção
    render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['G25'], seq: 1 },
        onVerse: vi.fn(),
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue(sampleRelated),
        occurrencesFn: vi.fn().mockRejectedValue({ code: 'lex_index_building', message: 'building' }),
        translationsFn: vi.fn()
      }
    })
    expect(await screen.findByText('Índice de ocorrências em construção…')).toBeTruthy()
    cleanup()

    // 2) Strong sem ocorrências
    render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['H999'], seq: 2 },
        onVerse: vi.fn(),
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue({
          primary: { ...sampleRelated.primary, key: 'H999', title: 'H999' },
          related: []
        }),
        occurrencesFn: vi.fn().mockResolvedValue({
          ...page1Dto,
          strong: 'H999',
          total_verses: 0,
          total_occurrences: 0,
          by_book: [],
          total_pages: 0,
          items: []
        }),
        translationsFn: vi.fn()
      }
    })
    expect(await screen.findByText('Nenhuma ocorrência de H999 nesta Bíblia.')).toBeTruthy()
    cleanup()

    // 3) Nenhuma Bíblia com Strong
    render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['G25'], seq: 3 },
        onVerse: vi.fn(),
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue(sampleRelated),
        occurrencesFn: vi.fn().mockResolvedValue({
          ...page1Dto,
          selected_module_id: null,
          bibles: [],
          total_verses: 0,
          total_occurrences: 0,
          by_book: [],
          total_pages: 0,
          items: []
        }),
        translationsFn: vi.fn()
      }
    })
    expect(await screen.findByText('Nenhuma Bíblia com números de Strong disponível no índice.')).toBeTruthy()
  })

  it('mostra cartão de morfologia decodificada quando há código e oculta quando não há', async () => {
    // 1) Com morfologia
    render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['G25'], morph: 'V-AAI-3S', seq: 10 },
        onVerse: vi.fn(),
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue(sampleRelated),
        occurrencesFn: vi.fn().mockResolvedValue(page1Dto),
        translationsFn: vi.fn().mockResolvedValue(transDto),
        morphDecodeFn: vi.fn().mockResolvedValue({
          code: 'V-AAI-3S',
          found: true,
          explanation: 'Verbo, aoristo, ativo, indicativo, 3ª pessoa, singular',
          module_id: 'robinson',
          module_title: 'Robinson PT'
        })
      }
    })
    expect(await screen.findByText('V-AAI-3S')).toBeTruthy()
    expect(await screen.findByText('Verbo, aoristo, ativo, indicativo, 3ª pessoa, singular')).toBeTruthy()
    cleanup()

    // 2) Sem morfologia (Bíblia sem tags WT): bloco não aparece
    const { container } = render(LexiconTab, {
      props: {
        t,
        books,
        request: { ids: ['G25'], seq: 11 },
        onVerse: vi.fn(),
        onManageFolders: vi.fn(),
        loadRelatedFn: vi.fn().mockResolvedValue(sampleRelated),
        occurrencesFn: vi.fn().mockResolvedValue(page1Dto),
        translationsFn: vi.fn().mockResolvedValue(transDto)
      }
    })
    expect(await screen.findByText('3 versículos · 4 ocorrências')).toBeTruthy()
    expect(container.querySelector('.lex-morph-card')).toBeNull()
  })
})
