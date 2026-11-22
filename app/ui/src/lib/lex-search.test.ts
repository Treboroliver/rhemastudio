import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import { tick } from 'svelte'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import CommandBar from './CommandBar.svelte'
import FirstRunWizard from './FirstRunWizard.svelte'
import LexiconTab from './LexiconTab.svelte'
import ptBR from '../i18n/pt-BR.json'
import type { LexSearchDto } from './study'

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

const sampleLexResult: LexSearchDto = {
  query: 'amor',
  total: 3,
  shown: 3,
  interpretations: ['Latin("amor")'],
  notes: [],
  groups: [
    {
      label: 'sugestões por palavra',
      module_id: 'sugestões por palavra',
      kind: 'strong',
      is_suggestions: true,
      is_linked: false,
      hits: [
        {
          module_id: 'strong.dct.twm',
          module_title: 'Strong',
          kind: 'strong',
          key: 'G26',
          topic_id: 26,
          field: 'reverse_index',
          score: 172,
          provenance: 'reverse_index (Dice 0.550)',
          dice: 0.55
        },
        {
          module_id: 'strong.dct.twm',
          module_title: 'Strong',
          kind: 'strong',
          key: 'H160',
          topic_id: 160,
          field: 'reverse_index',
          score: 159,
          provenance: 'reverse_index (Dice 0.234)',
          dice: 0.234
        }
      ]
    },
    {
      label: 'Louw-Nida',
      module_id: 'louw.dct.twm',
      kind: 'louw_nida',
      is_suggestions: false,
      is_linked: false,
      hits: [
        {
          module_id: 'louw.dct.twm',
          module_title: 'Louw-Nida',
          kind: 'louw_nida',
          key: '25.43',
          topic_id: 42,
          field: 'key',
          score: 1000,
          provenance: null,
          dice: null
        }
      ]
    }
  ]
}

describe('Busca léxica na CommandBar e na aba Léxico', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    cleanup()
    vi.useRealTimers()
  })

  it('mantém prioridade da referência bíblica na CommandBar', async () => {
    const resolve = vi.fn().mockResolvedValue({
      ranges: [{ label: 'Jo 3:16', book: 43, start_chapter: 3, start_verse: 16, end_chapter: 3, end_verse: 16 }],
      errors: [],
      suggestions: []
    })
    const lexSearch = vi.fn().mockResolvedValue(sampleLexResult)

    render(CommandBar, {
      t,
      bibles: [],
      selectedBibleIds: [],
      resolve,
      lexSearch,
      onGo: vi.fn(),
      onSelectBible: vi.fn(),
      onSelectLexicon: vi.fn(),
      onClose: vi.fn()
    })

    const input = screen.getByRole('combobox')
    await fireEvent.input(input, { target: { value: 'Jo 3:16' } })
    await vi.advanceTimersByTimeAsync(200)

    expect(resolve).toHaveBeenCalled()
    expect(lexSearch).not.toHaveBeenCalled()
  })

  it('ativa busca léxica com debounce de 120 ms, prefixo l e navegação por teclado', async () => {
    const resolve = vi.fn().mockResolvedValue({ ranges: [], errors: [{ code: 'unknown_book', text: 'amor' }], suggestions: [] })
    const lexSearch = vi.fn().mockResolvedValue(sampleLexResult)
    const onSelectLexicon = vi.fn()
    const onClose = vi.fn()

    render(CommandBar, {
      t,
      bibles: [],
      selectedBibleIds: [],
      resolve,
      lexSearch,
      onGo: vi.fn(),
      onSelectBible: vi.fn(),
      onSelectLexicon,
      onClose
    })

    const input = screen.getByRole('combobox')
    await fireEvent.input(input, { target: { value: 'l amor' } })
    await vi.advanceTimersByTimeAsync(80)
    expect(lexSearch).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(50)
    await tick()
    expect(lexSearch).toHaveBeenCalledWith('amor', 20)
    expect(resolve).not.toHaveBeenCalled()

    await fireEvent.keyDown(input, { key: 'ArrowDown' })
    await tick()
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(onSelectLexicon).toHaveBeenCalledWith(expect.objectContaining({ key: 'H160' }))
    expect(onClose).toHaveBeenCalled()
  })

  it('exibe status do índice, busca agrupada e abre entrada não-Strong na aba Léxico', async () => {
    const statusFn = vi.fn().mockResolvedValue({
      state: 'building',
      stage: 'lexicon',
      module_index: 1,
      module_count: 2,
      percent: 43,
      module_id: 'strong.dct.twm',
      entries_processed: 100,
      message: null
    })
    const buildFn = vi.fn().mockResolvedValue({
      state: 'ready',
      stage: null,
      module_index: 0,
      module_count: 0,
      percent: 100,
      module_id: null,
      entries_processed: 0,
      message: null
    })
    const searchFn = vi.fn().mockResolvedValue(sampleLexResult)
    const loadEntryFn = vi.fn().mockResolvedValue({
      key: '25.43',
      title: '25.43',
      original_word: null,
      module_id: 'louw.dct.twm',
      module_title: 'Louw-Nida',
      degraded: false,
      nodes: [{ type: 'text', value: 'Texto sintetico' }]
    })

    render(LexiconTab, {
      t,
      request: null,
      onVerse: vi.fn(),
      onManageFolders: vi.fn(),
      statusFn,
      buildFn,
      searchFn,
      loadEntryFn
    })

    await vi.advanceTimersByTimeAsync(10)
    expect(screen.getByText('Construindo 43%')).toBeTruthy()

    const searchInput = screen.getByRole('combobox')
    await fireEvent.input(searchInput, { target: { value: 'amor' } })
    await vi.advanceTimersByTimeAsync(130)

    expect(searchFn).toHaveBeenCalledWith('amor', 30)
    expect(screen.getByText('Grego')).toBeTruthy()
    expect(screen.getByText('Hebraico')).toBeTruthy()
    expect(screen.getByText('Louw-Nida')).toBeTruthy()

    await fireEvent.keyDown(searchInput, { key: 'ArrowDown' })
    await fireEvent.keyDown(searchInput, { key: 'ArrowDown' })
    await fireEvent.keyDown(searchInput, { key: 'Enter' })
    await vi.advanceTimersByTimeAsync(10)

    expect(loadEntryFn).toHaveBeenCalledWith('louw.dct.twm', '25.43', 42)
    expect(screen.getByText('Texto sintetico')).toBeTruthy()
  })

  it('exibe mensagens de erro específicas sem botão Tentar novamente quando não faz sentido', async () => {
    const loadEntryFn = vi
      .fn()
      .mockRejectedValueOnce({ code: 'entry_empty', message: 'vazio' })
      .mockRejectedValueOnce({ code: 'module_not_found', message: 'ausente' })

    const { rerender } = render(LexiconTab, {
      t,
      request: null,
      entryRequest: { moduleId: 'm1', key: 'A', topicId: 1, seq: 1 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn(),
      loadEntryFn
    })
    await vi.advanceTimersByTimeAsync(10)
    expect(screen.getByText('Este verbete está vazio neste módulo.')).toBeTruthy()
    expect(screen.queryByText('Tentar novamente')).toBeNull()

    await rerender({
      t,
      request: null,
      entryRequest: { moduleId: 'm2', key: 'B', topicId: 2, seq: 2 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn(),
      loadEntryFn
    })
    await vi.advanceTimersByTimeAsync(10)
    expect(screen.getByText('O módulo selecionado não está disponível na biblioteca.')).toBeTruthy()
    expect(screen.queryByText('Tentar novamente')).toBeNull()
  })

  it('permite remover pastas e exibe aviso de pasta inacessível no assistente', async () => {
    const onRemoveFolder = vi.fn()
    render(FirstRunWizard, {
      t,
      modules: [],
      folders: ['C:\\ModulosA', 'C:\\ModulosB'],
      defaultFolders: [],
      progress: 0,
      scanCompleted: false,
      loading: false,
      error: null,
      initialStep: 2,
      defaultBibleId: null,
      warnings: ['Pasta inacessível ignorada: C:\\ModulosB'],
      onChooseFolders: vi.fn(),
      onRemoveFolder,
      onScan: vi.fn(),
      onFinish: vi.fn()
    })

    expect(screen.getByText('Pasta inacessível ignorada: C:\\ModulosB')).toBeTruthy()
    const removeButtons = screen.getAllByText('Remover')
    expect(removeButtons).toHaveLength(2)
    await fireEvent.click(removeButtons[1])
    expect(onRemoveFolder).toHaveBeenCalledWith('C:\\ModulosB')
  })

  it('exibe controle Outros dicionários (N), lembra escolha e oculta quando N=0', async () => {
    const loadRelatedFn = vi
      .fn()
      .mockResolvedValueOnce({
        primary: {
          key: 'G25',
          title: 'G25',
          original_word: 'ἀγαπάω',
          module_id: 'strong1',
          module_title: 'Strong Principal',
          degraded: false,
          nodes: [{ type: 'text', value: 'Verbete principal' }]
        },
        related: [
          {
            module_id: 'louw1',
            module_title: 'Louw-Nida',
            kind: 'louw_nida',
            key: '25.43',
            topic_id: 43,
            title: '25.43',
            reason: 'ligação do Louw-Nida',
            error: null
          }
        ]
      })
      .mockResolvedValueOnce({
        primary: {
          key: 'G3056',
          title: 'G3056',
          original_word: 'λόγος',
          module_id: 'strong1',
          module_title: 'Strong Principal',
          degraded: false,
          nodes: [{ type: 'text', value: 'Verbete G3056' }]
        },
        related: []
      })
    const loadEntryFn = vi.fn().mockResolvedValue({
      key: '25.43',
      title: '25.43',
      original_word: null,
      module_id: 'louw1',
      module_title: 'Louw-Nida',
      degraded: false,
      nodes: [{ type: 'text', value: 'Verbete Louw-Nida' }]
    })

    const { rerender } = render(LexiconTab, {
      t,
      request: { ids: ['G25'], seq: 1 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn(),
      loadRelatedFn,
      loadEntryFn
    })
    await vi.advanceTimersByTimeAsync(10)

    const toggle = screen.getByText(/Outros dicionários \(1\)/)
    expect(toggle).toBeTruthy()
    await fireEvent.click(toggle)
    expect(screen.getByText('ligação do Louw-Nida')).toBeTruthy()
    await fireEvent.click(screen.getByText(/Louw-Nida · 25\.43/))
    await vi.advanceTimersByTimeAsync(10)

    expect(loadEntryFn).toHaveBeenCalledWith('louw1', '25.43', 43)
    expect(screen.getByText('Verbete Louw-Nida')).toBeTruthy()

    await rerender({
      t,
      request: { ids: ['G3056'], seq: 2 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn(),
      loadRelatedFn,
      loadEntryFn
    })
    await vi.advanceTimersByTimeAsync(10)
    expect(screen.queryByText(/Outros dicionários/)).toBeNull()
  })
})
