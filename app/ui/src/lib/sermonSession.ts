import {
  asSermonError,
  MAX_BLOCKS,
  sermonGet,
  sermonSave,
  type Block,
  type SermonDto
} from './sermon'

export interface ActiveSermon {
  id: string
  title: string
}

export interface InsertResult {
  title: string
  undo: () => Promise<void> | void
}

/** Editor aberto: recebe inserções direto na memória, sem passar pelo banco. */
export interface EditorHandle {
  id: string
  insert: (block: Block, baseText?: string) => Promise<InsertResult>
}

export interface BibleChoice {
  id: string
  abbreviation: string
  title: string
}

/** Ponte entre o sermão e o resto do app (leitura e léxico). */
export interface SermonBridge {
  bibles: BibleChoice[]
  activeBibleId: string | null
  goToPassage: (target: { book: number; chapter: number; verse: number }) => void
  openEntry: (strongId: string) => void
}

let active: ActiveSermon | null = null
let editor: EditorHandle | null = null

export const getActiveSermon = () => active
export const setActiveSermon = (value: ActiveSermon | null) => {
  active = value
}
export const clearActiveSermonIf = (id: string) => {
  if (active?.id === id) active = null
}

export function registerEditor(handle: EditorHandle): () => void {
  editor = handle
  return () => {
    if (editor === handle) editor = null
  }
}

export function resetSermonSession() {
  active = null
  editor = null
}

const limitError = () => ({ code: 'sermon_block_limit', message: `Limite de ${MAX_BLOCKS} blocos` })

/**
 * Insere um bloco no sermão: no editor aberto (depois do bloco focado) ou, sem editor,
 * no fim do sermão salvo. `baseText` só preenche o texto base quando ele está vazio.
 */
export async function insertBlockIntoSermon(
  id: string,
  block: Block,
  baseText?: string
): Promise<InsertResult> {
  if (editor && editor.id === id) {
    const result = await editor.insert(block, baseText)
    if (active?.id === id) active = { id, title: result.title }
    return result
  }
  let sermon: SermonDto
  try {
    sermon = await sermonGet(id)
  } catch (cause) {
    if (asSermonError(cause).code === 'sermon_not_found') clearActiveSermonIf(id)
    throw cause
  }
  if (sermon.blocks.length >= MAX_BLOCKS) throw limitError()
  const filled = !!baseText && !sermon.base_text.trim()
  const next: SermonDto = {
    ...sermon,
    base_text: filled ? baseText : sermon.base_text,
    blocks: [...sermon.blocks, block]
  }
  await sermonSave(next)
  active = { id, title: sermon.title }
  return {
    title: sermon.title,
    undo: async () => {
      const current = await sermonGet(id)
      await sermonSave({
        ...current,
        base_text: filled && current.base_text === baseText ? sermon.base_text : current.base_text,
        blocks: current.blocks.filter((candidate) => candidate.id !== block.id)
      })
    }
  }
}
