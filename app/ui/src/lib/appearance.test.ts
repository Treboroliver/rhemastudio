import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  addBibleColumn,
  applyTheme,
  bibleForShortcut,
  contrastRatio,
  MAX_BIBLE_FONT_SIZE,
  MIN_BIBLE_FONT_SIZE,
  removeBibleColumn,
  resizeBibleFont,
  restoreBibleColumns,
  withSettings
} from './appearance'
import { type AppSettings, type ModuleInfo } from './modules'

const baseSettings = (): AppSettings => ({
  schema_version: 2,
  module_folders: [],
  default_bible_id: null,
  selected_bible_ids: [],
  interface_language: 'pt-BR',
  theme: 'dark',
  show_strong_numbers: false,
  bible_font_size: 20,
  first_use_completed: false
})

function bible(id: string, state: ModuleInfo['state'] = 'Ready'): ModuleInfo {
  return {
    id,
    path: `C:\\synthetic\\${id}.ont`,
    kind: 'Bible',
    state,
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

describe('parallel Bible settings', () => {
  it('adds and removes columns while enforcing the four-column limit', () => {
    const four = ['a', 'b', 'c', 'd'].reduce((ids, id) => addBibleColumn(ids, id), [] as string[])
    expect(four).toEqual(['a', 'b', 'c', 'd'])
    expect(addBibleColumn(four, 'e')).toBe(four)
    expect(removeBibleColumn(four, 'b')).toEqual(['a', 'c', 'd'])
  })

  it('restores only ready, non-protected Bibles and keeps the saved order', () => {
    const modules = [bible('a'), bible('b'), bible('locked', 'Protected'), bible('commentary')]
    expect(restoreBibleColumns(['b', 'locked', 'a', 'a'], modules, 'b')).toEqual(['b', 'a'])
    expect(restoreBibleColumns([], modules, 'locked')).toEqual([])
  })

  it('maps Alt+1..4 to the selected column and rejects other keys', () => {
    expect(['1', '2', '3', '4'].map((key) => bibleForShortcut(['a', 'b', 'c', 'd'], key)))
      .toEqual(['a', 'b', 'c', 'd'])
    expect(bibleForShortcut(['a'], '4')).toBeNull()
    expect(bibleForShortcut(['a'], '5')).toBeNull()
  })
})

describe('reading appearance settings', () => {
  it('clamps font size, resets to default, and persists the setting version', () => {
    expect(resizeBibleFont(MIN_BIBLE_FONT_SIZE, -1)).toBe(MIN_BIBLE_FONT_SIZE)
    expect(resizeBibleFont(MAX_BIBLE_FONT_SIZE, 1)).toBe(MAX_BIBLE_FONT_SIZE)
    expect(resizeBibleFont(18, 0)).toBe(20)
    const settings = withSettings(baseSettings(), { bible_font_size: 22 })
    expect(settings).toMatchObject({ schema_version: 2, bible_font_size: 22 })
  })

  it('persists Strong visibility and theme changes without losing other settings', () => {
    const settings = baseSettings()
    expect(withSettings(settings, { show_strong_numbers: true }).show_strong_numbers).toBe(true)
    expect(withSettings(settings, { theme: 'light' }).theme).toBe('light')
    expect(withSettings(settings, { theme: 'auto' }).theme).toBe('auto')
    applyTheme('light', document.documentElement)
    expect(document.documentElement.dataset.theme).toBe('light')
    applyTheme('dark', document.documentElement)
    expect(document.documentElement.dataset.theme).toBe('dark')
    applyTheme('auto', document.documentElement)
    expect(document.documentElement.dataset.theme).toBe('auto')
  })
})

describe('theme contrast tokens', () => {
  it('keeps every semantic reading and interface text pair above 4.5:1', async () => {
    const css = await readFile(join(process.cwd(), 'src', 'app.css'), 'utf8')
    const colorDeclarations = css
      .split(/\r?\n/)
      .filter((line) => /#[\da-fA-F]{6}|rgba?\(/.test(line))
    expect(colorDeclarations.every((line) => line.includes('--'))).toBe(true)
    const blocks = new Map<string, Record<string, string>>()
    const selectorPattern = /(:root(?:\[data-theme="(?:light|auto)"\])?)\s*\{([^}]*)\}/g
    for (const match of css.matchAll(selectorPattern)) {
      const tokens = Object.fromEntries(
        [...match[2].matchAll(/(--[\w-]+):\s*(#[\da-fA-F]{6});/g)].map((token) => [token[1], token[2]])
      )
      const theme = match[1] === ':root'
        ? 'dark'
        : match[1].includes('light')
          ? 'light'
          : 'auto-light'
      blocks.set(theme, tokens)
    }

    const pairs = [
      ['--text', '--bg'],
      ['--text-strong', '--bg'],
      ['--text', '--selected-bg'],
      ['--text-strong', '--card'],
      ['--muted', '--panel'],
      ['--muted', '--card'],
      ['--accent-text', '--bg'],
      ['--accent-text', '--selected-bg'],
      ['--strong-underline', '--bg'],
      ['--strong-underline', '--selected-bg'],
      ['--jesus', '--bg'],
      ['--jesus', '--selected-bg'],
      ['--quote', '--bg'],
      ['--quote', '--card'],
      ['--warning', '--panel'],
      ['--warning', '--bg'],
      ['--warning-subtle', '--panel'],
      ['--warning-subtle', '--card'],
      ['--error', '--error-bg'],
      ['--primary-text', '--accent-deep'],
      ['--primary-text', '--accent-hover']
    ]
    expect(css).toMatch(/\.strong-word:hover,\s*\.strong-word:focus-visible/)
    expect(css).toMatch(/\.strong-word\.show-strong\s*\{\s*text-decoration:\s*underline dashed/)
    for (const theme of ['dark', 'light', 'auto-light']) {
      const tokens = blocks.get(theme)
      expect(tokens, `missing CSS tokens for ${theme}`).toBeDefined()
      for (const [foreground, background] of pairs) {
        const ratio = contrastRatio(tokens![foreground], tokens![background])
        expect(ratio, `${theme}: ${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5)
      }
    }
  })
})
