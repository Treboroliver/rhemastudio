<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import type { ModuleInfo } from './modules'
  import {
    passageFromRange,
    prettyRange,
    asApiError,
    type Passage,
    type RefErrorDto,
    type ResolveDto
  } from './reader'
  import { formatLexField, type LexSearchDto, type LexSearchHitDto } from './study'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    bibles: ModuleInfo[]
    selectedBibleIds: string[]
    resolve: (text: string) => Promise<ResolveDto>
    onGo: (passage: Passage) => void
    onSelectBible: (module: ModuleInfo) => void
    onClose: () => void
    delay?: number
    lexDelay?: number
    lexSearch?: (query: string, limit?: number) => Promise<LexSearchDto>
    onSelectLexicon?: (hit: LexSearchHitDto) => void
  }

  let {
    t,
    bibles,
    selectedBibleIds,
    resolve,
    onGo,
    onSelectBible,
    onClose,
    delay = 30,
    lexDelay = 120,
    lexSearch,
    onSelectLexicon
  }: Props = $props()

  interface Choice {
    id: string
    label: string
    detail: string
    group?: string
    hit?: LexSearchHitDto
    run: () => void
  }

  const listId = $props.id()
  let text = $state('')
  let result = $state<ResolveDto | null>(null)
  let lexResult = $state<LexSearchDto | null>(null)
  let lexErrorCode = $state<string | null>(null)
  let active = $state(0)
  let input = $state<HTMLInputElement | null>(null)
  let dialog = $state<HTMLElement | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  let lexTimer: ReturnType<typeof setTimeout> | undefined
  let sequence = 0
  let lexSequence = 0
  const previousFocus = typeof document === 'undefined' ? null : document.activeElement

  const forcedLexQuery = $derived.by(() => {
    const trimmed = text.trimStart()
    if (/^l\s+/i.test(trimmed)) return trimmed.slice(2).trim()
    return null
  })

  const bibleMatch = $derived.by(() => {
    if (forcedLexQuery !== null) return null
    const query = text.trim().toLowerCase()
    if (!query) return null
    return bibles.find((module) => module.abbreviation?.toLowerCase() === query) ?? null
  })

  const interpretation = $derived(
    bibleMatch ? null : result?.ranges.length ? result.ranges.map((r) => prettyRange(r.label)).join('; ') : null
  )

  const choices = $derived.by<Choice[]>(() => {
    if (bibleMatch) {
      return [
        {
          id: 'bible',
          label: bibleMatch.title,
          detail: t(
            selectedBibleIds.includes(bibleMatch.id)
              ? 'activateBibleColumn'
              : selectedBibleIds.length >= 4
                ? 'parallelLimit'
                : 'addBibleColumn'
          ),
          run: () => onSelectBible(bibleMatch)
        }
      ]
    }
    const list: Choice[] = []
    if (result) {
      for (const range of result.ranges) {
        list.push({
          id: `range-${list.length}`,
          label: prettyRange(range.label),
          detail: t('goTo'),
          run: () => onGo(passageFromRange(range))
        })
      }
      for (const item of result.suggestions) {
        const target = item.range
        list.push({
          id: `suggestion-${list.length}`,
          label: target ? prettyRange(target.label) : item.name,
          detail: item.abbreviation,
          run: () => onGo(target ? passageFromRange(target) : { book: item.book, chapter: 1, verse: null })
        })
      }
    }
    if (list.length === 0 && lexResult && onSelectLexicon) {
      for (const group of lexResult.groups) {
        const groupLabel = group.is_suggestions ? t('lexSuggestionsGroup') : group.label
        for (const hit of group.hits) {
          const diceText = hit.dice !== null ? ` · ${t('lexDice', { value: hit.dice.toFixed(2) })}` : ''
          const prov = hit.provenance && !group.is_suggestions ? ` · ${hit.provenance}` : ''
          const detail = group.is_suggestions
            ? `${formatLexField(t, hit.field)}${diceText}`
            : `${formatLexField(t, hit.field)}${prov}`
          list.push({
            id: `lex-${list.length}-${hit.module_id}-${hit.key}`,
            label: hit.key,
            detail,
            group: groupLabel,
            hit,
            run: () => onSelectLexicon(hit)
          })
        }
      }
    }
    return list
  })

  const isLexMode = $derived(
    forcedLexQuery !== null ||
      (!bibleMatch &&
        !(result?.ranges.length ?? 0) &&
        !(result?.suggestions.length ?? 0) &&
        (lexResult !== null || lexErrorCode !== null))
  )

  const hasSuggestions = $derived((result?.suggestions.length ?? 0) > 0)

  const messages = $derived.by(() => {
    if (lexErrorCode === 'lex_index_building') return [t('lexSearchBuilding')]
    if (lexErrorCode === 'lex_rebuild_required') return [t('lexIndexRebuildNeeded')]
    if (lexErrorCode) return [t('lexSearchError')]
    if (isLexMode && lexResult && lexResult.groups.length === 0 && text.trim()) return [t('lexSearchEmpty')]
    if (!result || bibleMatch || isLexMode || !text.trim()) return []
    return result.errors
      .filter((error) => !(hasSuggestions && (error.code === 'unknown_book' || error.code === 'invalid_syntax')))
      .map(describe)
  })

  function describe(error: RefErrorDto): string {
    const book = error.book_name ?? ''
    const max = error.max ?? 0
    switch (error.code) {
      case 'invalid_chapter':
        return t(max === 1 ? 'refChapterSingle' : 'refChapterMax', { book, max })
      case 'invalid_verse':
        return t(max === 1 ? 'refVerseSingle' : 'refVerseMax', {
          book,
          chapter: error.chapter ?? 0,
          max
        })
      case 'unknown_book':
        return t('refUnknownBook', { text: error.text ?? '' })
      case 'invalid_range':
        return t('refInvalidRange')
      case 'input_too_long':
        return t('refTooLong')
      default:
        return t('refInvalidSyntax')
    }
  }

  async function runLex(query: string) {
    if (!lexSearch) return
    const current = ++lexSequence
    if (!query.trim()) {
      lexResult = null
      lexErrorCode = null
      return
    }
    try {
      const found = await lexSearch(query.trim(), 20)
      if (current === lexSequence) {
        lexResult = found
        lexErrorCode = null
        active = 0
      }
    } catch (cause) {
      if (current === lexSequence) {
        lexResult = null
        lexErrorCode = asApiError(cause).code
      }
    }
  }

  async function run(query: string) {
    const trimmed = query.trimStart()
    if (/^l\s+/i.test(trimmed)) {
      result = null
      clearTimeout(lexTimer)
      const lexQ = trimmed.slice(2).trim()
      lexTimer = setTimeout(() => void runLex(lexQ), lexDelay)
      return
    }
    const current = ++sequence
    if (!query.trim()) {
      result = null
      lexResult = null
      lexErrorCode = null
      return
    }
    try {
      const outcome = await resolve(query)
      if (current !== sequence) return
      result = outcome
      active = 0
      if (
        !bibleMatch &&
        outcome.ranges.length === 0 &&
        outcome.suggestions.length === 0 &&
        lexSearch
      ) {
        clearTimeout(lexTimer)
        lexTimer = setTimeout(() => void runLex(query), lexDelay)
      } else {
        clearTimeout(lexTimer)
        lexSequence++
        lexResult = null
        lexErrorCode = null
      }
    } catch {
      if (current === sequence) result = { ranges: [], errors: [{ code: 'invalid_syntax' }], suggestions: [] }
    }
  }

  function oninput() {
    clearTimeout(timer)
    clearTimeout(lexTimer)
    const query = text
    const trimmed = query.trimStart()
    if (/^l\s+/i.test(trimmed)) {
      result = null
      const lexQ = trimmed.slice(2).trim()
      lexTimer = setTimeout(() => void runLex(lexQ), lexDelay)
      return
    }
    timer = setTimeout(() => void run(query), delay)
  }

  async function confirm() {
    clearTimeout(timer)
    clearTimeout(lexTimer)
    if (choices.length === 0) return
    choices[Math.min(active, choices.length - 1)]?.run()
    onClose()
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onClose()
    } else if (event.key === 'ArrowDown' && choices.length) {
      event.preventDefault()
      active = (active + 1) % choices.length
    } else if (event.key === 'ArrowUp' && choices.length) {
      event.preventDefault()
      active = (active - 1 + choices.length) % choices.length
    } else if (event.key === 'Enter') {
      event.preventDefault()
      void confirm()
    } else if (event.key === 'Tab' && dialog) {
      const focusable = [
        ...dialog.querySelectorAll<HTMLElement>('input, button:not([disabled])')
      ]
      if (focusable.length <= 1) {
        event.preventDefault()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
  }

  onMount(() => {
    void tick().then(() => input?.focus())
  })

  onDestroy(() => {
    clearTimeout(timer)
    clearTimeout(lexTimer)
    if (previousFocus instanceof HTMLElement) previousFocus.focus()
  })
</script>

<div class="command-backdrop" role="presentation" onpointerdown={onClose}></div>
<div
  class="command-bar"
  role="dialog"
  aria-modal="true"
  aria-label={t('commandBar')}
  tabindex="-1"
  bind:this={dialog}
  {onkeydown}
>
  <input
    bind:this={input}
    bind:value={text}
    {oninput}
    type="text"
    class="command-input"
    role="combobox"
    aria-expanded={choices.length > 0}
    aria-controls={listId}
    aria-activedescendant={choices.length ? `${listId}-${active}` : undefined}
    aria-label={t('commandPlaceholder')}
    placeholder={t('commandPlaceholder')}
    autocomplete="off"
    spellcheck="false"
  />
  <p class="command-interpretation" role="status">
    {#if interpretation}{interpretation}{/if}
  </p>
  {#each messages as message}
    <p class="command-error" role="alert">{message}</p>
  {/each}
  <ul class="command-list" id={listId} role="listbox" aria-label={t('commandResults')}>
    {#each choices as choice, index (choice.id)}
      {#if choice.group && (index === 0 || choices[index - 1]?.group !== choice.group)}
        <li class="command-group-heading" role="presentation">{choice.group}</li>
      {/if}
      <li
        id={`${listId}-${index}`}
        role="option"
        aria-selected={index === active}
        class:active={index === active}
        onpointerdown={(event) => {
          event.preventDefault()
          active = index
          void confirm()
        }}
      >
        <span>{choice.label}</span>
        <span class="command-detail">{choice.detail}</span>
      </li>
    {/each}
  </ul>
  <p class="command-hint">{t('commandHint')}</p>
</div>
