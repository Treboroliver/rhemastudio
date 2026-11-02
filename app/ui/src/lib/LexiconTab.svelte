<script lang="ts">
  import { untrack } from 'svelte'
  import RichContent from './RichContent.svelte'
  import { asApiError } from './reader'
  import { lookupStrong, type LexiconEntryDto, type StrongRequest } from './study'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    request: StrongRequest | null
    onVerse: (target: { book: number; chapter: number; verse: number }) => void
    onManageFolders: () => void
    onInsert?: (key: string) => void
  }

  interface Visit {
    key: string
    choices: string[]
  }

  const HISTORY_LIMIT = 50

  let { t, request, onVerse, onManageFolders, onInsert }: Props = $props()

  let visits = $state<Visit[]>([])
  let entry = $state<LexiconEntryDto | null>(null)
  let errorCode = $state<string | null>(null)
  let loading = $state(false)
  let sequence = 0

  const current = $derived(visits.length ? visits[visits.length - 1] : null)

  async function load(key: string) {
    const id = ++sequence
    loading = true
    entry = null
    errorCode = null
    try {
      const found = await lookupStrong(key)
      if (id !== sequence) return
      entry = found
    } catch (cause) {
      if (id !== sequence) return
      errorCode = asApiError(cause).code
    } finally {
      if (id === sequence) loading = false
    }
  }

  function visit(key: string, choices: string[]) {
    if (current?.key === key) {
      visits = [...visits.slice(0, -1), { key, choices }]
      return void load(key)
    }
    visits = [...visits, { key, choices }].slice(-HISTORY_LIMIT)
    void load(key)
  }

  function choose(key: string) {
    if (!current || current.key === key) return
    visits = [...visits.slice(0, -1), { key, choices: current.choices }]
    void load(key)
  }

  function back() {
    if (visits.length < 2) return
    visits = visits.slice(0, -1)
    void load(visits[visits.length - 1].key)
  }

  $effect(() => {
    const incoming = request
    if (!incoming || !incoming.ids.length) return
    untrack(() => visit(incoming.ids[0], incoming.ids))
  })

  const actions = {
    onVerse: (target: { book: number; chapter: number; verse: number }) => onVerse(target),
    onEntry: (id: string) => visit(id, [id])
  }
</script>

<div class="lexicon-tab">
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
        <button type="button" class="quiet" onclick={() => current && load(current.key)}>{t('retry')}</button>
      </div>
    {/if}
  {/if}
</div>
