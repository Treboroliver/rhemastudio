import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import ParallelBiblePane from './ParallelBiblePane.svelte'
import type { ModuleInfo } from './modules'
import type { AlignedChaptersDto, UiNode } from './reader'

afterEach(() => {
  cleanup()
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

const strings: Record<string, string> = {
  readingArea: 'Leitura paralela',
  previousChapter: 'Capítulo anterior',
  nextChapter: 'Próximo capítulo',
  decreaseFont: 'Diminuir fonte',
  resetFont: 'Fonte padrão',
  increaseFont: 'Aumentar fonte',
  showStrongNumbers: 'Mostrar Strong',
  addBible: 'Adicionar Bíblia',
  removeBible: 'Remover {title}',
  narrowColumnsNotice: 'Janela estreita: no máximo duas Bíblias.',
  chapterLoading: 'Carregando capítulo',
  selectBible: 'Selecione uma Bíblia',
  verseList: 'Versículos',
  verseMissing: 'Versículo ausente',
  chapterIncomplete: 'Capítulo incompleto',
  errorProtected: 'Módulo protegido: abra-o no theWord.',
  errorBookMissing: 'Esta Bíblia não contém {book}.',
  errorCorrupted: 'Bíblia corrompida.',
  errorUnsupported: 'Formato não suportado.',
  errorModuleMissing: 'Bíblia não encontrada.',
  errorChapter: 'Não foi possível abrir o capítulo.'
}
const t = (key: string, values: Record<string, string | number> = {}) =>
  (strings[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))

function moduleInfo(id: string): ModuleInfo {
  return {
    id,
    path: `C:\\synthetic\\${id}.ont`,
    kind: 'Bible',
    state: 'Ready',
    title: `Synthetic ${id}`,
    abbreviation: id,
    language: 'en',
    language_name: 'English',
    description: null,
    version: null,
    size: 1,
    has_strong: false
  }
}

const textNode = (value: string): UiNode => ({ type: 'text', value })

function aligned(
  moduleIds: string[],
  rowCount = 4,
  overrides: (UiNode[] | null)[][] = []
): AlignedChaptersDto {
  return {
    columns: moduleIds.map((module_id) => ({ module_id, error: null, incomplete: false })),
    rows: Array.from({ length: rowCount }, (_, index) => ({
      number: index + 1,
      columns: moduleIds.map((_, column) =>
        overrides[index] && column in overrides[index]
          ? overrides[index][column]
          : [textNode(`Synthetic line ${index + 1}, column ${column + 1}`)]
      )
    }))
  }
}

function props(ids: string[], rows: AlignedChaptersDto, extras: Record<string, unknown> = {}) {
  return {
    t,
    columns: ids.map(moduleInfo),
    aligned: rows,
    title: 'Synthetic chapter',
    bookName: 'Synthetic book',
    loading: false,
    error: null,
    selectedVerse: null,
    activeBibleId: ids[0] ?? null,
    canPrevious: true,
    canNext: true,
    fontSize: 20,
    showStrong: false,
    canAdd: true,
    onAdd: vi.fn(),
    onRemove: vi.fn(),
    onActivate: vi.fn(),
    onSelectVerse: vi.fn(),
    onPrevious: vi.fn(),
    onNext: vi.fn(),
    onFontSize: vi.fn(),
    onToggleStrong: vi.fn(),
    ...extras
  }
}

describe('parallel Bible pane', () => {
  it('adds and removes columns through the toolbar and each heading', async () => {
    const componentProps = props(['a', 'b'], aligned(['a', 'b']))
    render(ParallelBiblePane, { props: componentProps })
    await fireEvent.click(screen.getByRole('button', { name: 'Adicionar Bíblia' }))
    expect(componentProps.onAdd).toHaveBeenCalledOnce()
    await fireEvent.click(screen.getByRole('button', { name: 'Remover Synthetic b' }))
    expect(componentProps.onRemove).toHaveBeenCalledWith('b')
    await fireEvent.click(screen.getByRole('button', { name: 'Synthetic b' }))
    expect(componentProps.onActivate).toHaveBeenCalledWith('b')
  })

  it('disables adding when all four columns are occupied', () => {
    const ids = ['a', 'b', 'c', 'd']
    render(ParallelBiblePane, { props: props(ids, aligned(ids), { canAdd: false }) })
    expect(screen.getByRole('button', { name: 'Adicionar Bíblia' }).hasAttribute('disabled')).toBe(true)
  })

  it('shows only two columns in a narrow window and explains the limit', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 850 })
    const ids = ['a', 'b', 'c', 'd']
    const { container } = render(ParallelBiblePane, { props: props(ids, aligned(ids)) })
    expect(container.querySelectorAll('.parallel-column-header')).toHaveLength(2)
    expect(screen.getByText('Janela estreita: no máximo duas Bíblias.')).toBeTruthy()
  })

  it('keeps selection shared across columns and reserves a cell for a missing verse', async () => {
    const ids = ['a', 'b']
    const componentProps = props(ids, aligned(ids, 4, [[null, null]]), { selectedVerse: 2 })
    const { container } = render(ParallelBiblePane, { props: componentProps })
    expect(container.querySelectorAll('.parallel-row.selected')).toHaveLength(1)
    expect(container.querySelectorAll('.parallel-row.selected .parallel-cell')).toHaveLength(2)
    expect(container.querySelectorAll('.parallel-row[data-verse="1"] .missing-verse')).toHaveLength(2)
    await fireEvent.click(container.querySelector('.parallel-row[data-verse="3"]')!)
    expect(componentProps.onSelectVerse).toHaveBeenCalledWith(3)
    await fireEvent.keyDown(container.querySelector('.parallel-row[data-verse="3"]')!, { key: 'ArrowDown' })
    expect(componentProps.onSelectVerse).toHaveBeenCalledWith(4)
  })

  it('shows a protected-module error only in its own column', () => {
    const ids = ['a', 'b']
    const chapter = aligned(ids)
    chapter.columns[1].error = { code: 'module_protected', message: 'protected' }
    chapter.rows[0].columns[1] = null
    const { container } = render(ParallelBiblePane, { props: props(ids, chapter) })
    expect(screen.getByRole('alert').textContent).toBe('Módulo protegido: abra-o no theWord.')
    expect(container.querySelector('.parallel-cell:not(.missing-verse)')?.textContent).toContain('Synthetic line 1')
  })

  it('toggles Strong display and keeps font controls within the allowed range', async () => {
    const ids = ['a']
    const chapter = aligned(ids)
    chapter.rows[0].columns[0] = [{ type: 'strong', lang: 'G', num: 25, children: [textNode('Synthetic term')] }]
    const componentProps = props(ids, chapter, { fontSize: 32 })
    const { container, rerender } = render(ParallelBiblePane, { props: componentProps })
    expect(container.querySelector('.strong-word')?.classList.contains('show-strong')).toBe(false)
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Mostrar Strong' }))
    expect(componentProps.onToggleStrong).toHaveBeenCalledOnce()
    await rerender({ ...componentProps, showStrong: true })
    expect(container.querySelector('.strong-word')?.classList.contains('show-strong')).toBe(true)
    expect(screen.getByRole('button', { name: 'Aumentar fonte' }).hasAttribute('disabled')).toBe(true)
    await fireEvent.click(screen.getByRole('button', { name: 'Diminuir fonte' }))
    expect(componentProps.onFontSize).toHaveBeenCalledWith(-1)
  })

  it('mounts a heavy synthetic long chapter in 20-row batches across four columns', async () => {
    const ids = ['a', 'b', 'c', 'd']
    const longText = 'Synthetic module content '.repeat(12)
    const heavy = aligned(ids, 176)
    for (const row of heavy.rows) {
      row.columns = ids.map(() => [textNode(`${longText}${row.number}`)])
    }
    const started = performance.now()
    const componentProps = props(ids, heavy)
    const { container, rerender } = render(ParallelBiblePane, {
      props: componentProps
    })
    expect(container.querySelectorAll('.parallel-row')).toHaveLength(20)
    expect(container.querySelectorAll('.parallel-cell')).toHaveLength(20 * 4)
    await rerender({ ...componentProps, selectedVerse: 176 })
    await vi.waitFor(() => expect(container.querySelectorAll('.parallel-row')).toHaveLength(176))
    const mountedMs = performance.now() - started
    expect(container.querySelectorAll('.parallel-cell')).toHaveLength(176 * 4)
    console.info(`synthetic-long-chapter-four-columns-full-mount-ms=${mountedMs.toFixed(2)}`)
    expect(mountedMs).toBeLessThan(2000)
  })
})
