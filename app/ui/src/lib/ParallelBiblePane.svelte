<script lang="ts">
  import { tick, untrack } from 'svelte'
  import NodeView from './NodeView.svelte'
  import { GREEK_FONT_OPTIONS, HEBREW_FONT_OPTIONS } from './appearance'
  import { detectColumnScript } from './bidi'
  import type { AppSettings, ModuleInfo } from './modules'
  import type { AlignedChaptersDto, ApiError } from './reader'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    columns: ModuleInfo[]
    aligned: AlignedChaptersDto | null
    title: string | null
    bookName: string | null
    loading: boolean
    error: ApiError | null
    selectedVerse: number | null
    onInsert?: () => void
    activeBibleId: string | null
    canPrevious: boolean
    canNext: boolean
    fontSize: number
    showStrong: boolean
    highlightJesusWords: boolean
    showCantillation?: boolean
    greekFont?: string
    hebrewFont?: string
    theme: AppSettings['theme']
    canAdd: boolean
    onAdd: () => void
    onRemove: (id: string) => void
    onActivate: (id: string) => void
    onSelectVerse: (verse: number) => void
    onPrevious: () => void
    onNext: () => void
    onFontSize: (direction: -1 | 0 | 1) => void
    onToggleStrong: () => void
    onToggleJesusWords: () => void
    onToggleCantillation?: () => void
    onGreekFontChange?: (font: string) => void
    onHebrewFontChange?: (font: string) => void
    onThemeChange: (theme: AppSettings['theme']) => void
    onStrong?: (ids: string[], element: HTMLElement, morph?: string | null) => void
  }

  let {
    t,
    columns,
    aligned,
    title,
    bookName,
    loading,
    error,
    selectedVerse,
    onInsert,
    activeBibleId,
    canPrevious,
    canNext,
    fontSize,
    showStrong,
    highlightJesusWords,
    showCantillation = false,
    greekFont = 'auto',
    hebrewFont = 'auto',
    theme,
    canAdd,
    onAdd,
    onRemove,
    onActivate,
    onSelectVerse,
    onPrevious,
    onNext,
    onFontSize,
    onToggleStrong,
    onToggleJesusWords,
    onToggleCantillation,
    onGreekFontChange,
    onHebrewFontChange,
    onThemeChange,
    onStrong
  }: Props = $props()

  const nodeActions = {
    onStrong: (ids: string[], element: HTMLElement, morph?: string | null) =>
      onStrong?.(ids, element, morph)
  }

  let viewportWidth = $state(typeof window === 'undefined' ? 1200 : window.innerWidth)
  let rendered = $state(20)
  let rowsElement = $state<HTMLElement | null>(null)
  let pendingFocus: number | null = null
  const BATCH_SIZE = 20
  const narrow = $derived(viewportWidth < 900)
  const visibleColumns = $derived.by(() => {
    if (!narrow || columns.length <= 2) return columns.map((module, index) => ({ module, index }))
    const activeIndex = columns.findIndex((module) => module.id === activeBibleId)
    const ordered = activeIndex < 0
      ? columns.map((_, index) => index)
      : [activeIndex, ...columns.map((_, index) => index).filter((index) => index !== activeIndex)]
    return ordered.slice(0, 2).map((index) => ({ module: columns[index], index }))
  })
  const shownRows = $derived(aligned?.rows.slice(0, rendered) ?? [])
  const columnScripts = $derived(
    columns.map((module, index) => detectColumnScript(module, aligned?.rows, index))
  )

  $effect(() => {
    const total = aligned?.rows.length ?? 0
    const start = untrack(() => Math.min(total, Math.max(BATCH_SIZE, (selectedVerse ?? 0) + 1)))
    rendered = start
    if (total <= start) return
    let frame = 0
    const grow = () => {
      rendered = Math.min(total, rendered + BATCH_SIZE)
      if (rendered < total) frame = requestAnimationFrame(grow)
    }
    frame = requestAnimationFrame(grow)
    return () => cancelAnimationFrame(frame)
  })

  $effect(() => {
    const wanted = (selectedVerse ?? 0) + 1
    untrack(() => {
      if (wanted > rendered) rendered = Math.min(aligned?.rows.length ?? wanted, wanted)
    })
  })

  function columnError(index: number): string | null {
    const error = aligned?.columns[index]?.error
    if (!error) return null
    if (error.code === 'module_protected') return t('errorProtected')
    if (error.code === 'book_not_in_module') return t('errorBookMissing', { book: bookName ?? '' })
    if (error.code === 'module_corrupted') return t('errorCorrupted')
    if (error.code === 'module_unsupported') return t('errorUnsupported')
    if (error.code === 'module_not_found') return t('errorModuleMissing')
    return t('errorChapter')
  }

  function globalErrorMessage(): string {
    if (error?.code === 'module_protected') return t('errorProtected')
    if (error?.code === 'book_not_in_module') return t('errorBookMissing', { book: bookName ?? '' })
    if (error?.code === 'module_corrupted') return t('errorCorrupted')
    if (error?.code === 'module_unsupported') return t('errorUnsupported')
    if (error?.code === 'module_not_found') return t('errorModuleMissing')
    return t('errorChapter')
  }

  function activate(id: string) {
    onActivate(id)
    void tick()
  }

  function focusRow(number: number) {
    const row = rowsElement?.querySelector<HTMLElement>(`[data-verse="${number}"]`)
    const focused = document.activeElement
    // Não tira o foco de uma palavra (Strong) ou nota que acabou de receber um clique.
    const keepFocus = !!focused && focused !== row && !!rowsElement?.contains(focused)
    if (!keepFocus) row?.focus()
    if (typeof row?.scrollIntoView === 'function') row.scrollIntoView({ block: 'nearest' })
    if (pendingFocus === number) pendingFocus = null
  }

  function selectVerse(number: number) {
    pendingFocus = number
    onSelectVerse(number)
    void tick().then(() => focusRow(number))
  }

  function onRowKeydown(event: KeyboardEvent, number: number) {
    if (event.target !== event.currentTarget || !aligned) return
    const first = aligned.rows[0]?.number ?? number
    const last = aligned.rows[aligned.rows.length - 1]?.number ?? number
    let target: number | null = null
    if (event.key === 'ArrowDown') target = Math.min(number + 1, last)
    else if (event.key === 'ArrowUp') target = Math.max(number - 1, first)
    else if (event.key === 'Home') target = first
    else if (event.key === 'End') target = last
    else if (event.key === 'Enter' || event.key === ' ') target = number
    if (target === null) return
    event.preventDefault()
    selectVerse(target)
  }

  $effect(() => {
    void rendered
    const target = pendingFocus ?? selectedVerse
    if (target !== null) void tick().then(() => focusRow(target))
  })
</script>

<svelte:window bind:innerWidth={viewportWidth} />

<section class="bible-pane parallel-pane" aria-label={t('readingArea')}>
  <header class="pane-header parallel-toolbar">
    <div class="chapter-controls" role="group" aria-label={t('chapterNavigation')}>
      <button class="nav-button" aria-label={t('previousChapter')} title={`${t('previousChapter')} (Ctrl+←)`} disabled={!canPrevious} onclick={onPrevious}>‹</button>
      <div class="pane-title"><h2 title={title ?? t('selectBible')}>{title ?? t('selectBible')}</h2></div>
      <button class="nav-button" aria-label={t('nextChapter')} title={`${t('nextChapter')} (Ctrl+→)`} disabled={!canNext} onclick={onNext}>›</button>
    </div>
    <div class="reading-controls">
      {#if onInsert && selectedVerse !== null}
        <button type="button" class="compact-control insert-verse" title={`${t('insertIntoSermon')} (Ctrl+Enter)`} onclick={onInsert}>{t('insertIntoSermon')}</button>
      {/if}
      <details class="reading-menu">
        <summary class="compact-control settings-trigger" aria-label={t('readingSettings')} title={t('readingSettings')}>Aa</summary>
        <div class="reading-menu-popover" role="region" aria-label={t('readingSettings')}>
          <div class="font-controls" role="group" aria-label={t('fontSize')}>
            <button class="quiet compact-control" aria-label={t('decreaseFont')} title="Ctrl+-" disabled={fontSize <= 14} onclick={() => onFontSize(-1)}>A−</button>
            <button class="quiet compact-control" aria-label={t('resetFont')} title="Ctrl+0" onclick={() => onFontSize(0)}>{fontSize} px</button>
            <button class="quiet compact-control" aria-label={t('increaseFont')} title="Ctrl+=" disabled={fontSize >= 32} onclick={() => onFontSize(1)}>A+</button>
          </div>
          <label class="menu-setting">
            <input type="checkbox" checked={showStrong} onchange={onToggleStrong} />
            {t('showStrongNumbers')}
          </label>
          <label class="menu-setting">
            <input type="checkbox" checked={highlightJesusWords} onchange={onToggleJesusWords} />
            {t('highlightWordsOfJesus')}
          </label>
          {#if onToggleCantillation}
            <label class="menu-setting">
              <input type="checkbox" checked={showCantillation} onchange={onToggleCantillation} />
              {t('showCantillation')}
            </label>
          {/if}
          {#if onGreekFontChange}
            <label class="theme-setting">
              <span>{t('greekFont')}</span>
              <select
                aria-label={t('greekFont')}
                value={greekFont}
                onchange={(event) => onGreekFontChange?.(event.currentTarget.value)}
              >
                {#each GREEK_FONT_OPTIONS as opt (opt)}
                  <option value={opt}>{opt === 'auto' ? t('fontAuto') : opt}</option>
                {/each}
              </select>
            </label>
          {/if}
          {#if onHebrewFontChange}
            <label class="theme-setting">
              <span>{t('hebrewFont')}</span>
              <select
                aria-label={t('hebrewFont')}
                value={hebrewFont}
                onchange={(event) => onHebrewFontChange?.(event.currentTarget.value)}
              >
                {#each HEBREW_FONT_OPTIONS as opt (opt)}
                  <option value={opt}>{opt === 'auto' ? t('fontAuto') : opt}</option>
                {/each}
              </select>
            </label>
          {/if}
          <label class="theme-setting">
            <span>{t('theme')}</span>
            <select
              aria-label={t('theme')}
              value={theme}
              onchange={(event) => {
                const value = event.currentTarget.value
                if (value === 'dark' || value === 'light' || value === 'auto') onThemeChange(value)
              }}
            >
              <option value="dark">{t('themeDark')}</option>
              <option value="light">{t('themeLight')}</option>
              <option value="auto">{t('themeAuto')}</option>
            </select>
          </label>
        </div>
      </details>
      <span class="add-column-wrap" title={columns.length >= 4 ? t('parallelLimit') : undefined}>
        <button
          class="primary add-column"
          aria-label={t('addBible')}
          aria-describedby={columns.length >= 4 ? 'parallel-limit-hint' : undefined}
          title={columns.length >= 4 ? t('parallelLimit') : t('addBible')}
          disabled={!canAdd || columns.length >= 4}
          onclick={onAdd}
        >{t('addBibleButton')}</button>
        {#if columns.length >= 4}<span id="parallel-limit-hint" class="sr-only">{t('parallelLimit')}</span>{/if}
      </span>
    </div>
  </header>

  <div class="pane-body parallel-body">
    {#if narrow && columns.length > 2}<p class="pane-notice narrow-notice" role="status">{t('narrowColumnsNotice')}</p>{/if}
    {#if loading}
      <p class="pane-message" role="status">{t('chapterLoading')}</p>
    {:else if error}
      <p class="pane-message error" role="alert">{globalErrorMessage()}</p>
    {:else if columns.length === 0}
      <p class="pane-message" role="status">{t('selectBible')}</p>
    {:else if aligned}
      <div class="parallel-grid" style={`--column-count:${visibleColumns.length};--bible-font-size:${fontSize}px`}>
        <div class="parallel-headers">
          {#each visibleColumns as { module, index } (module.id)}
            <div class="parallel-head-cell">
              <div class="parallel-column-header" class:active-column={module.id === activeBibleId}>
                <button
                  class="column-activate"
                  aria-label={t('activateBibleColumn', { title: module.title, shortcut: `Alt+${index + 1}` })}
                  aria-pressed={module.id === activeBibleId}
                  title={module.title}
                  onclick={() => activate(module.id)}
                >
                  <span class="column-abbreviation">{module.abbreviation ?? module.title}</span>
                  <span class="column-shortcut" aria-hidden="true">Alt+{index + 1}</span>
                </button>
                <button class="remove-column" aria-label={t('removeBible', { title: module.title })} title={t('removeBible', { title: module.title })} onclick={() => onRemove(module.id)}>×</button>
              </div>
              {#if columnError(index)}
                <p class="column-notice" role="alert">{columnError(index)}</p>
              {:else if aligned.columns[index]?.incomplete}
                <p class="column-notice" role="status">{t('chapterIncomplete')}</p>
              {/if}
            </div>
          {/each}
        </div>
        <div class="parallel-rows" bind:this={rowsElement} role="listbox" aria-label={t('verseList')}>
          {#each shownRows as row, rowIndex (row.number)}
            {@const selected = selectedVerse === row.number}
            <div
              class="parallel-row"
              class:selected
              role="option"
              aria-selected={selected}
              aria-setsize={aligned.rows.length}
              aria-posinset={rowIndex + 1}
              tabindex={selected || (selectedVerse === null && rowIndex === 0) ? 0 : -1}
              data-verse={row.number}
              onclick={() => selectVerse(row.number)}
              onkeydown={(event) => onRowKeydown(event, row.number)}
            >
              {#each visibleColumns as { module, index } (module.id)}
                {@const nodes = row.columns[index]}
                {@const script = columnScripts[index] ?? 'other'}
                <div
                  class="parallel-cell"
                  class:missing-verse={nodes === null || nodes === undefined}
                  class:script-heb={script === 'heb'}
                  class:script-grc={script === 'grc'}
                  dir={script === 'heb' ? 'rtl' : undefined}
                  aria-label={nodes === null || nodes === undefined ? t('verseMissing') : undefined}
                >
                  {#if nodes}
                    <sup class="verse-number">{row.number}</sup>
                    <NodeView {nodes} {t} showStrong={showStrong} {showCantillation} actions={nodeActions} />
                  {:else}
                    <span class="missing-verse-space" aria-hidden="true"></span>
                  {/if}
                </div>
              {/each}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>
