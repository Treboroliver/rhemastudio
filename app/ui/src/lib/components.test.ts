import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import FirstRunWizard from './FirstRunWizard.svelte'
import LibraryTree from './LibraryTree.svelte'
import type { AppSettings, ModuleInfo, ModuleState } from './modules'

const labels: Record<string, string> = {
  stepLabel: 'Passo {current} de {total}',
  welcomeTitle: 'Bem-vindo ao RhemaStudio',
  welcomeBody: 'Vamos localizar seus módulos.',
  continue: 'Continuar',
  back: 'Voltar',
  locateTitle: 'Localizar seus módulos',
  locateBody: 'Escolha as pastas.',
  defaultFolder: 'Pasta padrão encontrada',
  chosenFolders: 'Pastas selecionadas',
  chooseFolder: 'Escolher pasta…',
  scan: 'Procurar módulos',
  scanning: 'Procurando módulos…',
  scanProgress: '{count} módulos encontrados',
  scanFailed: 'Não foi possível procurar.',
  summaryTitle: 'Sua biblioteca está pronta',
  summaryCounts: 'Encontrei {bibles} Bíblias, {commentaries} comentários e {dictionaries} dicionários.',
  chooseDefaultBible: 'Escolha sua Bíblia padrão',
  noReadyBible: 'Nenhuma Bíblia pronta.',
  noModulesHelp: 'Escolha uma pasta.',
  protectedModules: 'Módulos protegidos',
  unsupportedModules: 'Outros módulos',
  finishSetup: 'Concluir',
  library: 'Biblioteca',
  bibles: 'Bíblias',
  commentaries: 'Comentários',
  dictionaries: 'Dicionários',
  books: 'Livros',
  manageFolders: 'Gerenciar pastas',
  refresh: 'Atualizar biblioteca',
  protectedMessage: 'Módulo protegido: abra-o no theWord',
  unsupportedMessage: 'Em breve',
  corruptedMessage: 'Este módulo não pôde ser lido.',
  ready: 'Pronto',
  emptyGroup: 'Nenhum módulo encontrado',
  loading: 'Carregando biblioteca…'
}

const t = (key: string, values: Record<string, string | number> = {}) =>
  (labels[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))

afterEach(cleanup)

function moduleInfo(
  id: string,
  kind: ModuleInfo['kind'],
  state: ModuleState,
  title = id
): ModuleInfo {
  return {
    id,
    path: `C:\\modules\\${id}.twm`,
    kind,
    state,
    title,
    abbreviation: id.toUpperCase(),
    language: 'pt',
    language_name: 'Português',
    description: null,
    version: null,
    size: 100,
    has_strong: false
  }
}

describe('FirstRunWizard', () => {
  it('guides first use, summarizes synthetic modules, and lets a ready Bible be selected', async () => {
    const modules = [
      moduleInfo('bible', 'Bible', 'Ready', 'Bíblia sintética'),
      moduleInfo('commentary', 'Commentary', 'Ready'),
      moduleInfo('dictionary', 'Dictionary', 'Ready'),
      moduleInfo('locked', 'Bible', 'Protected'),
      moduleInfo('book', 'Book', { UnsupportedFormat: { message: 'unsupported' } })
    ]
    const onScan = vi.fn()
    const onFinish = vi.fn<(settings: Pick<AppSettings, 'module_folders' | 'default_bible_id'>) => void>()
    const props = {
      t,
      modules: [],
      folders: ['C:\\modules'],
      defaultFolders: ['C:\\ProgramData\\The Word'],
      progress: 0,
      scanCompleted: false,
      loading: false,
      error: null,
      defaultBibleId: null,
      onChooseFolders: vi.fn(),
      onScan,
      onFinish
    }
    const { rerender } = render(FirstRunWizard, { props })
    await fireEvent.click(screen.getByRole('button', { name: 'Continuar' }))
    expect(screen.getByRole('heading', { name: 'Localizar seus módulos' })).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Procurar módulos' }))
    expect(onScan).toHaveBeenCalledOnce()
    await rerender({ ...props, modules, scanCompleted: true, initialStep: 3 })
    expect(screen.getByRole('heading', { name: 'Sua biblioteca está pronta' })).toBeTruthy()
    expect(screen.getByText(/2 Bíblias, 1 comentários e 1 dicionários/)).toBeTruthy()
    expect(screen.getByText('Módulos protegidos')).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Concluir' }))
    expect(onFinish).toHaveBeenCalledWith({
      module_folders: ['C:\\modules'],
      default_bible_id: 'bible'
    })
  })

  it('shows helpful guidance when no ready Bible exists', async () => {
    const props = {
      t,
      modules: [],
      folders: [],
      defaultFolders: [],
      progress: 0,
      scanCompleted: true,
      loading: false,
      error: null,
      initialStep: 3,
      defaultBibleId: null,
      onChooseFolders: vi.fn(),
      onScan: vi.fn(),
      onFinish: vi.fn()
    }
    render(FirstRunWizard, { props })
    expect(screen.getByText('Nenhuma Bíblia pronta.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Escolher pasta…' })).toBeTruthy()
  })
})

describe('LibraryTree', () => {
  it('groups modules by type and exposes ready, protected, unsupported, and corrupted states', async () => {
    const onSelect = vi.fn()
    const modules = [
      moduleInfo('bible', 'Bible', 'Ready', 'Bíblia sintética'),
      moduleInfo('commentary', 'Commentary', 'Ready', 'Comentário sintético'),
      moduleInfo('dictionary', 'Dictionary', 'Ready', 'Dicionário sintético'),
      moduleInfo('book', 'Book', { UnsupportedFormat: { message: 'not supported' } }, 'Livro sintético'),
      moduleInfo('locked', 'Bible', 'Protected', 'Bíblia protegida'),
      moduleInfo('broken', 'Dictionary', { Corrupted: { message: 'damaged' } }, 'Dicionário danificado')
    ]
    render(LibraryTree, {
      props: {
        t,
        modules,
        activeBibleId: null,
        loading: false,
        onSelect,
        onManageFolders: vi.fn(),
        onRefresh: vi.fn()
      }
    })
    expect(screen.getByText('Bíblias')).toBeTruthy()
    expect(screen.getByText('Comentários')).toBeTruthy()
    expect(screen.getByText('Dicionários')).toBeTruthy()
    expect(screen.getByText('Livros')).toBeTruthy()
    expect(screen.getByText(/Módulo protegido: abra-o no theWord/)).toBeTruthy()
    expect(screen.getByText('Em breve')).toBeTruthy()
    expect(screen.getByText('Este módulo não pôde ser lido.')).toBeTruthy()

    await fireEvent.click(screen.getByRole('treeitem', { name: /Bíblia sintética/ }))
    expect(onSelect).toHaveBeenCalledWith(modules[0])
    await fireEvent.click(screen.getByRole('treeitem', { name: /Bíblia protegida/ }))
    expect(onSelect).toHaveBeenCalledTimes(1)

    const bible = screen.getByRole('treeitem', { name: /Bíblia sintética/ })
    const protectedBible = screen.getByRole('treeitem', { name: /Bíblia protegida/ })
    bible.focus()
    await fireEvent.keyDown(bible, { key: 'ArrowDown' })
    expect(document.activeElement).toBe(protectedBible)
  })
})
