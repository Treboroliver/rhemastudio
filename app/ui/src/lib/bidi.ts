import type { ModuleInfo } from './modules'
import type { AlignedVerseDto, UiNode } from './reader'

export type TextScript = 'heb' | 'grc' | 'other'

export interface ScriptRun {
  text: string
  script: TextScript
}

const CANTILLATION_RE = /[\u0591-\u05AF]/g
const HEBREW_CHAR_RE = /[\u0590-\u05FF]/
const GREEK_CHAR_RE = /[\u0370-\u03FF\u1F00-\u1FFF]/
const ORIGINAL_SCRIPT_RE = /[\u0370-\u03FF\u1F00-\u1FFF\u0590-\u05FF]/

/** Remove sinais de cantilação hebraica (te'amim U+0591–U+05AF), preservando vogais (niqqud). */
export function stripCantillation(text: string): string {
  return CANTILLATION_RE.test(text) ? text.replace(CANTILLATION_RE, '') : text
}

function charScript(ch: string): TextScript {
  if (HEBREW_CHAR_RE.test(ch)) return 'heb'
  if (GREEK_CHAR_RE.test(ch)) return 'grc'
  return 'other'
}

/**
 * Divide um texto em trechos contíguos por escrita (hebraico, grego, outro) para isolar
 * bidirecionalidade (`<bdi dir="rtl">`) e aplicar fontes sem alterar parágrafos inteiros.
 */
export function splitScriptRuns(text: string, showCantillation = false): ScriptRun[] {
  const clean = showCantillation ? text : stripCantillation(text)
  if (!ORIGINAL_SCRIPT_RE.test(clean)) {
    return [{ text: clean, script: 'other' }]
  }

  const runs: ScriptRun[] = []
  let currentText = ''
  let currentScript: TextScript | null = null

  const chars = Array.from(clean)
  for (let i = 0; i < chars.length; i++) {
    const ch = chars[i]
    let script = charScript(ch)
    // Mantém espaço/hífen/maqaf dentro do mesmo trecho se cercado pela mesma escrita original
    if (script === 'other' && (currentScript === 'heb' || currentScript === 'grc') && (ch === ' ' || ch === '-' || ch === '־')) {
      const next = chars[i + 1]
      if (next && charScript(next) === currentScript) {
        script = currentScript
      }
    }
    if (currentScript === null) {
      currentScript = script
      currentText = ch
    } else if (script === currentScript) {
      currentText += ch
    } else {
      runs.push({ text: currentText, script: currentScript })
      currentScript = script
      currentText = ch
    }
  }
  if (currentText && currentScript !== null) {
    runs.push({ text: currentText, script: currentScript })
  }
  return runs
}

function collectSampleText(nodes: UiNode[] | null | undefined, out: string[]): void {
  if (!nodes || out.length >= 20) return
  for (const node of nodes) {
    if (out.length >= 20) return
    if (node.type === 'text') {
      if (node.value.trim()) out.push(node.value)
    } else if ('children' in node && Array.isArray(node.children)) {
      collectSampleText(node.children, out)
    }
  }
}

/** Detecta a escrita predominante de uma coluna bíblica (config `language` com fallback por conteúdo). */
export function detectColumnScript(
  module: ModuleInfo,
  rows: AlignedVerseDto[] | undefined,
  columnIndex: number
): TextScript {
  const lang = (module.language ?? '').trim().toLowerCase()
  if (/^(heb|he|iw|hbo|hebrew|hebraico)$/.test(lang)) return 'heb'
  if (/^(grc|grk|el|ell|greg|gre|greek|grego)$/.test(lang)) return 'grc'

  if (!rows?.length) return 'other'
  const samples: string[] = []
  for (let i = 0; i < Math.min(rows.length, 15); i++) {
    collectSampleText(rows[i].columns[columnIndex], samples)
  }
  const joined = samples.join(' ')
  let heb = 0
  let grc = 0
  let lat = 0
  for (const ch of joined) {
    const cp = ch.codePointAt(0) ?? 0
    if (cp >= 0x0590 && cp <= 0x05ff) heb++
    else if ((cp >= 0x0370 && cp <= 0x03ff) || (cp >= 0x1f00 && cp <= 0x1fff)) grc++
    else if ((cp >= 0x41 && cp <= 0x5a) || (cp >= 0x61 && cp <= 0x7a)) lat++
  }
  if (heb >= 10 && heb > grc && heb * 2 >= lat) return 'heb'
  if (grc >= 10 && grc > heb && grc * 2 >= lat) return 'grc'
  return 'other'
}
