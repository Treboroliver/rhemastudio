import { describe, expect, it } from 'vitest'
import {
  History,
  nextChapter,
  passageFromRange,
  previousChapter,
  prettyRange,
  type BookInfo,
  type Passage
} from './reader'

const books = [
  { number: 42, chapters: 24 },
  { number: 43, chapters: 21 },
  { number: 44, chapters: 28 },
  { number: 57, chapters: 1 },
  { number: 58, chapters: 13 }
] as BookInfo[]

const at = (book: number, chapter: number, verse: number | null = null): Passage => ({
  book,
  chapter,
  verse
})

describe('navegação entre capítulos', () => {
  it('passa de um livro ao próximo', () => {
    expect(nextChapter(at(43, 21), books)).toEqual(at(44, 1))
    expect(nextChapter(at(43, 3, 16), books)).toEqual(at(43, 4))
  })

  it('passa de um livro de um capítulo só ao próximo livro', () => {
    expect(nextChapter(at(57, 1), books)).toEqual(at(58, 1))
  })

  it('volta ao último capítulo do livro anterior', () => {
    expect(previousChapter(at(44, 1), books)).toEqual(at(43, 21))
    expect(previousChapter(at(58, 1), books)).toEqual(at(57, 1))
  })

  it('não passa dos limites da lista', () => {
    expect(nextChapter(at(58, 13), books)).toBeNull()
    expect(previousChapter(at(1, 1), [{ number: 1, chapters: 50 }])).toBeNull()
    expect(nextChapter(at(99, 1), books)).toBeNull()
  })
})

describe('histórico', () => {
  it('volta e avança', () => {
    const history = new History()
    history.push(at(43, 1))
    history.push(at(43, 2))
    history.push(at(43, 3))
    expect(history.back()).toEqual(at(43, 2))
    expect(history.back()).toEqual(at(43, 1))
    expect(history.back()).toBeNull()
    expect(history.forward()).toEqual(at(43, 2))
  })

  it('descarta o futuro ao navegar depois de voltar', () => {
    const history = new History()
    history.push(at(43, 1))
    history.push(at(43, 2))
    history.back()
    history.push(at(44, 1))
    expect(history.canForward).toBe(false)
    expect(history.size).toBe(2)
  })

  it('ignora repetição da mesma passagem e atualiza o versículo atual', () => {
    const history = new History()
    history.push(at(43, 3))
    history.push(at(43, 3))
    expect(history.size).toBe(1)
    history.replaceCurrent(at(43, 3, 16))
    expect(history.current).toEqual(at(43, 3, 16))
  })

  it('guarda pelo menos 50 posições', () => {
    const history = new History()
    for (let chapter = 1; chapter <= 120; chapter += 1) history.push(at(19, chapter))
    expect(history.size).toBeGreaterThanOrEqual(50)
    let steps = 0
    while (history.back()) steps += 1
    expect(steps).toBeGreaterThanOrEqual(50)
  })
})

describe('faixas', () => {
  it('capítulo inteiro não seleciona versículo', () => {
    const point = { book: 43, chapter: 3, verse: 1 }
    const whole = { start: point, end: { ...point, verse: 36 }, label: '', short_label: '', whole: true }
    expect(passageFromRange(whole)).toEqual(at(43, 3))
    expect(passageFromRange({ ...whole, whole: false })).toEqual(at(43, 3, 1))
  })

  it('usa travessão entre números', () => {
    expect(prettyRange('João 3:16-18')).toBe('João 3:16–18')
  })
})
