<script lang="ts">
  import { untrack } from 'svelte'
  import RichContent from './RichContent.svelte'
  import type { ModuleInfo } from './modules'
  import { asApiError, type NodeActions, type UiNode } from './reader'
  import {
    COMMENTARY_FOLLOW_DELAY_MS,
    formatSize,
    getCommentaryEntry,
    getCommentaryIndex,
    type CommentaryHeaderDto,
    type CommentaryIndexDto,
    type CommentaryModuleDto
  } from './study'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    selection: { book: number; chapter: number; verse: number } | null
    commentaries: ModuleInfo[]
    activeIds: string[]
    /** Só consulta o índice quando a aba está visível. */
    active?: boolean
    onToggle: (id: string, enabled: boolean) => void
    onVerse: NonNullable<NodeActions['onVerse']>
    onEntry: NonNullable<NodeActions['onEntry']>
    onManageFolders: () => void
  }

  interface PageState {
    nodes: UiNode[]
    hasMore: boolean
    degraded?: boolean
    nextOffset: number
    loading: boolean
    errorCode: string | null
  }

  let { t, selection, commentaries, activeIds, active = true, onToggle, onVerse, onEntry, onManageFolders }: Props = $props()

  let index = $state<CommentaryIndexDto | null>(null)
  let indexError = $state<string | null>(null)
  let loading = $state(false)
  let open = $state<Record<string, boolean>>({})
  let pages = $state<Record<string, PageState>>({})
  let sequence = 0
  let generation = 0

  const actions: NodeActions = {
    onVerse: (target) => onVerse(target),
    onEntry: (id) => onEntry(id)
  }
  const entryKey = (module: CommentaryModuleDto, entry: CommentaryHeaderDto) =>
    `${module.module_id}:${entry.topic_id}`
  const totalEntries = $derived(index?.modules.reduce((sum, module) => sum + module.entries.length, 0) ?? 0)
  const hasModuleErrors = $derived(index?.modules.some((module) => module.error) ?? false)

  async function fetchIndex(book: number, chapter: number, verse: number) {
    const id = ++sequence
    loading = true
    indexError = null
    try {
      const loaded = await getCommentaryIndex(book, chapter, verse)
      if (id !== sequence) return
      generation += 1
      open = {}
      pages = {}
      index = loaded
    } catch (cause) {
      if (id !== sequence) return
      indexError = asApiError(cause).code
    } finally {
      if (id === sequence) loading = false
    }
  }

  $effect(() => {
    const target = selection
    const enabled = activeIds.join('|')
    void enabled
    if (!active) return
    if (!target || !activeIds.length) {
      sequence += 1
      index = null
      indexError = null
      loading = false
      return
    }
    const timer = setTimeout(
      () => untrack(() => void fetchIndex(target.book, target.chapter, target.verse)),
      COMMENTARY_FOLLOW_DELAY_MS
    )
    return () => clearTimeout(timer)
  })

  async function loadPage(module: CommentaryModuleDto, entry: CommentaryHeaderDto, offset: number) {
    const key = entryKey(module, entry)
    const owner = generation
    const previous = pages[key]
    pages = {
      ...pages,
      [key]: {
        nodes: offset === 0 ? [] : (previous?.nodes ?? []),
        hasMore: false,
        nextOffset: offset,
        loading: true,
        errorCode: null
      }
    }
    try {
      const page = await getCommentaryEntry(module.module_id, entry.topic_id, offset)
      if (owner !== generation) return
      pages = {
        ...pages,
        [key]: {
          nodes: offset === 0 ? page.nodes : [...(previous?.nodes ?? []), ...page.nodes],
          hasMore: page.has_more,
          degraded: page.degraded || previous?.degraded,
          nextOffset: page.next_offset,
          loading: false,
          errorCode: null
        }
      }
    } catch (cause) {
      if (owner !== generation) return
      pages = {
        ...pages,
        [key]: {
          nodes: previous?.nodes ?? [],
          hasMore: previous?.hasMore ?? false,
          nextOffset: offset,
          loading: false,
          errorCode: asApiError(cause).code
        }
      }
    }
  }

  function toggle(module: CommentaryModuleDto, entry: CommentaryHeaderDto) {
    if (!entry.supported) return
    const key = entryKey(module, entry)
    const next = !open[key]
    open = { ...open, [key]: next }
    if (next && !pages[key]) void loadPage(module, entry, 0)
  }

  function moduleErrorText(code: string): string {
    return code === 'module_protected' ? t('errorProtected') : t('commentaryModuleError')
  }

  function retryIndex() {
    if (selection) void fetchIndex(selection.book, selection.chapter, selection.verse)
  }
</script>

<div class="commentary-tab">
  {#if commentaries.length === 0}
    <div class="panel-empty" role="status">
      <p>{t('commentaryNone')}</p>
      <button type="button" class="primary" onclick={onManageFolders}>{t('manageFolders')}</button>
    </div>
  {:else}
    {#if !activeIds.length}
      <p class="panel-empty" role="status">{t('commentaryNoneActive')}</p>
    {:else if !selection}
      <p class="panel-empty" role="status">{t('commentarySelectVerse')}</p>
    {:else if indexError}
      <div class="panel-empty" role="alert">
        <p>{t('commentaryError')}</p>
        <button type="button" class="quiet" onclick={retryIndex}>{t('retry')}</button>
      </div>
    {:else if !index}
      <p class="panel-empty" role="status">{t('commentaryLoading')}</p>
    {:else}
      <div class="commentary-results" aria-busy={loading}>
        {#if totalEntries === 0 && !hasModuleErrors}
          <p class="panel-empty" role="status">{t('commentaryEmpty')}</p>
        {/if}
        {#each index.modules as module (module.module_id)}
          {#if module.error || module.entries.length}
            <section class="commentary-module" aria-label={module.module_title}>
              <h3>{module.module_title}</h3>
              {#if module.error}
                <p class="entry-notice" role="status">{moduleErrorText(module.error.code)}</p>
              {/if}
              {#each module.entries as entry (entry.topic_id)}
                {@const key = entryKey(module, entry)}
                {@const page = pages[key]}
                <div class="commentary-entry" class:unsupported={!entry.supported}>
                  <button
                    type="button"
                    class="entry-toggle"
                    aria-expanded={entry.supported ? !!open[key] : undefined}
                    aria-controls={entry.supported ? `entry-${key}` : undefined}
                    aria-disabled={!entry.supported}
                    onclick={() => toggle(module, entry)}
                  >
                    <span class="entry-scope">{entry.label}</span>
                    {#if entry.size_bytes}<span class="entry-size">{formatSize(entry.size_bytes)}</span>{/if}
                  </button>
                  {#if !entry.supported}
                    <p class="entry-notice" role="status">{t('commentaryUnsupported')}</p>
                  {:else if open[key]}
                    <div class="entry-body" id={`entry-${key}`}>
                      {#if page}
                        {#if page.nodes.length}
                          <RichContent nodes={page.nodes} {t} {actions} />
                        {/if}
                        {#if page.degraded && !page.loading}
                          <p class="panel-note" role="status">{t('contentDegraded')}</p>
                        {/if}
                        {#if page.loading}
                          <p class="panel-note" role="status">{t('commentaryLoading')}</p>
                        {:else if page.errorCode === 'unsupported_format'}
                          <p class="entry-notice" role="status">{t('commentaryUnsupported')}</p>
                        {:else if page.errorCode}
                          <p class="entry-notice" role="alert">{t('commentaryError')}</p>
                          <button type="button" class="quiet" onclick={() => loadPage(module, entry, page.nextOffset)}>{t('retry')}</button>
                        {:else if page.hasMore}
                          <button type="button" class="quiet show-more" onclick={() => loadPage(module, entry, page.nextOffset)}>{t('commentaryMore')}</button>
                        {/if}
                      {/if}
                    </div>
                  {/if}
                </div>
              {/each}
            </section>
          {/if}
        {/each}
      </div>
    {/if}

    <fieldset class="module-picker">
      <legend>{t('commentaryModules')}</legend>
      {#each commentaries as module (module.id)}
        <label>
          <input
            type="checkbox"
            checked={activeIds.includes(module.id)}
            onchange={(event) => onToggle(module.id, event.currentTarget.checked)}
          />
          <span>{module.title}</span>
        </label>
      {/each}
    </fieldset>
  {/if}
</div>
