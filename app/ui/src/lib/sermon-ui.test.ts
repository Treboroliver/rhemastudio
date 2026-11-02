import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import SermonTab from './SermonTab.svelte'
import ptBr from '../i18n/pt-BR.json'
import en from '../i18n/en.json'
import { MAX_BLOCKS, type Block, type SermonDto, type VersionInfoDto } from './sermon'

const invokeMock = vi.hoisted(() => vi.fn())
const closeHooks = vi.hoisted(() => ({ handler: null as null | (() => Promise<void>) }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: () => Promise<void>) => {
      closeHooks.handler = handler
      return () => undefined
    }
  })
}))

const t = (key: string, values: Record<string, string | number> = {}) =>
  ((ptBr as Record<string, string>)[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))

interface Db {
  sermons: Map<string, SermonDto>
  versions: Map<string, VersionInfoDto[]>
  restoreTo: SermonDto | null
  calls: [string, Record<string, unknown>][]
  saves: SermonDto[]
  failSave: number
  gate: Promise<void> | null
  unavailable: boolean
  listFails: boolean
  exportResult: string | null
  active: number
  peak: number
  counter: number
}

let db: Db

const fold = (value: string) => value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase()
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value)) as T

function sermon(id: string, title: string, blocks: Block[] = [], extra: Partial<SermonDto> = {}): SermonDto {
  return { id, title, base_text: '', created_at: 1000, updated_at: 2000, archived: false, blocks, ...extra }
}
const text = (id: string, markdown: string): Block => ({ type: 'text', id, markdown })
const heading = (id: string, value: string, level = 2): Block => ({ type: 'heading', id, level, text: value })

beforeEach(() => {
  vi.useFakeTimers()
  closeHooks.handler = null
  db = {
    sermons: new Map(),
    versions: new Map(),
    restoreTo: null,
    calls: [],
    saves: [],
    failSave: 0,
    gate: null,
    unavailable: false,
    listFails: false,
    exportResult: '/tmp/saida.md',
    active: 0,
    peak: 0,
    counter: 0
  }
  invokeMock.mockImplementation(async (command: string, args: Record<string, unknown> = {}) => {
    db.calls.push([command, args])
    if (db.unavailable) throw { code: 'sermon_unavailable', message: 'indisponível' }
    switch (command) {
      case 'sermon_list': {
        if (db.listFails) throw { code: 'sermon_database', message: 'falha' }
        const query = fold(String(args.query ?? ''))
        return [...db.sermons.values()]
          .filter((item) => args.includeArchived || !item.archived)
          .filter((item) => !query || fold(item.title).includes(query))
          .sort((a, b) => b.updated_at - a.updated_at)
          .map((item) => ({ ...item, blocks: undefined, block_count: item.blocks.length }))
      }
      case 'sermon_create': {
        db.counter += 1
        const created = sermon(`new-${db.counter}`, String(args.title), [], { updated_at: 3000 })
        db.sermons.set(created.id, created)
        return clone(created)
      }
      case 'sermon_get': {
        const found = db.sermons.get(String(args.id))
        if (!found) throw { code: 'sermon_not_found', message: 'não encontrado' }
        return clone(found)
      }
      case 'sermon_save': {
        const payload = clone(args.sermon as SermonDto)
        db.active += 1
        db.peak = Math.max(db.peak, db.active)
        db.saves.push(payload)
        try {
          if (db.gate) await db.gate
          if (db.failSave > 0) {
            db.failSave -= 1
            throw { code: 'sermon_database', message: 'falha' }
          }
          db.sermons.set(payload.id, payload)
          return clone(payload)
        } finally {
          db.active -= 1
        }
      }
      case 'sermon_duplicate': {
        const source = db.sermons.get(String(args.id))
        if (!source) throw { code: 'sermon_not_found', message: 'x' }
        db.counter += 1
        const copy = { ...clone(source), id: `copy-${db.counter}`, title: `${source.title} (cópia)`, updated_at: 5000 }
        db.sermons.set(copy.id, copy)
        return clone(copy)
      }
      case 'sermon_archive': {
        const found = db.sermons.get(String(args.id))
        if (found) found.archived = Boolean(args.archived)
        return null
      }
      case 'sermon_delete':
        db.sermons.delete(String(args.id))
        return null
      case 'sermon_versions':
        return clone(db.versions.get(String(args.id)) ?? [])
      case 'sermon_restore': {
        if (!db.restoreTo) throw { code: 'sermon_not_found', message: 'x' }
        db.sermons.set(String(args.id), clone(db.restoreTo))
        return clone(db.restoreTo)
      }
      case 'sermon_save_export':
        return db.exportResult
      default:
        throw new Error(`comando inesperado: ${command}`)
    }
  })
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
})

const settle = async (ms = 0) => {
  await vi.advanceTimersByTimeAsync(ms)
}
const books = [{ number: 43, abbreviation: 'Jo' }]
const mount = async (active = true) => {
  const view = render(SermonTab, { props: { t, language: 'pt-BR', books, active } })
  await settle()
  return view
}
const titleInput = () => document.querySelector<HTMLInputElement>('[data-focus="title"]')!
const baseInput = () => document.querySelector<HTMLInputElement>('[data-focus="base"]')!
const fields = () => Array.from(document.querySelectorAll<HTMLTextAreaElement>('textarea.sermon-textarea'))
const saveCalls = () => db.calls.filter(([command]) => command === 'sermon_save').length
const click = async (element: Element) => {
  await fireEvent.click(element)
  await settle()
}
const typeInto = async (element: Element, value: string) => {
  await fireEvent.input(element, { target: { value } })
}
async function openEditor(title = 'Sermão A') {
  await mount()
  await click(screen.getByText(title))
}
async function addBlock(kindName: string, position = 1) {
  await click(screen.getByRole('button', { name: `Adicionar bloco na posição ${position}` }))
  await click(screen.getByRole('menuitem', { name: kindName }))
}
const seedA = (blocks: Block[] = [text('a', 'um'), text('b', 'dois'), text('c', 'três')]) =>
  db.sermons.set('s1', sermon('s1', 'Sermão A', blocks, { base_text: 'Texto base inventado' }))

describe('i18n', () => {
  it('has the same keys in pt-BR and en', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ptBr).sort())
  })
})

describe('lista de sermões', () => {
  it('mostra título, texto base, contagem e ordena pelo mais recente', async () => {
    seedA()
    db.sermons.set('s2', sermon('s2', 'Sermão B', [], { updated_at: 9000 }))
    await mount()
    const titles = Array.from(document.querySelectorAll('.sermon-title')).map((node) => node.textContent)
    expect(titles).toEqual(['Sermão B', 'Sermão A'])
    expect(screen.getByText('Texto base inventado')).toBeTruthy()
    expect(screen.getByText(/3 blocos/)).toBeTruthy()
  })

  it('mostra o estado vazio, o de erro com nova tentativa e o de banco indisponível', async () => {
    await mount()
    expect(screen.getByText('Você ainda não tem sermões. Crie o primeiro.')).toBeTruthy()
    cleanup()
    db.listFails = true
    await mount()
    expect(screen.getByRole('alert').textContent).toContain('Não foi possível carregar a lista')
    db.listFails = false
    await click(screen.getByRole('button', { name: 'Tentar de novo' }))
    expect(screen.getByText('Você ainda não tem sermões. Crie o primeiro.')).toBeTruthy()
    cleanup()
    db.unavailable = true
    await mount()
    expect(screen.getByRole('alert').textContent).toBe(
      'Não foi possível abrir seus sermões neste momento. Seus dados não foram alterados.'
    )
  })

  it('busca por título (ignorando acentos) e alterna arquivados', async () => {
    seedA()
    db.sermons.set('s2', sermon('s2', 'Graça abundante', [], { archived: true }))
    await mount()
    expect(screen.queryByText('Graça abundante')).toBeNull()
    await click(screen.getByLabelText('Mostrar arquivados'))
    expect(screen.getByText('Graça abundante')).toBeTruthy()
    expect(db.calls.at(-1)?.[1]).toMatchObject({ includeArchived: true })
    await fireEvent.input(screen.getByRole('searchbox'), { target: { value: 'graca' } })
    await settle()
    expect(db.calls.at(-1)?.[1]).toMatchObject({ query: 'graca' })
    expect(screen.queryByText('Sermão A')).toBeNull()
    expect(screen.getByText('Graça abundante')).toBeTruthy()
    await fireEvent.input(screen.getByRole('searchbox'), { target: { value: 'zzz' } })
    await settle()
    expect(screen.getByText('Nenhum sermão encontrado com esse título.')).toBeTruthy()
  })

  it('cria um sermão sem título e abre o editor com o foco no título', async () => {
    await mount()
    await click(screen.getByRole('button', { name: 'Novo sermão' }))
    expect(db.calls.some(([command, args]) => command === 'sermon_create' && args.title === 'Sermão sem título')).toBe(true)
    expect(titleInput().value).toBe('Sermão sem título')
    expect(document.activeElement).toBe(titleInput())
  })

  it('duplica, arquiva e exclui com confirmação', async () => {
    seedA()
    await mount()
    const menu = async (title: string, item: string) => {
      await click(screen.getByRole('button', { name: `Ações do sermão ${title}` }))
      await click(screen.getByRole('menuitem', { name: item }))
    }
    await menu('Sermão A', 'Duplicar')
    expect(screen.getByText('Sermão A (cópia)')).toBeTruthy()

    await menu('Sermão A (cópia)', 'Arquivar')
    expect(screen.queryByText('Sermão A (cópia)')).toBeNull()
    await click(screen.getByLabelText('Mostrar arquivados'))
    await menu('Sermão A (cópia)', 'Desarquivar')
    expect(db.sermons.get('copy-1')?.archived).toBe(false)

    await menu('Sermão A (cópia)', 'Excluir')
    expect(screen.getByRole('alertdialog').textContent).toContain('não poderão ser recuperados')
    expect(db.sermons.has('copy-1')).toBe(true)
    await click(screen.getByRole('button', { name: 'Cancelar' }))
    expect(db.sermons.has('copy-1')).toBe(true)
    await menu('Sermão A (cópia)', 'Excluir')
    await click(screen.getByRole('button', { name: 'Excluir para sempre' }))
    expect(db.sermons.has('copy-1')).toBe(false)
    expect(screen.queryByText('Sermão A (cópia)')).toBeNull()
  })
})

describe('editor', () => {
  it('abre o sermão, mostra os campos e dá foco ao título', async () => {
    seedA()
    await openEditor()
    expect(titleInput().value).toBe('Sermão A')
    expect(baseInput().value).toBe('Texto base inventado')
    expect(fields().map((field) => field.value)).toEqual(['um', 'dois', 'três'])
    expect(document.activeElement).toBe(titleInput())
  })

  it('salva 500 ms depois da última alteração, com o sermão inteiro', async () => {
    seedA()
    await openEditor()
    await typeInto(titleInput(), 'Novo título')
    await settle(499)
    expect(saveCalls()).toBe(0)
    await typeInto(baseInput(), 'Outro texto base')
    await settle(499)
    expect(saveCalls()).toBe(0)
    await settle(1)
    expect(saveCalls()).toBe(1)
    expect(db.saves[0]).toMatchObject({ title: 'Novo título', base_text: 'Outro texto base' })
    expect(db.saves[0].blocks).toHaveLength(3)
    expect(screen.getByText('Salvo')).toBeTruthy()
  })

  it('edita o texto de um bloco e o tipo/nível de um título', async () => {
    seedA([heading('h', 'Ideia', 2), text('t', 'corpo')])
    await openEditor()
    await typeInto(fields()[1], 'corpo novo')
    await fireEvent.change(screen.getByLabelText('Nível do título do bloco 1'), { target: { value: '3' } })
    await fireEvent.change(screen.getByLabelText('Tipo do bloco 2'), { target: { value: 'note' } })
    await settle(500)
    expect(db.saves.at(-1)!.blocks).toEqual([
      { type: 'heading', id: 'h', level: 3, text: 'Ideia' },
      { type: 'note', id: 't', markdown: 'corpo novo' }
    ])
  })

  it('adiciona blocos pelo menu e leva o foco ao novo bloco', async () => {
    seedA([text('a', 'um')])
    await openEditor()
    await addBlock('Ilustração', 2)
    expect(fields()).toHaveLength(2)
    expect(document.activeElement).toBe(fields()[1])
    await addBlock('Título', 2)
    expect(fields().map((field) => field.getAttribute('aria-label'))).toEqual([
      'Texto, bloco 1',
      'Título, bloco 2',
      'Ilustração, bloco 3'
    ])
    await settle(500)
    expect(db.saves.at(-1)!.blocks.map((block) => block.type)).toEqual(['text', 'heading', 'illustration'])
  })

  it('move por botão e por Alt+↑/↓ e salva a nova ordem', async () => {
    seedA()
    await openEditor()
    await click(screen.getByRole('button', { name: 'Mover bloco 1 para baixo' }))
    expect(fields().map((field) => field.value)).toEqual(['dois', 'um', 'três'])
    await fireEvent.keyDown(fields()[2], { key: 'ArrowUp', altKey: true })
    await settle()
    expect(fields().map((field) => field.value)).toEqual(['dois', 'três', 'um'])
    await fireEvent.keyDown(fields()[1], { key: 'ArrowDown', altKey: true })
    await settle()
    expect(fields().map((field) => field.value)).toEqual(['dois', 'um', 'três'])
    expect((screen.getByRole('button', { name: 'Mover bloco 1 para cima' }) as HTMLButtonElement).disabled).toBe(true)
    await settle(500)
    expect(db.saves.at(-1)!.blocks.map((block) => block.id)).toEqual(['b', 'a', 'c'])
  })

  it('duplica um bloco com novo id', async () => {
    seedA([text('a', 'um')])
    await openEditor()
    await click(screen.getByRole('button', { name: 'Duplicar bloco 1' }))
    expect(fields().map((field) => field.value)).toEqual(['um', 'um'])
    await settle(500)
    const ids = db.saves.at(-1)!.blocks.map((block) => block.id)
    expect(new Set(ids).size).toBe(2)
  })

  it('remove com aviso de 8 s e desfaz, inclusive a ordem', async () => {
    seedA()
    await openEditor()
    await click(screen.getByRole('button', { name: 'Remover bloco 2' }))
    expect(fields().map((field) => field.value)).toEqual(['um', 'três'])
    expect(screen.getByText('Bloco removido')).toBeTruthy()
    await click(screen.getByRole('button', { name: 'Desfazer' }))
    expect(fields().map((field) => field.value)).toEqual(['um', 'dois', 'três'])
    expect(screen.queryByText('Bloco removido')).toBeNull()

    await click(screen.getByRole('button', { name: 'Remover bloco 1' }))
    await settle(7999)
    expect(screen.getByText('Bloco removido')).toBeTruthy()
    await settle(1)
    expect(screen.queryByText('Bloco removido')).toBeNull()
    await settle(500)
    expect(db.saves.at(-1)!.blocks.map((block) => block.id)).toEqual(['b', 'c'])
  })

  it('exibe versículo e léxico salvos como somente leitura, com mover e remover', async () => {
    seedA([
      {
        type: 'verse',
        id: 'v',
        range: { start: { book: 'John', chapter: 3, verse: 16 }, end: { book: 'John', chapter: 3, verse: 17 } },
        module_id: 'bib',
        module_abbr: 'XYZ',
        snapshot: 'Texto inventado do versículo'
      },
      { type: 'lexicon', id: 'l', strong_id: 'G25', module_id: 'dic', title: 'amar', snapshot: 'Verbete inventado' }
    ])
    await openEditor()
    expect(screen.getByText('Jo 3:16-17 · XYZ')).toBeTruthy()
    expect(screen.getByText('Texto inventado do versículo')).toBeTruthy()
    expect(screen.getByText('G25 — amar')).toBeTruthy()
    expect(fields()).toHaveLength(0)
    await click(screen.getByRole('button', { name: 'Mover bloco 1 para baixo' }))
    expect(Array.from(document.querySelectorAll('.sermon-ref')).map((node) => node.textContent)).toEqual([
      'G25 — amar',
      'Jo 3:16-17 · XYZ'
    ])
    await click(screen.getByRole('button', { name: 'Remover bloco 1' }))
    await settle(500)
    expect(db.saves.at(-1)!.blocks.map((block) => block.id)).toEqual(['v'])
  })

  it('respeita os limites de título, texto base, bloco e quantidade de blocos', async () => {
    seedA([text('a', 'um')])
    await openEditor()
    expect(titleInput().maxLength).toBe(200)
    expect(baseInput().maxLength).toBe(200)
    await typeInto(titleInput(), 'x'.repeat(200))
    expect(screen.getByText('O título pode ter até 200 caracteres.')).toBeTruthy()

    await typeInto(fields()[0], 'y'.repeat(201 * 1024))
    expect(fields()[0].value).toBe('um')
    expect(screen.getByText(/tamanho máximo \(200 KB\)/)).toBeTruthy()
    await settle(500)
    expect(db.saves.at(-1)!.blocks[0]).toMatchObject({ markdown: 'um' })
  })

  // Lento (renderiza 1.000 blocos); rode com RHEMA_SLOW_TESTS=1. O limite também é coberto em sermon-insert.test.ts.
  it.skipIf(!process.env.RHEMA_SLOW_TESTS)('não passa de 1.000 blocos', { timeout: 120_000 }, async () => {
    seedA(Array.from({ length: MAX_BLOCKS }, (_, index) => text(`b${index}`, `n${index}`)))
    await openEditor()
    expect(fields()).toHaveLength(MAX_BLOCKS)
    await addBlock('Texto', MAX_BLOCKS + 1)
    expect(fields()).toHaveLength(MAX_BLOCKS)
    expect(screen.getByText('Um sermão pode ter até 1000 blocos.')).toBeTruthy()
    expect((screen.getByRole('button', { name: 'Duplicar bloco 1' }) as HTMLButtonElement).disabled).toBe(false)
  })

  it('título vazio é gravado como "Sermão sem título" sem apagar o campo', async () => {
    seedA()
    await openEditor()
    await typeInto(titleInput(), '   ')
    await settle(500)
    expect(db.saves.at(-1)!.title).toBe('Sermão sem título')
    expect(titleInput().value).toBe('   ')
  })
})

describe('salvamento', () => {
  it('nunca grava duas vezes ao mesmo tempo e regrava com o estado mais recente', async () => {
    seedA()
    await openEditor()
    let release: () => void = () => undefined
    db.gate = new Promise<void>((resolve) => (release = resolve))
    await typeInto(titleInput(), 'Primeira')
    await settle(500)
    expect(db.saves).toHaveLength(1)
    expect(screen.getByText('Salvando…')).toBeTruthy()
    await typeInto(titleInput(), 'Segunda')
    await typeInto(fields()[0], 'durante a gravação')
    await settle(2000)
    expect(db.saves).toHaveLength(1)
    db.gate = null
    release()
    await settle()
    expect(db.saves).toHaveLength(2)
    expect(db.peak).toBe(1)
    expect(db.saves[1]).toMatchObject({ title: 'Segunda' })
    expect(db.saves[1].blocks[0]).toMatchObject({ markdown: 'durante a gravação' })
    expect(screen.getByText('Salvo')).toBeTruthy()
  })

  it('em erro mostra o aviso, mantém o estado e a nova tentativa grava tudo', async () => {
    seedA()
    await openEditor()
    db.failSave = 1
    await typeInto(fields()[1], 'texto importante')
    await settle(500)
    expect(screen.getByText('Erro ao salvar — tentar de novo')).toBeTruthy()
    expect(fields()[1].value).toBe('texto importante')
    expect(db.sermons.get('s1')!.blocks[1]).toMatchObject({ markdown: 'dois' })
    await click(screen.getByRole('button', { name: 'Tentar de novo' }))
    expect(screen.getByText('Salvo')).toBeTruthy()
    expect(db.sermons.get('s1')!.blocks[1]).toMatchObject({ markdown: 'texto importante' })
    expect(fields()[1].value).toBe('texto importante')
  })

  it('Ctrl+S grava na hora', async () => {
    seedA()
    await openEditor()
    await typeInto(titleInput(), 'Rápido')
    expect(saveCalls()).toBe(0)
    await fireEvent.keyDown(titleInput(), { key: 's', ctrlKey: true })
    await settle()
    expect(saveCalls()).toBe(1)
    expect(db.saves[0].title).toBe('Rápido')
  })

  it('grava antes de voltar para a lista e não volta se a gravação falha', async () => {
    seedA()
    await openEditor()
    db.failSave = 1
    await typeInto(titleInput(), 'Ao voltar')
    await click(screen.getByRole('button', { name: /Voltar para a lista/ }))
    expect(titleInput()).toBeTruthy()
    expect(screen.getByText(/Suas alterações continuam aqui/)).toBeTruthy()
    await click(screen.getByRole('button', { name: /Voltar para a lista/ }))
    expect(db.sermons.get('s1')!.title).toBe('Ao voltar')
    expect(screen.getByText('Ao voltar')).toBeTruthy()
    expect(document.querySelector('.sermon-list')).toBeTruthy()
  })

  it('grava ao deixar a aba', async () => {
    seedA()
    const view = await mount()
    await click(screen.getByText('Sermão A'))
    await typeInto(titleInput(), 'Troca de aba')
    expect(saveCalls()).toBe(0)
    await view.rerender({ active: false })
    await settle()
    expect(saveCalls()).toBe(1)
    expect(db.saves[0].title).toBe('Troca de aba')
  })

  it('grava ao fechar a janela, esperando no máximo 1 s', async () => {
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    seedA()
    await openEditor()
    expect(closeHooks.handler).toBeTruthy()
    await typeInto(titleInput(), 'Ao fechar')
    let done = false
    void closeHooks.handler!().then(() => (done = true))
    await settle()
    expect(done).toBe(true)
    expect(db.saves[0].title).toBe('Ao fechar')

    db.gate = new Promise<void>(() => undefined)
    await typeInto(titleInput(), 'Travado')
    done = false
    void closeHooks.handler!().then(() => (done = true))
    await settle(999)
    expect(done).toBe(false)
    await settle(1)
    expect(done).toBe(true)
  })
})

describe('exportação e versões', () => {
  it('exporta depois de gravar e informa o resultado', async () => {
    seedA()
    await openEditor()
    await typeInto(titleInput(), 'Para exportar')
    await click(screen.getByRole('button', { name: 'Exportar' }))
    const order = db.calls.map(([command]) => command).filter((command) => command.startsWith('sermon_s'))
    expect(order.slice(-2)).toEqual(['sermon_save', 'sermon_save_export'])
    expect(screen.getByText('Sermão exportado com sucesso.')).toBeTruthy()
    db.exportResult = null
    await click(screen.getByRole('button', { name: 'Exportar' }))
    expect(screen.getByText('Exportação cancelada.')).toBeTruthy()
  })

  it('lista versões anteriores e restaura após confirmação, recarregando o editor', async () => {
    seedA()
    db.versions.set('s1', [{ id: 7, sermon_id: 's1', created_at: 1_700_000_000_000, title: 'Versão antiga', block_count: 1 }])
    db.restoreTo = sermon('s1', 'Versão antiga', [text('x', 'conteúdo restaurado')])
    await openEditor()
    await click(screen.getByRole('button', { name: 'Versões anteriores' }))
    expect(screen.getByText(/Versão antiga · 1 blocos/)).toBeTruthy()
    await click(screen.getByRole('button', { name: 'Restaurar' }))
    expect(screen.getByRole('alertdialog').textContent).toContain('A versão atual será guardada antes')
    expect(db.calls.some(([command]) => command === 'sermon_restore')).toBe(false)
    await click(screen.getByRole('button', { name: 'Restaurar esta versão' }))
    expect(db.calls.some(([command, args]) => command === 'sermon_restore' && args.versionId === 7)).toBe(true)
    expect(titleInput().value).toBe('Versão antiga')
    expect(fields().map((field) => field.value)).toEqual(['conteúdo restaurado'])
    expect(screen.getByText('Versão restaurada.')).toBeTruthy()
  })

  it('mostra mensagem simples quando não há versões', async () => {
    seedA()
    await openEditor()
    await click(screen.getByRole('button', { name: 'Versões anteriores' }))
    expect(screen.getByText('Ainda não há cópias de segurança deste sermão.')).toBeTruthy()
  })

  it('avisa quando o sermão não pode ser aberto', async () => {
    await mount()
    db.sermons.set('s9', sermon('s9', 'Some'))
    await click(screen.getByRole('button', { name: 'Novo sermão' }))
    expect(titleInput()).toBeTruthy()
    cleanup()
    db.sermons.clear()
    await mount()
    expect(screen.getByText('Você ainda não tem sermões. Crie o primeiro.')).toBeTruthy()
  })
})
