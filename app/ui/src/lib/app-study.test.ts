import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../App.svelte'
import type { AppSettings, ModuleInfo } from './modules'
import type { UiNode } from './reader'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => undefined) }))

const text = (value: string): UiNode => ({ type: 'text', value })
const apiError = (code: string) => ({ code, message: code })

function module(id: string, kind: ModuleInfo['kind'], state: ModuleInfo['state'] = 'Ready'): ModuleInfo {
  return {
    id,
    path: `C:\\synthetic\\${id}`,
    kind,
    state,
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

const books = [
  { number: 43, name: 'Livro 43', abbreviation: 'L43', chapters: 21, testament: 'new' },
  { number: 44, name: 'Livro 44', abbreviation: 'L44', chapters: 28, testament: 'new' }
]

interface World {
  settings: AppSettings
  modules: ModuleInfo[]
  calls: [string, Record<string, unknown>][]
  lexicon: (key: string) => unknown
}

let world: World

function baseSettings(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
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
    study_panel_open: true,
    ...overrides
  }
}

function verseNodes(number: number): UiNode[] {
  if (number === 16) {
    return [text('Texto sintético '), { type: 'strong', lang: 'G', num: 25, children: [text('palavra')] }]
  }
  if (number === 1) {
    return [
      {
        type: 'strong',
        lang: 'G',
        num: 26,
        children: [{ type: 'strong', lang: 'G', num: 25, children: [text('dupla')] }]
      }
    ]
  }
  return [text(`Texto sintético ${number}`)]
}

function chapters(args: Record<string, unknown>) {
  const moduleIds = args.moduleIds as string[]
  return {
    columns: moduleIds.map((module_id) => ({ module_id, error: null, incomplete: false })),
    rows: Array.from({ length: 20 }, (_, index) => ({
      number: index + 1,
      columns: moduleIds.map(() => verseNodes(index + 1))
    }))
  }
}

const header = (topic_id: number, label: string) => ({
  topic_id,
  scope: 'verse',
  label,
  chapter: 3,
  from_verse: 16,
  to_verse: 16,
  size_bytes: 1024,
  content_type: 'rtf',
  supported: true
})

function install(overrides: Partial<World> = {}) {
  world = {
    settings: baseSettings(),
    modules: [
      module('bible', 'Bible'),
      module('cmt-a', 'Commentary'),
      module('cmt-b', 'Commentary'),
      module('cmt-locked', 'Commentary', 'Protected'),
      module('dict', 'Dictionary')
    ],
    calls: [],
    lexicon: (key) => ({
      key,
      title: key,
      original_word: null,
      module_id: 'dict',
      module_title: 'Módulo dict',
      nodes: [text(`Verbete sintético ${key}`)]
    }),
    ...overrides
  }
  invokeMock.mockImplementation(async (command: string, args: Record<string, unknown> = {}) => {
    world.calls.push([command, args])
    switch (command) {
      case 'settings_get':
        return world.settings
      case 'library_list':
        return world.modules
      case 'library_default_folders':
        return []
      case 'book_names':
        return books
      case 'get_chapters':
        return chapters(args)
      case 'set_last_passage':
        return { ...world.settings, last_passage: args.passage }
      case 'settings_set':
        world.settings = args.settings as AppSettings
        return world.settings
      case 'lookup_strong': {
        const result = world.lexicon(args.strongId as string)
        if (result instanceof Error) throw apiError(result.message)
        return result
      }
      case 'get_commentary_index':
        return {
          book: args.book,
          chapter: args.chapter,
          verse: args.verse,
          modules: [
            {
              module_id: 'cmt-a',
              module_title: 'Módulo cmt-a',
              error: null,
              entries: [header(1, `Versículo ${args.verse}`)]
            }
          ]
        }
      case 'get_commentary_entry':
        return {
          module_id: args.moduleId,
          topic_id: args.topicId,
          offset: 0,
          next_offset: 60,
          has_more: false,
          nodes: [
            { type: 'verseLink', book: 43, chapter: 1, verse: 2, children: [text('ir ao capítulo 1')] },
            { type: 'paragraph' }
          ]
        }
      default:
        throw apiError('internal')
    }
  })
}

const callsTo = (command: string) => world.calls.filter(([name]) => name === command)

async function mountApp() {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  render(App)
  await screen.findByRole('listbox', { name: 'Versículos' })
}

beforeEach(() => {
  invokeMock.mockReset()
  install()
})
afterEach(() => {
  cleanup()
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
})

const panelBody = (name: 'lexicon' | 'commentary') => document.getElementById(`study-panel-${name}`) as HTMLElement

describe('painel de estudo no aplicativo', () => {
  it('agrupa controles acessíveis e persiste o destaque das palavras de Jesus e o tema', async () => {
    await mountApp()

    expect(screen.getByText('+ Bíblia')).toBeTruthy()
    const header = screen.getByRole('button', { name: 'Ativar Módulo bible (Alt+1)' })
    expect(header.getAttribute('aria-pressed')).toBe('true')
    expect(header.getAttribute('title')).toBe('Módulo bible')
    expect(screen.getByRole('button', { name: 'Remover Módulo bible' })).toBeTruthy()

    await fireEvent.click(screen.getByText('Aa'))
    const settings = screen.getByRole('region', { name: 'Configurações da leitura' })
    const highlight = within(settings).getByRole('checkbox', { name: 'Destacar palavras de Jesus' })
    expect((highlight as HTMLInputElement).checked).toBe(true)
    expect(within(settings).getByRole('checkbox', { name: /Mostrar números de Strong/ })).toBeTruthy()
    const theme = within(settings).getByRole('combobox', { name: 'Tema' }) as HTMLSelectElement
    expect(theme.value).toBe('dark')

    await fireEvent.click(highlight)
    await waitFor(() => expect(world.settings.highlight_words_of_jesus).toBe(false))
    expect(document.documentElement.dataset.highlightJesus).toBe('false')

    await fireEvent.change(theme, {
      target: { value: 'light' }
    })
    await waitFor(() => expect(world.settings.theme).toBe('light'))
  })

  it('desabilita adicionar Bíblia na quarta coluna e informa o limite', async () => {
    install({
      settings: baseSettings({
        selected_bible_ids: ['bible', 'bible-2', 'bible-3', 'bible-4']
      }),
      modules: [
        module('bible', 'Bible'),
        module('bible-2', 'Bible'),
        module('bible-3', 'Bible'),
        module('bible-4', 'Bible')
      ]
    })
    await mountApp()

    const add = screen.getByText('+ Bíblia').closest('button')
    expect(add?.disabled).toBe(true)
    expect(add?.getAttribute('title')).toContain('máximo é quatro')
  })

  it('clicar numa palavra com Strong abre a aba Léxico, destaca a palavra e foca o painel', async () => {
    await mountApp()
    const word = document.querySelector('[data-verse="16"] button.strong-word') as HTMLButtonElement
    expect(word.classList.contains('show-strong')).toBe(false)
    await fireEvent.click(word)
    expect(await screen.findByText('Verbete sintético G25')).toBeTruthy()
    expect(callsTo('lookup_strong').at(-1)?.[1]).toEqual({ strongId: 'G25' })
    expect(screen.getByRole('tab', { name: 'Léxico' }).getAttribute('aria-selected')).toBe('true')
    expect(word.getAttribute('data-strong-active')).toBe('true')
    await waitFor(() => expect(document.activeElement).toBe(panelBody('lexicon')))
    await fireEvent.keyDown(panelBody('lexicon'), { key: 'Escape' })
    expect(document.activeElement).toBe(word)
  })

  it('palavra com dois Strong oferece a lista para escolher', async () => {
    await mountApp()
    await fireEvent.click(document.querySelector('[data-verse="1"] button.strong-word') as HTMLElement)
    expect(await screen.findByText('Verbete sintético G26')).toBeTruthy()
    const group = screen.getByRole('group', { name: 'Números de Strong desta palavra' })
    await fireEvent.click(within(group).getByRole('button', { name: 'G25' }))
    expect(await screen.findByText('Verbete sintético G25')).toBeTruthy()
  })

  it('sem dicionário, mostra a orientação e o botão abre o gerenciamento de pastas', async () => {
    install({ lexicon: () => new Error('no_lexicon_installed') })
    await mountApp()
    await fireEvent.click(document.querySelector('[data-verse="16"] button.strong-word') as HTMLElement)
    expect(await screen.findByText('Instale um dicionário para ver o léxico')).toBeTruthy()
    await fireEvent.click(within(panelBody('lexicon')).getByRole('button', { name: 'Gerenciar pastas' }))
    expect(await screen.findByText('Localizar seus módulos')).toBeTruthy()
  })

  it('Ctrl+B recolhe e reabre o painel e guarda o estado nas configurações', async () => {
    await mountApp()
    const aside = document.querySelector('aside.study-panel') as HTMLElement
    expect(aside.hidden).toBe(false)
    await fireEvent.keyDown(window, { key: 'b', ctrlKey: true })
    expect(aside.hidden).toBe(true)
    await waitFor(() => expect(world.settings.study_panel_open).toBe(false))
    await fireEvent.keyDown(window, { key: 'b', ctrlKey: true })
    expect(aside.hidden).toBe(false)
    await waitFor(() => expect(world.settings.study_panel_open).toBe(true))
  })

  it('restaura o painel recolhido e um clique em Strong o reabre', async () => {
    install({ settings: baseSettings({ study_panel_open: false }) })
    await mountApp()
    const aside = document.querySelector('aside.study-panel') as HTMLElement
    expect(aside.hidden).toBe(true)
    await fireEvent.click(document.querySelector('[data-verse="16"] button.strong-word') as HTMLElement)
    expect(await screen.findByText('Verbete sintético G25')).toBeTruthy()
    expect(aside.hidden).toBe(false)
    await waitFor(() => expect(world.settings.study_panel_open).toBe(true))
  })

  it('o comentário segue o versículo selecionado e ainda mostra o escopo', async () => {
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Comentário' }))
    expect(await screen.findByRole('button', { name: /Versículo 16/ })).toBeTruthy()
    expect(callsTo('get_commentary_index').at(-1)?.[1]).toEqual({ book: 43, chapter: 3, verse: 16 })
    await fireEvent.click(document.querySelector('[data-verse="17"]') as HTMLElement)
    expect(await screen.findByRole('button', { name: /Versículo 17/ })).toBeTruthy()
    expect(callsTo('get_commentary_index').at(-1)?.[1]).toEqual({ book: 43, chapter: 3, verse: 17 })
  })

  it('ativar e desativar comentários persiste a escolha e nunca lista módulos protegidos', async () => {
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Comentário' }))
    const first = (await screen.findByRole('checkbox', { name: 'Módulo cmt-a' })) as HTMLInputElement
    const second = screen.getByRole('checkbox', { name: 'Módulo cmt-b' }) as HTMLInputElement
    expect(screen.queryByRole('checkbox', { name: 'Módulo cmt-locked' })).toBeNull()
    expect([first.checked, second.checked]).toEqual([true, true])
    await fireEvent.click(second)
    await waitFor(() => expect(world.settings.commentary_module_ids).toEqual(['cmt-a']))
    await waitFor(() => expect(second.checked).toBe(false))
    await fireEvent.click(first)
    await waitFor(() => expect(world.settings.commentary_module_ids).toEqual([]))
    expect(await screen.findByText('Nenhum comentário ativo. Marque ao menos um módulo abaixo.')).toBeTruthy()
    await fireEvent.click(second)
    await waitFor(() => expect(world.settings.commentary_module_ids).toEqual(['cmt-b']))
  })

  it('restaura comentários desativados nas configurações salvas', async () => {
    install({ settings: baseSettings({ commentary_module_ids: ['cmt-b'] }) })
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Comentário' }))
    expect(((await screen.findByRole('checkbox', { name: 'Módulo cmt-a' })) as HTMLInputElement).checked).toBe(false)
    expect((screen.getByRole('checkbox', { name: 'Módulo cmt-b' }) as HTMLInputElement).checked).toBe(true)
  })

  it('link para versículo navega a leitura e preserva o histórico', async () => {
    await mountApp()
    await fireEvent.click(screen.getByRole('tab', { name: 'Comentário' }))
    await fireEvent.click(await screen.findByRole('button', { name: /Versículo 16/ }))
    await fireEvent.click(await screen.findByRole('button', { name: 'ir ao capítulo 1' }))
    await waitFor(() => expect(callsTo('get_chapters').at(-1)?.[1]).toMatchObject({ book: 43, chapter: 1 }))
    await waitFor(() =>
      expect(document.querySelector('[data-verse="2"]')?.getAttribute('aria-selected')).toBe('true')
    )
    await fireEvent.keyDown(window, { key: 'ArrowLeft', altKey: true })
    await waitFor(() => expect(callsTo('get_chapters').at(-1)?.[1]).toMatchObject({ book: 43, chapter: 3 }))
    await waitFor(() =>
      expect(document.querySelector('[data-verse="16"]')?.getAttribute('aria-selected')).toBe('true')
    )
  })

  it('o interruptor de Strong continua independente do clique', async () => {
    install({ settings: baseSettings({ show_strong_numbers: true }) })
    await mountApp()
    const word = document.querySelector('[data-verse="16"] button.strong-word') as HTMLElement
    expect(word.classList.contains('show-strong')).toBe(true)
    await fireEvent.click(word)
    expect(await screen.findByText('Verbete sintético G25')).toBeTruthy()
  })
})
