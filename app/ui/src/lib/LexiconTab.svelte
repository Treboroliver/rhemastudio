<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import RichContent from './RichContent.svelte'
  import { asApiError } from './reader'
  import {
    formatLexField,
    lexEntry,
    lexIndexBuild,
    lexIndexStatus,
    lexSearch,
    lookupStrong,
    type LexEntryRequest,
    type LexIndexStatusDto,
    type LexProgressEvent,
    type LexSearchDto,
    type LexSearchHitDto,
    type LexiconEntryDto,
    type StrongRequest
  } from './study'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    request: StrongRequest | null
    entryRequest?: LexEntryRequest | null
    active?: boolean
    onVerse: (target: { book: number; chapter: number; verse: number }) => void
    onManageFolders: () => void
    onInsert?: (key: string) => void
    searchFn?: (query: string, limit?: number) => Promise<LexSearchDto>
    statusFn?: () => Promise<LexIndexStatusDto>
    buildFn?: () => Promise<LexIndexStatusDto>
    loadEntryFn?: (moduleId: string, key: string) => Promise<LexiconEntryDto>
    searchDelay?: number
  }

  interface Visit {
    key: string
    choices: string[]
    moduleId?: string
  }

  const HISTORY_LIMIT = 50
  const listId = $props.id()

  let {
    t,
    request,
    entryRequest = null,
    active = true,
    onVerse,
    onManageFolders,
    onInsert,
    searchFn = lexSearch,
    statusFn = lexIndexStatus,
    buildFn = lexIndexBuild,
    loadEntryFn = lexEntry,
    searchDelay = 120
  }: Props = $props()

  let visits = $state<Visit[]>([])
  let entry = $state<LexiconEntryDto | null>(null)
  let errorCode = $state<string | null>(null)
  let loading = $state(false)
  let sequence = 0

  let searchQuery = $state('')
  let searchResult = $state<LexSearchDto | null>(null)
  let searchErrorCode = $state<string | null>(null)
  let searchOpen = $state(false)
  let activeHit = $state(0)
  let searchTimer: ReturnType<typeof setTimeout> | undefined
  let searchSeq = 0

  let indexStatus = $state<LexIndexStatusDto | null>(null)
  let unlisten: UnlistenFn | null = null

  const current = $derived(visits.length ? visits[visits.length - 1] : null)

  const flatHits = $derived.by<LexSearchHitDto[]>(() => {
    if (!searchResult) return []
    return searchResult.groups.flatMap((g) => g.hits)
  })

  const statusText = $derived.by(() => {
    if (!indexStatus) return ''
    switch (indexStatus.state) {
      case 'ready':
        return t('lexIndexReady')
      case 'building':
        return t('lexIndexBuilding', { percent: indexStatus.percent })
      case 'rebuild_needed':
        return t('lexIndexRebuildNeeded')
      case 'empty':
        return t('lexIndexEmpty')
      default:
        return t('lexIndexError')
    }
  })

  async function refreshStatus() {
    try {
      indexStatus = await statusFn()
    } catch {
      // Em testes unitários sem backend Tauri, mantém silencioso
    }
  }

  async function rebuildIndex() {
    try {
      indexStatus = {
        state: 'building',
        stage: 'links',
        module_index: 0,
        module_count: 1,
        percent: 0,
        module_id: null,
        entries_processed: 0,
        message: null
      }
      indexStatus = await buildFn()
    } catch (cause) {
      indexStatus = {
        state: 'error',
        stage: null,
        module_index: 0,
        module_count: 0,
        percent: 0,
        module_id: null,
        entries_processed: 0,
        message: asApiError(cause).message
      }
    }
  }

  async function load(key: string, moduleId?: string) {
    const id = ++sequence
    loading = true
    entry = null
    errorCode = null
    try {
      const found = moduleId ? await loadEntryFn(moduleId, key) : await lookupStrong(key)
      if (id !== sequence) return
      entry = found
    } catch (cause) {
      if (id !== sequence) return
      errorCode = asApiError(cause).code
    } finally {
      if (id === sequence) loading = false
    }
  }

  function visit(key: string, choices: string[], moduleId?: string) {
    if (current?.key === key && current?.moduleId === moduleId) {
      visits = [...visits.slice(0, -1), { key, choices, moduleId }]
      return void load(key, moduleId)
    }
    visits = [...visits, { key, choices, moduleId }].slice(-HISTORY_LIMIT)
    void load(key, moduleId)
  }

  function choose(key: string) {
    if (!current || current.key === key) return
    visits = [...visits.slice(0, -1), { key, choices: current.choices }]
    void load(key)
  }

  function back() {
    if (visits.length < 2) return
    visits = visits.slice(0, -1)
    const prev = visits[visits.length - 1]
    void load(prev.key, prev.moduleId)
  }

  function openHit(hit: LexSearchHitDto) {
    searchOpen = false
    if (hit.kind === 'strong' || /^[GH]\d+$/i.test(hit.key)) {
      visit(hit.key, [hit.key])
    } else {
      visit(hit.key, [hit.key], hit.module_id)
    }
  }

  async function runSearch(q: string) {
    const id = ++searchSeq
    const trimmed = q.trim()
    if (!trimmed) {
      searchResult = null
      searchErrorCode = null
      searchOpen = false
      return
    }
    try {
      const res = await searchFn(trimmed, 30)
      if (id !== searchSeq) return
      searchResult = res
      searchErrorCode = null
      searchOpen = true
      activeHit = 0
    } catch (cause) {
      if (id !== searchSeq) return
      searchResult = null
      searchErrorCode = asApiError(cause).code
      searchOpen = true
    }
  }

  function onSearchInput(event: Event) {
    clearTimeout(searchTimer)
    const q = (event.currentTarget as HTMLInputElement | null)?.value ?? searchQuery ?? ''
    searchQuery = q
    if (!q.trim()) {
      searchResult = null
      searchErrorCode = null
      searchOpen = false
      return
    }
    searchTimer = setTimeout(() => void runSearch(q), searchDelay)
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      if (searchOpen) {
        event.preventDefault()
        event.stopPropagation()
        searchOpen = false
      }
    } else if (event.key === 'ArrowDown' && flatHits.length) {
      event.preventDefault()
      searchOpen = true
      activeHit = (activeHit + 1) % flatHits.length
    } else if (event.key === 'ArrowUp' && flatHits.length) {
      event.preventDefault()
      searchOpen = true
      activeHit = (activeHit - 1 + flatHits.length) % flatHits.length
    } else if (event.key === 'Enter' && flatHits.length && searchOpen) {
      event.preventDefault()
      const hit = flatHits[Math.min(activeHit, flatHits.length - 1)]
      if (hit) openHit(hit)
    }
  }

  function hitIndex(hit: LexSearchHitDto): number {
    return flatHits.indexOf(hit)
  }

  $effect(() => {
    const incoming = request
    if (!incoming || !incoming.ids.length) return
    untrack(() => visit(incoming.ids[0], incoming.ids))
  })

  $effect(() => {
    const incoming = entryRequest
    if (!incoming) return
    untrack(() => visit(incoming.key, [incoming.key], incoming.moduleId))
  })

  onMount(() => {
    const hasTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
    if (active && (hasTauri || statusFn !== lexIndexStatus)) {
      void refreshStatus()
    }
    if (!hasTauri) return
    void (async () => {
      try {
        unlisten = await listen<LexProgressEvent>('lex-index-progress', (ev) => {
          const p = ev.payload
          indexStatus = {
            state: p.state as LexIndexStatusDto['state'],
            stage: p.stage,
            module_index: p.module_index,
            module_count: p.module_count,
            percent: p.percent,
            module_id: p.module_id,
            entries_processed: p.entries_processed,
            message: null
          }
        })
      } catch {
        // Sem runtime Tauri (ex.: vitest)
      }
    })()
  })

  onDestroy(() => {
    clearTimeout(searchTimer)
    unlisten?.()
  })

  const actions = {
    onVerse: (target: { book: number; chapter: number; verse: number }) => onVerse(target),
    onEntry: (id: string) => visit(id, [id])
  }
</script>

<div class="lexicon-tab">
  <div class="lex-toolbar">
    {#if indexStatus}
      <div class="lex-index-status" role="status">
        <span>{statusText}</span>
        <button
          type="button"
          class="quiet compact-control"
          disabled={indexStatus.state === 'building'}
          onclick={() => void rebuildIndex()}
        >{t('lexRebuildButton')}</button>
      </div>
    {/if}
    <div class="lex-search-box">
      <input
        type="search"
        class="lex-search-input"
        bind:value={searchQuery}
        oninput={onSearchInput}
        onkeydown={onSearchKeydown}
        role="combobox"
        aria-label={t('lexSearchLabel')}
        aria-expanded={searchOpen && flatHits.length > 0}
        aria-controls={listId}
        aria-activedescendant={searchOpen && flatHits.length ? `${listId}-${activeHit}` : undefined}
        placeholder={t('lexSearchPlaceholder')}
        autocomplete="off"
        spellcheck="false"
      />
      {#if searchOpen}
        <div class="lex-search-results" id={listId} role="listbox" aria-label={t('lexSearchLabel')}>
          {#if searchErrorCode === 'lex_index_building'}
            <p class="panel-empty" role="status">{t('lexSearchBuilding')}</p>
          {:else if searchErrorCode === 'lex_rebuild_required'}
            <p class="panel-empty" role="status">{t('lexIndexRebuildNeeded')}</p>
          {:else if searchErrorCode}
            <p class="panel-empty" role="alert">{t('lexSearchError')}</p>
          {:else if searchResult && searchResult.groups.length === 0}
            <p class="panel-empty" role="status">{t('lexSearchEmpty')}</p>
          {:else if searchResult}
            {#each searchResult.groups as group (group.label)}
              <div class="lex-group">
                <p class="lex-group-title">{group.is_suggestions ? t('lexSuggestionsGroup') : group.label}</p>
                {#if group.is_suggestions}
                  {@const greek = group.hits.filter((h) => h.key.startsWith('G'))}
                  {@const hebrew = group.hits.filter((h) => h.key.startsWith('H'))}
                  <div class="lex-suggestions-split">
                    {#if greek.length}
                      <div class="lex-sug-col">
                        <span class="lex-sug-lang">{t('lexGreekGroup')}</span>
                        {#each greek as hit (hit.key)}
                          {@const idx = hitIndex(hit)}
                          <button
                            type="button"
                            id={`${listId}-${idx}`}
                            role="option"
                            aria-selected={idx === activeHit}
                            class="lex-hit"
                            class:active={idx === activeHit}
                            onclick={() => openHit(hit)}
                          >
                            <span class="lex-hit-key">{hit.key}</span>
                            {#if hit.dice !== null}
                              <span class="lex-hit-dice">{t('lexDice', { value: hit.dice.toFixed(2) })}</span>
                            {/if}
                          </button>
                        {/each}
                      </div>
                    {/if}
                    {#if hebrew.length}
                      <div class="lex-sug-col">
                        <span class="lex-sug-lang">{t('lexHebrewGroup')}</span>
                        {#each hebrew as hit (hit.key)}
                          {@const idx = hitIndex(hit)}
                          <button
                            type="button"
                            id={`${listId}-${idx}`}
                            role="option"
                            aria-selected={idx === activeHit}
                            class="lex-hit"
                            class:active={idx === activeHit}
                            onclick={() => openHit(hit)}
                          >
                            <span class="lex-hit-key">{hit.key}</span>
                            {#if hit.dice !== null}
                              <span class="lex-hit-dice">{t('lexDice', { value: hit.dice.toFixed(2) })}</span>
                            {/if}
                          </button>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {:else}
                  {#each group.hits as hit (`${hit.module_id}-${hit.key}-${hit.field}`)}
                    {@const idx = hitIndex(hit)}
                    <button
                      type="button"
                      id={`${listId}-${idx}`}
                      role="option"
                      aria-selected={idx === activeHit}
                      class="lex-hit"
                      class:active={idx === activeHit}
                      onclick={() => openHit(hit)}
                    >
                      <span class="lex-hit-key">{hit.key}</span>
                      <span class="lex-hit-meta">
                        {formatLexField(t, hit.field)}{hit.provenance ? ` · ${hit.provenance}` : ''}
                      </span>
                    </button>
                  {/each}
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if !current}
    <p class="panel-empty" role="status">{t('lexiconHint')}</p>
  {:else}
    <div class="lexicon-head">
      <button
        type="button"
        class="quiet compact-control"
        aria-label={t('lexiconBack')}
        title={t('lexiconBack')}
        disabled={visits.length < 2}
        onclick={back}
      >‹</button>
      {#if current.choices.length > 1}
        <div class="strong-choices" role="group" aria-label={t('lexiconChoices')}>
          {#each current.choices as choice (choice)}
            <button
              type="button"
              class="strong-choice"
              aria-pressed={choice === current.key}
              onclick={() => choose(choice)}
            >{choice}</button>
          {/each}
        </div>
      {:else}
        <span class="strong-key">{current.key}</span>
      {/if}
    </div>
    {#if current.choices.length > 1}
      <p class="panel-note">{t('lexiconChooseNote')}</p>
    {/if}

    {#if loading}
      <p class="panel-empty" role="status">{t('lexiconLoading')}</p>
    {:else if entry}
      <article class="lexicon-entry" aria-label={t('lexiconEntryLabel', { key: entry.key })}>
        <header>
          <h3>{entry.title}</h3>
          {#if entry.original_word}<p class="original-word" dir="auto">{entry.original_word}</p>{/if}
          <p class="panel-note">{entry.module_title}</p>
          {#if onInsert}
            <button type="button" class="compact-control" data-focus="insert-lexicon" onclick={() => onInsert(entry!.key)}>{t('insertIntoSermon')}</button>
          {/if}
        </header>
        {#if entry.degraded}<p class="panel-note" role="status">{t('contentDegraded')}</p>{/if}
        <RichContent nodes={entry.nodes} {t} {actions} />
      </article>
    {:else if errorCode === 'no_lexicon_installed'}
      <div class="panel-empty" role="status">
        <p>{t('lexiconNone')}</p>
        <button type="button" class="primary" onclick={onManageFolders}>{t('manageFolders')}</button>
      </div>
    {:else if errorCode === 'entry_not_found'}
      <p class="panel-empty" role="status">{t('lexiconNotFound')}</p>
    {:else if errorCode === 'unsupported_format'}
      <p class="panel-empty" role="status">{t('lexiconUnsupported')}</p>
    {:else if errorCode}
      <div class="panel-empty" role="alert">
        <p>{t('lexiconError')}</p>
        <button type="button" class="quiet" onclick={() => current && load(current.key, current.moduleId)}>{t('retry')}</button>
      </div>
    {/if}
  {/if}
</div>
