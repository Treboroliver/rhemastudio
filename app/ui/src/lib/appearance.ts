import { isReady, type AppSettings, type ModuleInfo } from './modules'

export const MAX_PARALLEL_BIBLES = 4
export const MIN_BIBLE_FONT_SIZE = 14
export const MAX_BIBLE_FONT_SIZE = 32
export const DEFAULT_BIBLE_FONT_SIZE = 20

export function applyTheme(theme: AppSettings['theme'], root: HTMLElement): void {
  root.dataset.theme = theme
}

export function restoreBibleColumns(
  savedIds: string[] | undefined,
  modules: ModuleInfo[],
  defaultId: string | null
): string[] {
  const readyIds = new Set(
    modules.filter((module) => module.kind === 'Bible' && isReady(module)).map((module) => module.id)
  )
  const restored = [...new Set(savedIds ?? [])]
    .filter((id) => readyIds.has(id))
    .slice(0, MAX_PARALLEL_BIBLES)
  if (restored.length || !defaultId || !readyIds.has(defaultId)) return restored
  return [defaultId]
}

export function addBibleColumn(ids: string[], id: string): string[] {
  if (ids.includes(id) || ids.length >= MAX_PARALLEL_BIBLES) return ids
  return [...ids, id]
}

export function removeBibleColumn(ids: string[], id: string): string[] {
  return ids.filter((candidate) => candidate !== id)
}

export function bibleForShortcut(ids: string[], key: string): string | null {
  if (!/^[1-4]$/.test(key)) return null
  return ids[Number(key) - 1] ?? null
}

export function resizeBibleFont(size: number, direction: -1 | 0 | 1): number {
  if (direction === 0) return DEFAULT_BIBLE_FONT_SIZE
  return Math.max(MIN_BIBLE_FONT_SIZE, Math.min(MAX_BIBLE_FONT_SIZE, size + direction * 2))
}

export function contrastRatio(foreground: string, background: string): number {
  const luminance = (color: string) => {
    const channels = color.match(/[0-9a-f]{2}/gi)?.map((channel) => parseInt(channel, 16) / 255)
    if (!channels || channels.length !== 3) throw new Error(`Invalid hex color: ${color}`)
    const [red, green, blue] = channels.map((value) =>
      value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
    )
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue
  }
  const first = luminance(foreground)
  const second = luminance(background)
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05)
}

export function withSettings(settings: AppSettings, changes: Partial<AppSettings>): AppSettings {
  return { ...settings, ...changes, schema_version: 4 }
}
