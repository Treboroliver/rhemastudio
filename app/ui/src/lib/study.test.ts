import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ptBr from '../i18n/pt-BR.json'
import en from '../i18n/en.json'
import CommentaryTab from './CommentaryTab.svelte'
import LexiconTab from './LexiconTab.svelte'
import NodeView from './NodeView.svelte'
import RichContent from './RichContent.svelte'
import StudyPanel from './StudyPanel.svelte'
import type { ModuleInfo } from './modules'
import { splitParagraphs, strongChain, type UiNode } from './reader'
import type { CommentaryIndexDto, CommentaryPageDto, LexiconEntryDto } from './study'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

const table = ptBr as Record<string, string>
const t = (key: string, values: Record<string, string | number> = {}) =>
  (table[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))

const text = (value: string): UiNode => ({ type: 'text', value })
const apiError = (code: string) => ({ code, message: code })

beforeEach(() => {
  invokeMock.mockReset()
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('i18n do painel de estudo', () => {
  it('pt-BR e en têm as mesmas chaves e nenhuma chave nova fica vazia', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ptBr).sort())
    for (const key of ['lexiconNone', 'lexiconNotFound', 'commentaryEmpty', 'commentaryMore']) {
      expect(table[key]).toBeTruthy()
      expect((en as Record<string, string>)[key]).toBeTruthy()
    }
    expect(table.lexiconNone).toBe('Instale um dicionário para ver o léxico')
    expect(table.lexiconNotFound).toBe('Este número não está no dicionário escolhido')
    expect(table.commentaryEmpty).toBe('Nenhum comentário para este versículo nos módulos ativos')
  })

  it('nenhum componente usa {@html}', async () => {
    const files: string[] = []
    const walk = async (directory: string) => {
      for (const entry of await readdir(directory, { withFileTypes: true })) {
        const path = join(directory, entry.name)
        if (entry.isDirectory()) await walk(path)
        else if (entry.name.endsWith('.svelte')) files.push(path)
      }
    }
    await walk(join(process.cwd(), 'src'))
    expect(files.length).toBeGreaterThan(5)
    for (const file of files) expect(await readFile(file, 'utf8')).not.toContain('{@html')
  })
})

describe('Strong clicável e links no NodeView', () => {
  const strong = (num: number, children: UiNode[], lang: 'G' | 'H' = 'G'): UiNode => ({
    type: 'strong',
    lang,
    num,
    children
  })

  it('clicar numa palavra com Strong chama a ação mesmo com o interruptor desligado', async () => {
    const onStrong = vi.fn()
    const { container } = render(NodeView, {
      nodes: [strong(25, [text('palavra')])],
      t,
      showStrong: false,
      actions: { onStrong }
    })
    const word = container.querySelector('button.strong-word') as HTMLButtonElement
    expect(word.tagName).toBe('BUTTON')
    expect(word.getAttribute('type')).toBe('button')
    expect(word.classList.contains('show-strong')).toBe(false)
    await fireEvent.click(word)
    expect(onStrong).toHaveBeenCalledWith(['G25'], word)
  })

  it('Strong aninhados viram um único botão com todos os números, sem repetir', async () => {
    const onStrong = vi.fn()
    const nested = strong(26, [strong(25, [strong(26, [text('palavra')])])])
    const { container } = render(NodeView, { nodes: [nested], t, actions: { onStrong } })
    const buttons = container.querySelectorAll('button.strong-word')
    expect(buttons).toHaveLength(1)
    expect(buttons[0].getAttribute('data-strong')).toBe('G26 G25')
    expect(buttons[0].textContent).toBe('palavra')
    await fireEvent.click(buttons[0])
    expect(onStrong).toHaveBeenCalledWith(['G26', 'G25'], buttons[0])
    expect(strongChain(nested as Extract<UiNode, { type: 'strong' }>).ids).toEqual(['G26', 'G25'])
  })

  it('códigos gramaticais não geram nada clicável', () => {
    const { container } = render(NodeView, {
      nodes: [strong(25, [text('palavra')]), text(' 8804')],
      t,
      actions: { onStrong: vi.fn() }
    })
    expect(container.querySelectorAll('button')).toHaveLength(1)
  })

  it('links de versículo e de verbete são botões; os demais são texto com dica', async () => {
    const onVerse = vi.fn()
    const onEntry = vi.fn()
    const nodes: UiNode[] = [
      { type: 'verseLink', book: 43, chapter: 3, verse: 16, children: [text('ligação de versículo')] },
      { type: 'entryLink', lang: 'H', num: 430, children: [text('ligação de verbete')] },
      { type: 'link', hint: 'person', children: [text('ligação desconhecida')] }
    ]
    const { container } = render(NodeView, { nodes, t, actions: { onVerse, onEntry } })
    await fireEvent.click(screen.getByRole('button', { name: 'ligação de versículo' }))
    expect(onVerse).toHaveBeenCalledWith({ book: 43, chapter: 3, verse: 16 })
    await fireEvent.click(screen.getByRole('button', { name: 'ligação de verbete' }))
    expect(onEntry).toHaveBeenCalledWith('H430')
    const unknown = screen.getByText('ligação desconhecida')
    expect(unknown.closest('button')).toBeNull()
    expect(unknown.closest('.unknown-link')?.getAttribute('title')).toBe(
      'Link de tipo não suportado (person)'
    )
    expect(container.querySelectorAll('button')).toHaveLength(2)
  })

  it('sem ações, os links aparecem como texto simples', () => {
    const { container } = render(NodeView, {
      nodes: [{ type: 'verseLink', book: 1, chapter: 1, verse: 1, children: [text('texto')] }],
      t
    })
    expect(container.querySelector('button')).toBeNull()
    expect(container.textContent).toBe('texto')
  })
})

describe('RichContent', () => {
  it('renderiza negrito, itálico, sublinhado, sobrescrito, parágrafos e links', () => {
    const nodes: UiNode[] = [
      { type: 'bold', children: [text('negrito')] },
      text(' '),
      { type: 'italic', children: [text('itálico')] },
      text(' '),
      { type: 'underline', children: [text('sublinhado')] },
      { type: 'superscript', children: [text('2')] },
      { type: 'paragraph' },
      { type: 'paragraph' },
      { type: 'verseLink', book: 43, chapter: 3, verse: 16, children: [text('ver')] },
      text(' <b>não é marcação</b>'),
      { type: 'lineBreak' },
      text('segunda linha')
    ]
    const { container } = render(RichContent, { nodes, t, actions: { onVerse: vi.fn() } })
    expect(container.querySelectorAll('p.rich-paragraph')).toHaveLength(2)
    expect(container.querySelector('strong')?.textContent).toBe('negrito')
    expect(container.querySelector('em')?.textContent).toBe('itálico')
    expect(container.querySelector('u')?.textContent).toBe('sublinhado')
    expect(container.querySelector('sup')?.textContent).toBe('2')
    expect(container.querySelector('b')).toBeNull()
    expect(container.textContent).toContain('<b>não é marcação</b>')
    expect(container.querySelector('br')).not.toBeNull()
    expect(splitParagraphs([text(' '), { type: 'paragraph' }, text('a')])).toHaveLength(1)
  })
})

function entry(key: string, overrides: Partial<LexiconEntryDto> = {}): LexiconEntryDto {
  return {
    key,
    title: key,
    original_word: null,
    module_id: 'lex',
    module_title: 'Léxico sintético',
    nodes: [text(`Conteúdo sintético de ${key}`)],
    ...overrides
  }
}

describe('aba Léxico', () => {
  function lexicon(handler: (key: string) => unknown, request = { ids: ['G25'], seq: 1 }) {
    invokeMock.mockImplementation(async (command: string, args: { strongId: string }) => {
      expect(command).toBe('lookup_strong')
      const result = handler(args.strongId)
      if (result instanceof Error) throw apiError(result.message)
      return result
    })
    const onVerse = vi.fn()
    const onManageFolders = vi.fn()
    const view = render(LexiconTab, { t, request, onVerse, onManageFolders })
    return { ...view, onVerse, onManageFolders }
  }

  it('mostra a entrada completa do verbete clicado', async () => {
    lexicon((key) => entry(key, { original_word: 'lema-sintético' }))
    expect(await screen.findByText('Conteúdo sintético de G25')).toBeTruthy()
    expect(screen.getByRole('heading', { name: 'G25' })).toBeTruthy()
    expect(screen.getByText('lema-sintético').getAttribute('dir')).toBe('auto')
    expect(screen.getByText('Léxico sintético')).toBeTruthy()
    expect(invokeMock).toHaveBeenCalledWith('lookup_strong', { strongId: 'G25' })
  })

  it('palavra com mais de um Strong mostra a lista no topo para escolher', async () => {
    lexicon((key) => entry(key), { ids: ['G26', 'G25'], seq: 1 })
    expect(await screen.findByText('Conteúdo sintético de G26')).toBeTruthy()
    const group = screen.getByRole('group', { name: 'Números de Strong desta palavra' })
    const [first, second] = Array.from(group.querySelectorAll('button'))
    expect(first.getAttribute('aria-pressed')).toBe('true')
    await fireEvent.click(second)
    expect(await screen.findByText('Conteúdo sintético de G25')).toBeTruthy()
    expect(second.getAttribute('aria-pressed')).toBe('true')
  })

  it('links para verbete abrem o verbete e o botão voltar retorna ao anterior', async () => {
    const link: UiNode = { type: 'entryLink', lang: 'G', num: 1, children: [text('ir ao G1')] }
    lexicon((key) => entry(key, { nodes: key === 'G25' ? [link] : [text(`Conteúdo de ${key}`)] }))
    const back = await screen.findByRole('button', { name: 'Voltar ao verbete anterior' })
    expect(back.hasAttribute('disabled')).toBe(true)
    await fireEvent.click(await screen.findByRole('button', { name: 'ir ao G1' }))
    expect(await screen.findByText('Conteúdo de G1')).toBeTruthy()
    expect(back.hasAttribute('disabled')).toBe(false)
    await fireEvent.click(back)
    expect(await screen.findByRole('button', { name: 'ir ao G1' })).toBeTruthy()
    expect(back.hasAttribute('disabled')).toBe(true)
  })

  it('links para versículo navegam a leitura', async () => {
    const link: UiNode = { type: 'verseLink', book: 43, chapter: 3, verse: 16, children: [text('ver versículo')] }
    const { onVerse } = lexicon((key) => entry(key, { nodes: [link] }))
    await fireEvent.click(await screen.findByRole('button', { name: 'ver versículo' }))
    expect(onVerse).toHaveBeenCalledWith({ book: 43, chapter: 3, verse: 16 })
  })

  it('um novo clique na Bíblia abre outro verbete e permite voltar ao anterior', async () => {
    invokeMock.mockImplementation(async (_command: string, args: { strongId: string }) => entry(args.strongId))
    const { rerender } = render(LexiconTab, {
      t,
      request: { ids: ['G25'], seq: 1 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn()
    })
    await screen.findByText('Conteúdo sintético de G25')
    await rerender({ request: { ids: ['H430'], seq: 2 } })
    expect(await screen.findByText('Conteúdo sintético de H430')).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Voltar ao verbete anterior' }))
    expect(await screen.findByText('Conteúdo sintético de G25')).toBeTruthy()
  })

  it('sem dicionário orienta o usuário e leva ao gerenciamento de pastas', async () => {
    const { onManageFolders } = lexicon(() => new Error('no_lexicon_installed'))
    expect(await screen.findByText('Instale um dicionário para ver o léxico')).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Gerenciar pastas' }))
    expect(onManageFolders).toHaveBeenCalledOnce()
  })

  it('mostra estados amigáveis para entrada ausente, formato não suportado e erro', async () => {
    lexicon(() => new Error('entry_not_found'))
    expect(await screen.findByText('Este número não está no dicionário escolhido')).toBeTruthy()
    expect(screen.getByText('G25')).toBeTruthy()
    cleanup()
    lexicon(() => new Error('unsupported_format'))
    expect(await screen.findByText(table.lexiconUnsupported)).toBeTruthy()
    cleanup()
    lexicon(() => new Error('internal'))
    expect(await screen.findByText(table.lexiconError)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Tentar novamente' })).toBeTruthy()
  })

  it('mostra o estado de carregamento e o aviso inicial sem palavra escolhida', async () => {
    let finish: (value: LexiconEntryDto) => void = () => undefined
    invokeMock.mockImplementation(() => new Promise<LexiconEntryDto>((resolve) => (finish = resolve)))
    render(LexiconTab, {
      t,
      request: { ids: ['G25'], seq: 1 },
      onVerse: vi.fn(),
      onManageFolders: vi.fn()
    })
    expect(await screen.findByText(table.lexiconLoading)).toBeTruthy()
    finish(entry('G25'))
    expect(await screen.findByText('Conteúdo sintético de G25')).toBeTruthy()
    cleanup()
    render(LexiconTab, { t, request: null, onVerse: vi.fn(), onManageFolders: vi.fn() })
    expect(screen.getByText(table.lexiconHint)).toBeTruthy()
  })
})

function commentaryModule(id: string, title = `Comentário ${id}`): ModuleInfo {
  return {
    id,
    path: `C:\\synthetic\\${id}.cmt.twm`,
    kind: 'Commentary',
    state: 'Ready',
    title,
    abbreviation: null,
    language: 'pt',
    language_name: 'Português',
    description: null,
    version: null,
    size: 1,
    has_strong: false
  }
}

const header = (topic_id: number, label: string, overrides = {}) => ({
  topic_id,
  scope: 'verse' as const,
  label,
  chapter: 3,
  from_verse: 16,
  to_verse: 16,
  size_bytes: 2048,
  content_type: 'rtf',
  supported: true,
  ...overrides
})

function index(verse: number, modules: CommentaryIndexDto['modules']): CommentaryIndexDto {
  return { book: 43, chapter: 3, verse, modules }
}

function page(nodes: UiNode[], hasMore: boolean, offset = 0): CommentaryPageDto {
  return { module_id: 'a', topic_id: 1, offset, next_offset: offset + 60, has_more: hasMore, nodes }
}

describe('aba Comentário', () => {
  function mount(extra: Record<string, unknown> = {}) {
    const props = {
      t,
      selection: { book: 43, chapter: 3, verse: 16 },
      commentaries: [commentaryModule('a'), commentaryModule('b')],
      activeIds: ['a'],
      onToggle: vi.fn(),
      onVerse: vi.fn(),
      onEntry: vi.fn(),
      onManageFolders: vi.fn(),
      ...extra
    }
    return { props, ...render(CommentaryTab, { props }) }
  }

  it('lista as entradas por módulo, do mais específico ao mais amplo, sem carregar o conteúdo', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      expect(command).toBe('get_commentary_index')
      return index(16, [
        {
          module_id: 'a',
          module_title: 'Comentário sintético',
          error: null,
          entries: [
            header(1, 'Versículo 16'),
            header(2, 'Capítulo 3', { scope: 'chapter' }),
            header(3, 'Introdução ao livro', { scope: 'bookIntro' })
          ]
        }
      ])
    })
    mount()
    const toggles = await screen.findAllByRole('button', { expanded: false })
    expect(toggles.map((button) => button.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
      'Versículo 16 2 KB',
      'Capítulo 3 2 KB',
      'Introdução ao livro 2 KB'
    ])
    expect(screen.getByRole('heading', { name: 'Comentário sintético' })).toBeTruthy()
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('get_commentary_index', { book: 43, chapter: 3, verse: 16 })
  })

  it('expandir carrega o conteúdo e "Mostrar mais" acrescenta a próxima página', async () => {
    invokeMock.mockImplementation(async (command: string, args: { offset?: number }) => {
      if (command === 'get_commentary_index') {
        return index(16, [{ module_id: 'a', module_title: 'Comentário sintético', error: null, entries: [header(1, 'Versículo 16')] }])
      }
      return args.offset === 0
        ? page([text('Primeira página sintética'), { type: 'paragraph' }], true)
        : page([text('Segunda página sintética')], false, 60)
    })
    mount()
    const toggle = await screen.findByRole('button', { name: /Versículo 16/ })
    expect(screen.queryByText('Primeira página sintética')).toBeNull()
    await fireEvent.click(toggle)
    expect(await screen.findByText('Primeira página sintética')).toBeTruthy()
    expect(toggle.getAttribute('aria-expanded')).toBe('true')
    expect(invokeMock).toHaveBeenLastCalledWith('get_commentary_entry', { moduleId: 'a', topicId: 1, offset: 0 })
    await fireEvent.click(screen.getByRole('button', { name: 'Mostrar mais' }))
    expect(await screen.findByText('Segunda página sintética')).toBeTruthy()
    expect(screen.getByText('Primeira página sintética')).toBeTruthy()
    expect(invokeMock).toHaveBeenLastCalledWith('get_commentary_entry', { moduleId: 'a', topicId: 1, offset: 60 })
    expect(screen.queryByRole('button', { name: 'Mostrar mais' })).toBeNull()
    await fireEvent.click(toggle)
    expect(screen.queryByText('Primeira página sintética')).toBeNull()
  })

  it('entrada em formato não suportado mostra um aviso discreto e não é carregada', async () => {
    invokeMock.mockImplementation(async () =>
      index(16, [
        {
          module_id: 'a',
          module_title: 'Comentário sintético',
          error: null,
          entries: [header(1, 'Versículo 16', { supported: false, content_type: 'rvf' }), header(2, 'Capítulo 3', { scope: 'chapter' })]
        }
      ])
    )
    mount()
    const rvf = await screen.findByRole('button', { name: /Versículo 16/ })
    expect(screen.getAllByText('Formato ainda não suportado.')).toHaveLength(1)
    await fireEvent.click(rvf)
    expect(rvf.hasAttribute('aria-expanded')).toBe(false)
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: /Capítulo 3/ })).toBeTruthy()
  })

  it('segue o versículo selecionado com atraso de 150 ms', async () => {
    vi.useFakeTimers()
    invokeMock.mockImplementation(async (_command: string, args: { verse: number }) =>
      index(args.verse, [{ module_id: 'a', module_title: 'Comentário sintético', error: null, entries: [header(args.verse, `Versículo ${args.verse}`)] }])
    )
    const { rerender } = mount()
    await vi.advanceTimersByTimeAsync(149)
    expect(invokeMock).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(await screen.findByRole('button', { name: /Versículo 16/ })).toBeTruthy()
    await rerender({ selection: { book: 43, chapter: 3, verse: 17 } })
    await rerender({ selection: { book: 43, chapter: 3, verse: 18 } })
    await vi.advanceTimersByTimeAsync(149)
    expect(invokeMock).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(1)
    expect(invokeMock).toHaveBeenCalledTimes(2)
    expect(invokeMock).toHaveBeenLastCalledWith('get_commentary_index', { book: 43, chapter: 3, verse: 18 })
    expect(await screen.findByRole('button', { name: /Versículo 18/ })).toBeTruthy()
    expect(screen.queryByRole('button', { name: /Versículo 16/ })).toBeNull()
  })

  it('ativar e desativar módulos chama a ação, que persiste a escolha', async () => {
    invokeMock.mockResolvedValue(index(16, []))
    const { props } = mount()
    const first = screen.getByRole('checkbox', { name: 'Comentário a' }) as HTMLInputElement
    const second = screen.getByRole('checkbox', { name: 'Comentário b' }) as HTMLInputElement
    expect(first.checked).toBe(true)
    expect(second.checked).toBe(false)
    await fireEvent.click(second)
    expect(props.onToggle).toHaveBeenLastCalledWith('b', true)
    await fireEvent.click(first)
    expect(props.onToggle).toHaveBeenLastCalledWith('a', false)
  })

  it('estados vazios: sem módulos, sem módulo ativo, sem versículo e sem comentários', async () => {
    mount({ commentaries: [], activeIds: [] })
    expect(screen.getByText(table.commentaryNone)).toBeTruthy()
    cleanup()
    const manage = mount({ commentaries: [commentaryModule('a')], activeIds: [] })
    expect(screen.getByText(table.commentaryNoneActive)).toBeTruthy()
    expect(screen.getByRole('checkbox', { name: 'Comentário a' })).toBeTruthy()
    expect(manage.props.onManageFolders).not.toHaveBeenCalled()
    cleanup()
    mount({ selection: null })
    expect(screen.getByText(table.commentarySelectVerse)).toBeTruthy()
    cleanup()
    invokeMock.mockResolvedValue(index(16, [{ module_id: 'a', module_title: 'Comentário a', error: null, entries: [] }]))
    mount()
    expect(await screen.findByText('Nenhum comentário para este versículo nos módulos ativos')).toBeTruthy()
  })

  it('sem comentários instalados, o botão leva ao gerenciamento de pastas', async () => {
    const { props } = mount({ commentaries: [], activeIds: [] })
    await fireEvent.click(screen.getByRole('button', { name: 'Gerenciar pastas' }))
    expect(props.onManageFolders).toHaveBeenCalledOnce()
  })

  it('erro do módulo fica isolado e erro geral oferece nova tentativa', async () => {
    invokeMock.mockResolvedValue(
      index(16, [
        { module_id: 'a', module_title: 'Comentário a', error: apiError('module_protected'), entries: [] },
        { module_id: 'b', module_title: 'Comentário b', error: null, entries: [header(1, 'Versículo 16')] }
      ])
    )
    mount({ activeIds: ['a', 'b'] })
    expect(await screen.findByText('Módulo protegido: abra-o no theWord.')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Versículo 16/ })).toBeTruthy()
    cleanup()
    invokeMock.mockReset()
    invokeMock.mockRejectedValueOnce(apiError('internal'))
    invokeMock.mockResolvedValue(index(16, [{ module_id: 'a', module_title: 'Comentário a', error: null, entries: [header(1, 'Versículo 16')] }]))
    mount()
    expect(await screen.findByText(table.commentaryError)).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Tentar novamente' }))
    expect(await screen.findByRole('button', { name: /Versículo 16/ })).toBeTruthy()
  })

  it('links dentro do comentário navegam a leitura e abrem verbetes', async () => {
    invokeMock.mockImplementation(async (command: string) =>
      command === 'get_commentary_index'
        ? index(16, [{ module_id: 'a', module_title: 'Comentário a', error: null, entries: [header(1, 'Versículo 16')] }])
        : page(
            [
              { type: 'verseLink', book: 43, chapter: 1, verse: 1, children: [text('ir ao versículo')] },
              { type: 'entryLink', lang: 'G', num: 26, children: [text('ir ao verbete')] }
            ],
            false
          )
    )
    const { props } = mount()
    await fireEvent.click(await screen.findByRole('button', { name: /Versículo 16/ }))
    await fireEvent.click(await screen.findByRole('button', { name: 'ir ao versículo' }))
    expect(props.onVerse).toHaveBeenCalledWith({ book: 43, chapter: 1, verse: 1 })
    await fireEvent.click(screen.getByRole('button', { name: 'ir ao verbete' }))
    expect(props.onEntry).toHaveBeenCalledWith('G26')
  })
})

describe('painel de estudo', () => {
  function panel(extra: Record<string, unknown> = {}) {
    invokeMock.mockResolvedValue(index(16, []))
    const props = {
      t,
      open: true,
      tab: 'lexicon' as const,
      strongRequest: null,
      selection: { book: 43, chapter: 3, verse: 16 },
      commentaries: [commentaryModule('a')],
      activeCommentaryIds: ['a'],
      onTab: vi.fn(),
      onToggleCommentary: vi.fn(),
      onVerse: vi.fn(),
      onEntry: vi.fn(),
      onManageFolders: vi.fn(),
      onEscape: vi.fn(),
      ...extra
    }
    return { props, ...render(StudyPanel, { props }) }
  }

  it('mostra só as abas Léxico e Comentário, com navegação por setas', async () => {
    const { props } = panel()
    const tabs = screen.getAllByRole('tab')
    expect(tabs.map((tab) => tab.textContent)).toEqual(['Léxico', 'Comentário', 'Sermão'])
    expect(screen.queryByRole('tab', { name: /Referências/ })).toBeNull()
    expect(tabs[0].getAttribute('aria-selected')).toBe('true')
    expect(tabs.map((tab) => tab.getAttribute('tabindex'))).toEqual(['0', '-1', '-1'])
    await fireEvent.keyDown(tabs[0], { key: 'ArrowRight' })
    expect(props.onTab).toHaveBeenLastCalledWith('commentary')
    await fireEvent.keyDown(tabs[0], { key: 'ArrowLeft' })
    expect(props.onTab).toHaveBeenLastCalledWith('sermon')
    await fireEvent.keyDown(tabs[1], { key: 'Home' })
    expect(props.onTab).toHaveBeenLastCalledWith('lexicon')
    expect(tabs[0].getAttribute('aria-controls')).toBe('study-panel-lexicon')
  })

  it('Esc pede para devolver o foco à palavra e o foco vai ao conteúdo ao abrir', async () => {
    const { props, component } = panel()
    const content = document.getElementById('study-panel-lexicon') as HTMLElement
    component.focusContent()
    expect(document.activeElement).toBe(content)
    expect(component.contains(content)).toBe(true)
    await fireEvent.keyDown(content, { key: 'Escape' })
    expect(props.onEscape).toHaveBeenCalledOnce()
  })

  it('fica oculto quando recolhido e mostra a aba escolhida', () => {
    const { container } = panel({ open: false, tab: 'commentary' })
    expect((container.querySelector('aside') as HTMLElement).hidden).toBe(true)
    expect((document.getElementById('study-panel-lexicon') as HTMLElement).hidden).toBe(true)
    expect((document.getElementById('study-panel-commentary') as HTMLElement).hidden).toBe(false)
  })

  it('só consulta comentários quando o painel e a aba estão visíveis', async () => {
    panel({ open: false, tab: 'commentary' })
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('renderiza link inline sem quebrar parágrafo e exibe módulo · código em erro de comentário', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_commentary_index') {
        return {
          book: 1,
          chapter: 24,
          verse: 1,
          modules: [
            {
              module_id: 'w1',
              module_title: 'Wiersbe',
              error: null,
              entries: [
                {
                  topic_id: 24,
                  scope: 'chapter',
                  label: 'Capítulo 24',
                  chapter: 24,
                  from_verse: 1,
                  to_verse: 67,
                  size_bytes: 100,
                  content_type: 'rtf',
                  supported: true
                }
              ]
            },
            {
              module_id: 'n1',
              module_title: 'NICOT',
              error: { code: 'module_corrupted', message: 'falha' },
              entries: []
            }
          ]
        }
      }
      if (cmd === 'get_commentary_entry') {
        return {
          module_id: 'w1',
          topic_id: 24,
          nodes: [
            { type: 'text', value: 'Veja ' },
            {
              type: 'verseLink',
              book: 43,
              chapter: 3,
              verse: 16,
              children: [{ type: 'text', value: 'Jo 3:16' }]
            },
            { type: 'text', value: ' no texto.' }
          ],
          has_more: false,
          degraded: false,
          next_offset: 0
        }
      }
      return null
    })

    const { container } = render(CommentaryTab, {
      props: {
        t,
        selection: { book: 1, chapter: 24, verse: 1 },
        commentaries: [
          {
            id: 'w1',
            path: 'w.twm',
            kind: 'Commentary',
            state: 'Ready',
            title: 'Wiersbe',
            abbreviation: null,
            language: 'pt',
            language_name: null,
            description: null,
            version: null,
            size: 1,
            has_strong: false
          },
          {
            id: 'n1',
            path: 'n.twm',
            kind: 'Commentary',
            state: 'Ready',
            title: 'NICOT',
            abbreviation: null,
            language: 'en',
            language_name: null,
            description: null,
            version: null,
            size: 1,
            has_strong: false
          }
        ],
        activeIds: ['w1', 'n1'],
        active: true,
        onToggle: vi.fn(),
        onVerse: vi.fn(),
        onEntry: vi.fn(),
        onManageFolders: vi.fn()
      }
    })

    expect(await screen.findByText('NICOT · module_corrupted')).toBeTruthy()
    const btn = await screen.findByRole('button', { name: /Capítulo 24/i })
    await fireEvent.click(btn)
    const linkBtn = await screen.findByRole('button', { name: 'Jo 3:16' })
    const paragraphs = container.querySelectorAll('.rich-paragraph')
    expect(paragraphs).toHaveLength(1)
    expect(paragraphs[0].contains(linkBtn)).toBe(true)
  })
})
