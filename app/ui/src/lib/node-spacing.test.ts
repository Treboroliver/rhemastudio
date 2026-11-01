import { cleanup, render } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import NodeView from './NodeView.svelte'
import type { UiNode } from './reader'

afterEach(cleanup)

const t = (key: string) => key
const text = (value: string): UiNode => ({ type: 'text', value })
const strong = (value: string, num = 1): UiNode => ({ type: 'strong', lang: 'G', num, children: [text(value)] })
const note = (value: string): UiNode => ({ type: 'note', children: [text(value)] })

/** Texto que o leitor deve ver: o equivalente do plain_text, sem o conteúdo das notas. */
function plain(nodes: UiNode[]): string {
  return nodes
    .map((node) => {
      if (node.type === 'text') return node.value
      if (node.type === 'note' || node.type === 'paragraph' || node.type === 'lineBreak') return ''
      return plain(node.children)
    })
    .join('')
}

const trees: Record<string, UiNode[]> = {
  'Strong seguido de ponto e vírgula': [text('and the '), strong('darkness'), text('; and the')],
  'Strong seguido de vírgula': [strong('word'), text(', and')],
  'Strong aninhado': [{ type: 'strong', lang: 'H', num: 2, children: [strong('x', 3)] }, text('; y')],
  'nota após palavra': [text('word'), note('nota'), text('; and')],
  'nota entre palavras': [text('a '), note('nota'), text('b')],
  'itálico': [text('a '), { type: 'italic', children: [text('adicionada')] }, text('; b')],
  'palavras de Jesus': [text('a '), { type: 'wordsOfJesus', children: [text('fala'), strong('x')] }, text(', b')],
  'Jesus com nota': [{ type: 'wordsOfJesus', children: [text('fala'), note('n')] }, text('; b')],
  'citação, negrito e sublinhado': [
    { type: 'otQuote', children: [text('q')] },
    { type: 'bold', children: [text('b')] },
    { type: 'underline', children: [text('u')] },
    text('.')
  ],
  'link de versículo e verbete': [
    { type: 'verseLink', book: 43, chapter: 1, verse: 1, children: [text('Jo 1:1')] },
    text('; '),
    { type: 'entryLink', lang: 'G', num: 25, children: [text('G25')] },
    text(', fim')
  ],
  'quebra de linha': [text('a'), { type: 'lineBreak' }, text('b')]
}

describe('espaçamento da leitura', () => {
  for (const [name, nodes] of Object.entries(trees)) {
    it(`textContent == plain_text: ${name}`, () => {
      const { container } = render(NodeView, { nodes, t })
      const rendered = (container.textContent ?? '').replaceAll('†', '')
      expect(rendered).toBe(plain(nodes))
    })
  }
})
