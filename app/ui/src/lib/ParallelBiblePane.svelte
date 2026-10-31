<script lang="ts">
  import { tick, untrack } from 'svelte'
  import NodeView from './NodeView.svelte'
  import type { ModuleInfo } from './modules'
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
    activeBibleId: string | null
    canPrevious: boolean
    canNext: boolean
    fontSize: number
    showStrong: boolean
    canAdd: boolean
    onAdd: () => void
    onRemove: (id: string) => void
    onActivate: (id: string) => void
    onSelectVerse: (verse: number) => void
    onPrevious: () => void
    onNext: () => void
    onFontSize: (direction: -1 | 0 | 1) => void
    onToggleStrong: () => void
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
    activeBibleId,
    canPrevious,
    canNext,
    fontSize,
    showStrong,
    canAdd,
    onAdd,
    onRemove,
    onActivate,
    onSelectVerse,
    onPrevious,
    onNext,
    onFontSize,
    onToggleStrong
  }: Props = $props()

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
    row?.focus()
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
    <button class="nav-button" aria-label={t('previousChapter')} title={`${t('previousChapter')} (Ctrl+←)`} disabled={!canPrevious} onclick={onPrevious}>‹</button>
    <div class="pane-title"><h2>{title ?? t('selectBible')}</h2></div>
    <button class="nav-button" aria-label={t('nextChapter')} title={`${t('nextChapter')} (Ctrl+→)`} disabled={!canNext} onclick={onNext}>›</button>
    <div class="reading-controls">
      <button class="quiet compact-control" aria-label={t('decreaseFont')} title="Ctrl+-" disabled={fontSize <= 14} onclick={() => onFontSize(-1)}>A−</button>
      <button class="quiet compact-control" aria-label={t('resetFont')} title="Ctrl+0" onclick={() => onFontSize(0)}>Aa {fontSize}</button>
      <button class="quiet compact-control" aria-label={t('increaseFont')} title="Ctrl+=" disabled={fontSize >= 32} onclick={() => onFontSize(1)}>A+</button>
      <label class="strong-toggle"><input type="checkbox" checked={showStrong} onchange={onToggleStrong} />{t('showStrongNumbers')}</label>
      <button class="primary add-column" aria-label={t('addBible')} disabled={!canAdd || columns.length >= 4} onclick={onAdd}>+</button>
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
                <button class="column-activate" aria-pressed={module.id === activeBibleId} onclick={() => activate(module.id)}>{module.title}</button>
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
                <div class="parallel-cell" class:missing-verse={nodes === null || nodes === undefined} aria-label={nodes === null || nodes === undefined ? t('verseMissing') : undefined}>
                  {#if nodes}
                    <sup class="verse-number">{row.number}</sup>
                    <NodeView {nodes} {t} showStrong={showStrong} />
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
