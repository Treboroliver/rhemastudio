import { describe, expect, it, vi, afterEach } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import ParallelBiblePane from './ParallelBiblePane.svelte'
import RichContent from './RichContent.svelte'
import { applyOriginalFonts } from './appearance'
import { splitScriptRuns, stripCantillation } from './bidi'
import ptBR from '../i18n/pt-BR.json'
import type { ModuleInfo } from './modules'
import type { AlignedChaptersDto } from './reader'

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

const ptBible: ModuleInfo = {
  id: 'pt-1',
  path: '/mods/sint_pt.ont',
  kind: 'Bible',
  state: 'Ready',
  title: 'Sintética PT',
  abbreviation: 'SPT',
  language: 'pt',
  language_name: 'Português',
  description: null,
  version: null,
  size: 100,
  has_strong: false
}

const hebBible: ModuleInfo = {
  id: 'heb-1',
  path: '/mods/sint_heb.ot',
  kind: 'Bible',
  state: 'Ready',
  title: 'Sintética Hebraico',
  abbreviation: 'HEB',
  language: null,
  language_name: null,
  description: null,
  version: null,
  size: 100,
  has_strong: false
}

// Letras hebraicas inventadas com vogal (kamatz \u05B8) e cantilação (etnahta \u0591)
const HEB_WITH_CANT = 'א\u05B8\u0591ב\u05B8ג אבגד הוזח טילמ'
const HEB_NO_CANT = 'א\u05B8ב\u05B8ג אבגד הוזח טילמ'

const aligned: AlignedChaptersDto = {
  columns: [
    { module_id: 'pt-1', error: null, incomplete: false },
    { module_id: 'heb-1', error: null, incomplete: false }
  ],
  rows: [
    {
      number: 1,
      columns: [
        [{ type: 'text', value: 'texto sintetico em portugues' }],
        [{ type: 'text', value: HEB_WITH_CANT }]
      ]
    }
  ]
}

describe('RTL, isolamento bidi, cantilação e fontes (E3)', () => {
  afterEach(() => {
    cleanup()
  })

  it('isola trechos hebraicos com bdi dir="rtl" e gregos com script-grc sem inverter o parágrafo', () => {
    const runs = splitScriptRuns('palavra αβγ e אבג fim', false)
    expect(runs.map((r) => r.script)).toEqual(['other', 'grc', 'other', 'heb', 'other'])

    const { container } = render(RichContent, {
      props: {
        t,
        nodes: [{ type: 'text', value: 'nota αβγ e א\u05B8\u0591ב fim' }]
      }
    })
    const bdi = container.querySelector('bdi[dir="rtl"].script-heb')
    const grc = container.querySelector('span.script-grc')
    expect(bdi?.textContent).toBe('א\u05B8ב')
    expect(grc?.textContent).toBe('αβγ')
    expect(container.querySelector('p')?.getAttribute('dir')).toBeNull()
  })

  it('remove cantilação por padrão preservando vogais e respeita o interruptor', () => {
    expect(stripCantillation(HEB_WITH_CANT)).toBe(HEB_NO_CANT)

    const onToggleCantillation = vi.fn()
    const { container } = render(ParallelBiblePane, {
      props: {
        t,
        columns: [ptBible, hebBible],
        aligned,
        title: 'Gn 1',
        bookName: 'Gênesis',
        loading: false,
        error: null,
        selectedVerse: 1,
        activeBibleId: 'pt-1',
        canPrevious: false,
        canNext: true,
        fontSize: 20,
        showStrong: false,
        highlightJesusWords: true,
        showCantillation: false,
        theme: 'dark',
        canAdd: false,
        onAdd: vi.fn(),
        onRemove: vi.fn(),
        onActivate: vi.fn(),
        onSelectVerse: vi.fn(),
        onPrevious: vi.fn(),
        onNext: vi.fn(),
        onFontSize: vi.fn(),
        onToggleStrong: vi.fn(),
        onToggleJesusWords: vi.fn(),
        onToggleCantillation,
        onGreekFontChange: vi.fn(),
        onHebrewFontChange: vi.fn(),
        onThemeChange: vi.fn()
      }
    })

    const cells = container.querySelectorAll('.parallel-cell')
    expect(cells.length).toBe(2)
    expect(cells[0].getAttribute('dir')).not.toBe('rtl')
    expect(cells[1].getAttribute('dir')).toBe('rtl')
    expect(cells[1].classList.contains('script-heb')).toBe(true)
    // Cantilação (\u0591) oculta por padrão, vogal (\u05B8) preservada
    expect(cells[1].textContent).toContain('\u05B8')
    expect(cells[1].textContent).not.toContain('\u0591')

    const checkbox = screen.getByLabelText('Mostrar cantilação') as HTMLInputElement
    expect(checkbox.checked).toBe(false)
    void fireEvent.click(checkbox)
    expect(onToggleCantillation).toHaveBeenCalledTimes(1)
  })

  it('aplica pilha de fontes locais gregas e hebraicas', () => {
    const el = document.createElement('div')
    applyOriginalFonts('Cardo', 'SBL Hebrew', el)
    expect(el.style.getPropertyValue('--greek-font')).toContain("'Cardo'")
    expect(el.style.getPropertyValue('--hebrew-font')).toContain("'SBL Hebrew'")
  })
})
