<script lang="ts">
  import { tick, untrack } from 'svelte'
  import NodeView from './NodeView.svelte'
  import type { ApiError, ChapterDto } from './reader'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    hasActiveBible: boolean
    bibleTitle: string | null
    title: string | null
    bookName: string | null
    chapter: ChapterDto | null
    loading: boolean
    error: ApiError | null
    selectedVerse: number | null
    canPrevious: boolean
    canNext: boolean
    onSelectVerse: (verse: number) => void
    onPrevious: () => void
    onNext: () => void
  }

  let {
    t,
    hasActiveBible,
    bibleTitle,
    title,
    bookName,
    chapter,
    loading,
    error,
    selectedVerse,
    canPrevious,
    canNext,
    onSelectVerse,
    onPrevious,
    onNext
  }: Props = $props()

  let versesElement = $state<HTMLElement | null>(null)
  // Capítulos longos (como o Salmo 119) são montados em lotes para a primeira pintura ficar rápida.
  const BATCH = 20
  let rendered = $state(BATCH)

  $effect(() => {
    const total = chapter?.verses.length ?? 0
    const start = untrack(() => Math.min(total, Math.max(BATCH, (selectedVerse ?? 0) + 1)))
    rendered = start
    if (total <= start) return
    let frame = 0
    const grow = () => {
      rendered = Math.min(total, rendered + BATCH)
      if (rendered < total) frame = requestAnimationFrame(grow)
    }
    frame = requestAnimationFrame(grow)
    return () => cancelAnimationFrame(frame)
  })

  $effect(() => {
    const wanted = (selectedVerse ?? 0) + 1
    untrack(() => {
      if (wanted > rendered) rendered = wanted
    })
  })

  const shown = $derived(chapter ? chapter.verses.slice(0, Math.max(rendered, BATCH)) : [])

  const isEmpty = $derived(
    !!chapter && chapter.verses.every((verse) => verse.nodes.length === 0)
  )

  const errorKey = $derived.by(() => {
    switch (error?.code) {
      case 'book_not_in_module':
        return 'errorBookMissing'
      case 'module_protected':
        return 'errorProtected'
      case 'module_corrupted':
        return 'errorCorrupted'
      case 'module_not_found':
        return 'errorModuleMissing'
      case 'verse_not_found':
        return 'errorIncomplete'
      case 'module_unsupported':
        return 'errorUnsupported'
      default:
        return 'errorChapter'
    }
  })

  function focusVerse(number: number) {
    const element = versesElement?.querySelector<HTMLElement>(`[data-verse="${number}"]`)
    element?.focus()
    if (typeof element?.scrollIntoView === 'function') element.scrollIntoView({ block: 'nearest' })
  }

  let pendingFocus: number | null = null

  function select(number: number) {
    pendingFocus = number
    onSelectVerse(number)
    void tick().then(tick).then(applyPendingFocus)
  }

  function applyPendingFocus() {
    if (pendingFocus === null) return
    const element = versesElement?.querySelector<HTMLElement>(`[data-verse="${pendingFocus}"]`)
    if (!element) return
    pendingFocus = null
    focusVerse(Number(element.dataset.verse))
  }

  // Foco pedido para um versículo que ainda estava sendo montado.
  $effect(() => {
    void rendered
    void selectedVerse
    void tick().then(applyPendingFocus)
  })

  function onVerseKeydown(event: KeyboardEvent, number: number) {
    if (event.target !== event.currentTarget || !chapter) return
    const last = chapter.verses[chapter.verses.length - 1]?.number ?? number
    let target: number | null = null
    if (event.key === 'ArrowDown') target = Math.min(number + 1, last)
    else if (event.key === 'ArrowUp') target = Math.max(number - 1, 1)
    else if (event.key === 'Home') target = chapter.verses[0]?.number ?? 1
    else if (event.key === 'End') target = last
    else if (event.key === 'Enter' || event.key === ' ') target = number
    if (target === null) return
    event.preventDefault()
    select(target)
  }

  // Ao chegar em um capítulo com versículo selecionado, leva-o à vista.
  $effect(() => {
    if (chapter && selectedVerse !== null) {
      void tick().then(() => {
        const element = versesElement?.querySelector<HTMLElement>(`[data-verse="${selectedVerse}"]`)
        if (typeof element?.scrollIntoView === 'function') element.scrollIntoView({ block: 'nearest' })
      })
    }
  })
</script>

<section class="bible-pane" aria-label={t('readingArea')}>
  {#if hasActiveBible && title}
    <header class="pane-header">
      <button
        class="nav-button"
        aria-label={t('previousChapter')}
        title={`${t('previousChapter')} (Ctrl+←)`}
        disabled={!canPrevious}
        onclick={onPrevious}
      >‹</button>
      <div class="pane-title">
        <h2>{title}</h2>
        {#if bibleTitle}<p>{t('activeBible', { title: bibleTitle })}</p>{/if}
      </div>
      <button
        class="nav-button"
        aria-label={t('nextChapter')}
        title={`${t('nextChapter')} (Ctrl+→)`}
        disabled={!canNext}
        onclick={onNext}
      >›</button>
    </header>
  {/if}

  <div class="pane-body">
    {#if !hasActiveBible}
      <div class="empty-reading">
        <div class="empty-book" aria-hidden="true">▤</div>
        <h2>{t('selectBible')}</h2>
      </div>
    {:else if loading}
      <p class="pane-message" role="status">{t('chapterLoading')}</p>
    {:else if error}
      <div class="pane-message error" role="alert">{t(errorKey, { book: bookName ?? '' })}</div>
    {:else if chapter && isEmpty}
      <p class="pane-message" role="status">{t('chapterEmpty')}</p>
    {:else if chapter}
      {#if chapter.incomplete}
        <p class="pane-notice" role="status">{t('chapterIncomplete')}</p>
      {/if}
      <div class="verses bible-text" bind:this={versesElement} role="listbox" aria-label={t('verseList')}>
        {#each shown as verse, index (verse.number)}
          {@const selected = selectedVerse === verse.number}
          <div
            class="verse"
            class:selected
            role="option"
            aria-selected={selected}
            aria-setsize={chapter.verses.length}
            aria-posinset={index + 1}
            tabindex={selected || (selectedVerse === null && verse.number === chapter.verses[0]?.number) ? 0 : -1}
            data-verse={verse.number}
            onclick={() => onSelectVerse(verse.number)}
            onkeydown={(event) => onVerseKeydown(event, verse.number)}
          >
            <sup class="verse-number">{verse.number}</sup>
            <NodeView nodes={verse.nodes} {t} />
          </div>
        {/each}
      </div>
    {/if}
  </div>
</section>
