import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  convertBlock,
  createBlock,
  insertItem,
  moveItem,
  referenceLabel,
  SaveQueue,
  withText,
  blockBytes,
  MAX_BLOCK_BYTES,
  type Block,
  type SaveStatus
} from './sermon'

describe('helpers', () => {
  it('moves and inserts without mutating', () => {
    const items = ['a', 'b', 'c']
    expect(moveItem(items, 0, 1)).toEqual(['b', 'a', 'c'])
    expect(moveItem(items, 2, 3)).toBe(items)
    expect(insertItem(items, 1, 'x')).toEqual(['a', 'x', 'b', 'c'])
    expect(insertItem(items, 99, 'x')).toEqual(['a', 'b', 'c', 'x'])
    expect(items).toEqual(['a', 'b', 'c'])
  })

  it('converts between editable kinds keeping id and text', () => {
    const heading = { ...createBlock('heading'), type: 'heading', level: 3, text: 'Ideia' } as Block
    const note = convertBlock(heading, 'note')
    expect(note).toEqual({ type: 'note', id: heading.id, markdown: 'Ideia' })
    expect(convertBlock(note, 'heading')).toMatchObject({ type: 'heading', level: 2, text: 'Ideia' })
    const verse: Block = {
      type: 'verse',
      id: 'v',
      range: { start: { book: 'John', chapter: 1, verse: 1 }, end: { book: 'John', chapter: 1, verse: 1 } },
      module_id: 'm',
      module_abbr: 'XYZ',
      snapshot: 'texto inventado'
    }
    expect(convertBlock(verse, 'text')).toBe(verse)
    expect(withText(verse, 'outro')).toBe(verse)
  })

  it('formats verse references with the interface book names', () => {
    const books = [{ number: 43, abbreviation: 'Jo' }]
    const ref = (verse: number, endVerse: number, endChapter = 3) => ({
      start: { book: 'John', chapter: 3, verse },
      end: { book: 'John', chapter: endChapter, verse: endVerse }
    })
    expect(referenceLabel(ref(16, 16), books)).toBe('Jo 3:16')
    expect(referenceLabel(ref(16, 18), books)).toBe('Jo 3:16-18')
    expect(referenceLabel(ref(16, 2, 4), books)).toBe('Jo 3:16 – 4:2')
    expect(referenceLabel(ref(1, 1), [])).toBe('John 3:1')
  })

  it('measures block size in bytes', () => {
    const small = withText(createBlock('text'), 'ação')
    expect(blockBytes(small)).toBeGreaterThan(4)
    expect(blockBytes(withText(createBlock('text'), 'a'.repeat(MAX_BLOCK_BYTES)))).toBeGreaterThan(MAX_BLOCK_BYTES)
  })
})

describe('SaveQueue', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  function setup(save: () => Promise<void>) {
    const statuses: SaveStatus[] = []
    const queue = new SaveQueue({ save, onStatus: (status) => statuses.push(status) })
    return { queue, statuses }
  }

  it('waits 500 ms after the last change and coalesces edits', async () => {
    const save = vi.fn(async () => undefined)
    const { queue, statuses } = setup(save)
    queue.touch()
    await vi.advanceTimersByTimeAsync(400)
    queue.touch()
    await vi.advanceTimersByTimeAsync(400)
    expect(save).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(100)
    expect(save).toHaveBeenCalledTimes(1)
    expect(statuses.at(-1)).toBe('saved')
  })

  it('never runs two saves at once and saves again after changes made while saving', async () => {
    let release: () => void = () => undefined
    let running = 0
    let peak = 0
    const save = vi.fn(async () => {
      running += 1
      peak = Math.max(peak, running)
      if (save.mock.calls.length === 1) await new Promise<void>((resolve) => (release = resolve))
      running -= 1
    })
    const { queue } = setup(save)
    queue.touch()
    await vi.advanceTimersByTimeAsync(500)
    expect(save).toHaveBeenCalledTimes(1)
    queue.touch()
    await vi.advanceTimersByTimeAsync(500)
    expect(save).toHaveBeenCalledTimes(1)
    release()
    await vi.advanceTimersByTimeAsync(0)
    expect(save).toHaveBeenCalledTimes(2)
    expect(peak).toBe(1)
    expect(queue.pending).toBe(false)
  })

  it('keeps the change pending after an error and retries on flush', async () => {
    const save = vi.fn(async () => undefined)
    save.mockRejectedValueOnce(new Error('falha'))
    const { queue, statuses } = setup(save)
    queue.touch()
    await vi.advanceTimersByTimeAsync(500)
    expect(statuses.at(-1)).toBe('error')
    expect(queue.pending).toBe(true)
    expect(await queue.flush()).toBe(true)
    expect(save).toHaveBeenCalledTimes(2)
    expect(statuses.at(-1)).toBe('saved')
  })

  it('flush without changes does nothing and resolves true', async () => {
    const save = vi.fn(async () => undefined)
    const { queue } = setup(save)
    expect(await queue.flush()).toBe(true)
    expect(save).not.toHaveBeenCalled()
  })

  it('flush reports false while the save keeps failing', async () => {
    const { queue } = setup(async () => {
      throw new Error('falha')
    })
    queue.touch()
    expect(await queue.flush()).toBe(false)
  })
})
