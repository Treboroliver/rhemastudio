<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onDestroy, onMount, untrack } from 'svelte'
  import type { RefErrorDto, ResolveDto } from './reader'
  import { asSermonError, getVerseSnapshot, type VerseSnapshotDto } from './sermon'
  import type { BibleChoice } from './sermonSession'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    language: string
    bibles: BibleChoice[]
    activeBibleId: string | null
    initialText?: string
    onInsert: (snapshot: VerseSnapshotDto, moduleId: string) => void
    onCancel: () => void
  }

  const RESOLVE_DELAY_MS = 30

  let { t, language, bibles, activeBibleId, initialText = '', onInsert, onCancel }: Props = $props()

  let text = $state(untrack(() => initialText))
  let bibleIndex = $state(
    Math.max(
      0,
      untrack(() => bibles.findIndex((bible) => bible.id === activeBibleId))
    )
  )
  let errors = $state<RefErrorDto[]>([])
  let preview = $state<VerseSnapshotDto | null>(null)
  let previewError = $state<string | null>(null)
  let loading = $state(false)
  let input = $state<HTMLInputElement | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  let sequence = 0
  let lastRange: ResolveDto['ranges'][number] | null = null

  const bible = $derived(bibles[bibleIndex] ?? null)

  onMount(() => {
    input?.focus()
    input?.setSelectionRange(text.length, text.length)
    if (text.trim()) schedule()
  })
  onDestroy(() => clearTimeout(timer))

  function describe(error: RefErrorDto): string {
    const book = error.book_name ?? ''
    const max = error.max ?? 0
    switch (error.code) {
      case 'invalid_chapter':
        return t(max === 1 ? 'refChapterSingle' : 'refChapterMax', { book, max })
      case 'invalid_verse':
        return t(max === 1 ? 'refVerseSingle' : 'refVerseMax', { book, chapter: error.chapter ?? 0, max })
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

  function snapshotMessage(cause: unknown): string {
    const error = asSermonError(cause)
    if (error.code === 'range_too_large') return t('sermonVerseTooLong')
    if (error.code === 'book_not_in_module') return t('sermonVerseBookMissing')
    if (error.code === 'verse_not_found') return t('sermonVerseMissing')
    if (error.code === 'module_protected') return t('errorProtected')
    return t('sermonVerseError')
  }

  function schedule() {
    clearTimeout(timer)
    timer = setTimeout(() => void resolve(), RESOLVE_DELAY_MS)
  }

  async function resolve() {
    const current = ++sequence
    const query = text.trim()
    if (!query) {
      errors = []
      preview = null
      previewError = null
      loading = false
      lastRange = null
      return
    }
    loading = true
    try {
      const result = await invoke<ResolveDto>('resolve_reference', { text: query, lang: language })
      if (current !== sequence) return
      errors = result.errors
      lastRange = result.ranges[0] ?? null
    } catch {
      if (current !== sequence) return
      errors = [{ code: 'invalid_syntax' }]
      lastRange = null
    }
    await loadPreview(current)
  }

  async function loadPreview(current: number) {
    preview = null
    previewError = null
    if (!lastRange || !bible) {
      loading = false
      return
    }
    try {
      const snapshot = await getVerseSnapshot(bible.id, lastRange)
      if (current !== sequence) return
      preview = snapshot
    } catch (cause) {
      if (current !== sequence) return
      previewError = snapshotMessage(cause)
    } finally {
      if (current === sequence) loading = false
    }
  }

  function onInput(event: Event) {
    text = (event.currentTarget as HTMLInputElement).value
    ++sequence
    preview = null
    previewError = null
    loading = !!text.trim()
    schedule()
  }

  function cycleBible() {
    if (bibles.length < 2) return
    bibleIndex = (bibleIndex + 1) % bibles.length
    const current = ++sequence
    loading = !!lastRange
    void loadPreview(current)
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    } else if (event.key === 'Tab' && !event.shiftKey && bibles.length > 1) {
      event.preventDefault()
      cycleBible()
    } else if (event.key === 'Enter' && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault()
      event.stopPropagation()
      if (preview && bible) onInsert(preview, bible.id)
    }
  }

  const messages = $derived(text.trim() ? errors.map(describe) : [])
</script>

<div class="sermon-verse-box" role="group" aria-label={t('sermonVerseBoxLabel')}>
  <label class="sermon-field">
    <span>{t('sermonVerseRefLabel')}</span>
    <input
      type="text"
      class="sermon-input"
      data-focus="verse-ref"
      placeholder={t('sermonVerseRefPlaceholder')}
      autocomplete="off"
      bind:this={input}
      value={text}
      oninput={onInput}
      onkeydown={onKeydown}
    />
  </label>
  {#if bible}
    <p class="panel-note" role="status" aria-live="polite">
      {t('sermonVerseBible', { abbr: bible.abbreviation })}
      {#if bibles.length > 1}<span> · {t('sermonVerseTabHint')}</span>{/if}
    </p>
  {:else}
    <p class="panel-note" role="status">{t('sermonVerseNoBible')}</p>
  {/if}
  {#each messages as message (message)}
    <p class="sermon-verse-error" role="alert">{message}</p>
  {/each}
  {#if previewError}
    <p class="sermon-verse-error" role="alert">{previewError}</p>
  {:else if loading}
    <p class="panel-note" role="status">{t('sermonVerseLoading')}</p>
  {:else if preview}
    <div class="sermon-verse-preview" aria-label={t('sermonVersePreview')}>
      <p class="sermon-ref">{preview.reference_label} · {preview.module_abbr}</p>
      <p class="sermon-snapshot">{preview.text}</p>
      <p class="panel-note">{t('sermonVerseEnterHint')}</p>
    </div>
  {:else if !text.trim()}
    <p class="panel-note">{t('sermonVerseRefHint')}</p>
  {/if}
</div>
