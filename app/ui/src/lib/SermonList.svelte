<script lang="ts">
  import { onMount } from 'svelte'
  import { clearActiveSermonIf } from './sermonSession'
  import {
    asSermonError,
    formatDate,
    sermonArchive,
    sermonCreate,
    sermonDelete,
    sermonDuplicate,
    sermonList,
    type SermonSummaryDto
  } from './sermon'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    language: string
    onOpen: (id: string) => void
  }

  let { t, language, onOpen }: Props = $props()

  let items = $state<SermonSummaryDto[]>([])
  let loading = $state(true)
  let errorCode = $state<string | null>(null)
  let actionError = $state<string | null>(null)
  let query = $state('')
  let showArchived = $state(false)
  let menuId = $state<string | null>(null)
  let confirmDelete = $state<SermonSummaryDto | null>(null)
  let sequence = 0

  async function load() {
    const current = ++sequence
    loading = true
    try {
      const result = await sermonList(showArchived, query)
      if (current !== sequence) return
      items = result
      errorCode = null
    } catch (cause) {
      if (current !== sequence) return
      errorCode = asSermonError(cause).code
    } finally {
      if (current === sequence) loading = false
    }
  }

  onMount(() => void load())

  async function run(action: () => Promise<unknown>) {
    actionError = null
    menuId = null
    try {
      await action()
      await load()
    } catch (cause) {
      actionError = asSermonError(cause).code === 'sermon_unavailable' ? t('sermonUnavailable') : t('sermonActionError')
    }
  }

  async function create() {
    actionError = null
    try {
      const created = await sermonCreate(t('sermonUntitled'))
      onOpen(created.id)
    } catch (cause) {
      actionError = asSermonError(cause).code === 'sermon_unavailable' ? t('sermonUnavailable') : t('sermonActionError')
    }
  }

  async function confirmRemoval() {
    const target = confirmDelete
    confirmDelete = null
    if (target) {
      await run(() => sermonDelete(target.id))
      clearActiveSermonIf(target.id)
    }
  }

  function onMenuKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && menuId) {
      event.stopPropagation()
      const trigger = (event.currentTarget as HTMLElement).querySelector<HTMLElement>('[data-menu-trigger]')
      menuId = null
      trigger?.focus()
    }
  }
</script>

<section class="sermon-list" aria-label={t('sermonListLabel')}>
  <div class="sermon-toolbar">
    <button type="button" class="primary-action" onclick={() => void create()}>{t('sermonNew')}</button>
    <input
      type="search"
      class="sermon-search"
      aria-label={t('sermonSearch')}
      placeholder={t('sermonSearch')}
      bind:value={query}
      oninput={() => void load()}
    />
    <label class="sermon-check">
      <input type="checkbox" bind:checked={showArchived} onchange={() => void load()} />
      {t('sermonShowArchived')}
    </label>
  </div>

  {#if actionError}
    <p class="sermon-banner" role="alert">{actionError}</p>
  {/if}

  {#if loading && !items.length}
    <p class="panel-empty" role="status">{t('sermonLoading')}</p>
  {:else if errorCode === 'sermon_unavailable'}
    <div class="panel-empty" role="alert"><p>{t('sermonUnavailable')}</p></div>
  {:else if errorCode}
    <div class="panel-empty" role="alert">
      <p>{t('sermonListError')}</p>
      <button type="button" class="compact-control" onclick={() => void load()}>{t('sermonRetry')}</button>
    </div>
  {:else if !items.length}
    <p class="panel-empty">{query.trim() ? t('sermonNoResults') : t('sermonEmpty')}</p>
  {:else}
    <ul class="sermon-items">
      {#each items as item (item.id)}
        <li class="sermon-item" class:archived={item.archived}>
          <button type="button" class="sermon-open" onclick={() => onOpen(item.id)}>
            <span class="sermon-title">{item.title}</span>
            {#if item.base_text}<span class="sermon-base">{item.base_text}</span>{/if}
            <span class="sermon-meta">
              {formatDate(item.updated_at, language)} · {t('sermonBlocks', { count: item.block_count })}{item.archived
                ? ` · ${t('sermonArchivedTag')}`
                : ''}
            </span>
          </button>
          <!-- O Esc é tratado nos itens do menu; o contêiner só repassa o evento. -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="sermon-menu" onkeydown={onMenuKeydown}>
            <button
              type="button"
              class="compact-control"
              data-menu-trigger
              aria-haspopup="menu"
              aria-expanded={menuId === item.id}
              aria-label={t('sermonActionsFor', { title: item.title })}
              onclick={() => (menuId = menuId === item.id ? null : item.id)}
            >⋯</button>
            {#if menuId === item.id}
              <div class="sermon-menu-list" role="menu" aria-label={t('sermonActionsFor', { title: item.title })}>
                <button type="button" role="menuitem" onclick={() => onOpen(item.id)}>{t('sermonOpen')}</button>
                <button type="button" role="menuitem" onclick={() => void run(() => sermonDuplicate(item.id))}>{t('sermonDuplicate')}</button>
                <button type="button" role="menuitem" onclick={() => void run(() => sermonArchive(item.id, !item.archived))}>
                  {item.archived ? t('sermonUnarchive') : t('sermonArchive')}
                </button>
                <button type="button" role="menuitem" onclick={() => { menuId = null; confirmDelete = item }}>{t('sermonDelete')}</button>
              </div>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if confirmDelete}
    <div class="sermon-confirm" role="alertdialog" aria-label={t('sermonDeleteTitle')}>
      <p>{t('sermonDeleteBody', { title: confirmDelete.title })}</p>
      <div class="sermon-confirm-actions">
        <button type="button" class="danger-action" onclick={() => void confirmRemoval()}>{t('sermonDeleteConfirm')}</button>
        <button type="button" class="compact-control" onclick={() => (confirmDelete = null)}>{t('cancel')}</button>
      </div>
    </div>
  {/if}
</section>
