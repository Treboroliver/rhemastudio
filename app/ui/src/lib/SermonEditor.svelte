<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import {
    asSermonError,
    blockBytes,
    blockText,
    convertBlock,
    createBlock,
    duplicateBlock,
    editableKinds,
    formatDate,
    insertItem,
    isEditable,
    MAX_BASE_TEXT_CHARS,
    MAX_BLOCK_BYTES,
    MAX_BLOCKS,
    MAX_TITLE_CHARS,
    moveItem,
    referenceLabel,
    SaveQueue,
    sermonGet,
    sermonRestore,
    sermonSave,
    sermonSaveExport,
    sermonVersions,
    UNDO_WINDOW_MS,
    withText,
    type Block,
    type EditableKind,
    type SaveStatus,
    type SermonDto,
    type VersionInfoDto
  } from './sermon'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    language: string
    id: string
    books: { number: number; abbreviation: string }[]
    onBack: () => void
  }

  let { t, language, id, books, onBack }: Props = $props()

  let sermon = $state<SermonDto | null>(null)
  let loadError = $state<string | null>(null)
  let status = $state<SaveStatus>('saved')
  let notice = $state<string | null>(null)
  let undo = $state<{ block: Block; index: number } | null>(null)
  let menuAt = $state<number | null>(null)
  let versions = $state<VersionInfoDto[] | null>(null)
  let versionsError = $state(false)
  let showVersions = $state(false)
  let confirmRestore = $state<VersionInfoDto | null>(null)
  let exportMessage = $state<string | null>(null)
  let root = $state<HTMLElement | null>(null)
  let undoTimer: ReturnType<typeof setTimeout> | undefined

  const queue = new SaveQueue({
    save: async () => {
      if (!sermon) return
      const payload = $state.snapshot(sermon) as SermonDto
      if (!payload.title.trim()) payload.title = t('sermonUntitled')
      await sermonSave(payload)
    },
    onStatus: (next) => (status = next)
  })

  export function flush(): Promise<boolean> {
    return queue.flush()
  }

  const blockCount = $derived(sermon?.blocks.length ?? 0)
  const atBlockLimit = $derived(blockCount >= MAX_BLOCKS)
  const kindLabel = (kind: string) => t(`sermonKind_${kind}`)

  onMount(async () => {
    try {
      sermon = await sermonGet(id)
      await tick()
      root?.querySelector<HTMLElement>('[data-focus="title"]')?.focus()
    } catch (cause) {
      loadError = asSermonError(cause).code === 'sermon_unavailable' ? t('sermonUnavailable') : t('sermonLoadError')
    }
  })

  onDestroy(() => {
    clearTimeout(undoTimer)
    if (queue.pending) void queue.flush()
  })

  function touch() {
    notice = null
    queue.touch()
  }

  function indexOfBlock(blockId: string) {
    return sermon?.blocks.findIndex((block) => block.id === blockId) ?? -1
  }

  function focusBlock(blockId: string, preferred = 'field') {
    void tick().then(() => {
      const scope = Array.from(root?.querySelectorAll<HTMLElement>('[data-block-id]') ?? []).find(
        (element) => element.dataset.blockId === blockId
      )
      const candidates = [preferred, 'field', 'down', 'up']
      for (const key of candidates) {
        const element = scope?.querySelector<HTMLElement>(`[data-focus="${key}"]`)
        if (element && !(element as HTMLButtonElement).disabled) {
          element.focus()
          return
        }
      }
    })
  }

  function onTitleInput(event: Event) {
    if (!sermon) return
    sermon.title = (event.currentTarget as HTMLInputElement).value
    touch()
  }

  function onBaseInput(event: Event) {
    if (!sermon) return
    sermon.base_text = (event.currentTarget as HTMLInputElement).value
    touch()
  }

  function onBlockInput(blockId: string, event: Event) {
    if (!sermon) return
    const index = indexOfBlock(blockId)
    if (index < 0) return
    const element = event.currentTarget as HTMLTextAreaElement
    const current = sermon.blocks[index]
    const candidate = withText(current, element.value)
    if (blockBytes(candidate) > MAX_BLOCK_BYTES) {
      element.value = blockText(current)
      notice = t('sermonLimitBlock')
      return
    }
    sermon.blocks[index] = candidate
    touch()
  }

  function onKindChange(blockId: string, event: Event) {
    if (!sermon) return
    const index = indexOfBlock(blockId)
    if (index < 0) return
    sermon.blocks[index] = convertBlock(sermon.blocks[index], (event.currentTarget as HTMLSelectElement).value as EditableKind)
    touch()
    focusBlock(blockId, 'kind')
  }

  function onLevelChange(blockId: string, event: Event) {
    if (!sermon) return
    const index = indexOfBlock(blockId)
    const block = sermon.blocks[index]
    if (!block || block.type !== 'heading') return
    sermon.blocks[index] = { ...block, level: Number((event.currentTarget as HTMLSelectElement).value) }
    touch()
  }

  function move(blockId: string, delta: -1 | 1) {
    if (!sermon) return
    const from = indexOfBlock(blockId)
    const to = from + delta
    if (from < 0 || to < 0 || to >= sermon.blocks.length) return
    const focused = document.activeElement instanceof HTMLElement ? document.activeElement.dataset.focus : undefined
    sermon.blocks = moveItem(sermon.blocks, from, to)
    touch()
    focusBlock(blockId, focused ?? 'field')
  }

  function onBlockKeydown(event: KeyboardEvent, blockId: string) {
    if (!event.altKey || event.ctrlKey || event.shiftKey || event.metaKey) return
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
    event.preventDefault()
    move(blockId, event.key === 'ArrowUp' ? -1 : 1)
  }

  function duplicate(blockId: string) {
    if (!sermon) return
    if (atBlockLimit) {
      notice = t('sermonLimitBlocks', { max: MAX_BLOCKS })
      return
    }
    const index = indexOfBlock(blockId)
    if (index < 0) return
    const copy = duplicateBlock(sermon.blocks[index])
    sermon.blocks = insertItem(sermon.blocks, index + 1, copy)
    touch()
    focusBlock(copy.id, 'dup')
  }

  function remove(blockId: string) {
    if (!sermon) return
    const index = indexOfBlock(blockId)
    if (index < 0) return
    const block = $state.snapshot(sermon.blocks[index]) as Block
    sermon.blocks = sermon.blocks.filter((candidate) => candidate.id !== blockId)
    undo = { block, index }
    clearTimeout(undoTimer)
    undoTimer = setTimeout(() => (undo = null), UNDO_WINDOW_MS)
    touch()
    void tick().then(() => root?.querySelector<HTMLElement>('[data-focus="undo"]')?.focus())
  }

  function undoRemove() {
    if (!sermon || !undo) return
    const { block, index } = undo
    undo = null
    clearTimeout(undoTimer)
    if (sermon.blocks.length >= MAX_BLOCKS) {
      notice = t('sermonLimitBlocks', { max: MAX_BLOCKS })
      return
    }
    sermon.blocks = insertItem(sermon.blocks, index, block)
    touch()
    focusBlock(block.id)
  }

  function addBlock(kind: EditableKind, at: number) {
    if (!sermon) return
    menuAt = null
    if (atBlockLimit) {
      notice = t('sermonLimitBlocks', { max: MAX_BLOCKS })
      return
    }
    const block = createBlock(kind)
    sermon.blocks = insertItem(sermon.blocks, at, block)
    touch()
    focusBlock(block.id)
  }

  function onMenuKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || menuAt === null) return
    event.stopPropagation()
    const trigger = (event.currentTarget as HTMLElement).querySelector<HTMLElement>('[data-focus="add"]')
    menuAt = null
    trigger?.focus()
  }

  async function back() {
    clearTimeout(undoTimer)
    if (await queue.flush()) onBack()
    else notice = t('sermonBackBlocked')
  }

  async function retry() {
    await queue.flush()
  }

  function onRootKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 's') {
      event.preventDefault()
      void queue.flush()
    }
  }

  async function exportMarkdown() {
    exportMessage = null
    if (!(await queue.flush())) {
      exportMessage = t('sermonExportNeedsSave')
      return
    }
    try {
      const path = await sermonSaveExport(id)
      exportMessage = path ? t('sermonExported') : t('sermonExportCancelled')
    } catch {
      exportMessage = t('sermonExportError')
    }
  }

  async function loadVersions() {
    versionsError = false
    try {
      versions = await sermonVersions(id)
    } catch {
      versions = []
      versionsError = true
    }
  }

  async function toggleVersions() {
    showVersions = !showVersions
    confirmRestore = null
    if (showVersions) {
      await queue.flush()
      await loadVersions()
    }
  }

  async function restore() {
    const target = confirmRestore
    confirmRestore = null
    if (!target || !sermon) return
    if (!(await queue.flush())) {
      notice = t('sermonBackBlocked')
      return
    }
    try {
      sermon = await sermonRestore(id, target.id)
      queue.cancel()
      status = 'saved'
      undo = null
      notice = t('sermonRestored')
      await loadVersions()
      await tick()
      root?.querySelector<HTMLElement>('[data-focus="title"]')?.focus()
    } catch {
      notice = t('sermonRestoreError')
    }
  }

  function autosize(node: HTMLTextAreaElement, _value?: string) {
    const fit = () => {
      node.style.height = 'auto'
      node.style.height = `${node.scrollHeight}px`
    }
    fit()
    node.addEventListener('input', fit)
    return {
      update: fit,
      destroy: () => node.removeEventListener('input', fit)
    }
  }
</script>

{#snippet addSlot(at: number)}
  <!-- O Esc é tratado aqui para fechar só o menu; o contêiner não é interativo. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="sermon-slot" onkeydown={onMenuKeydown}>
    <button
      type="button"
      class="compact-control"
      data-focus="add"
      aria-haspopup="menu"
      aria-expanded={menuAt === at}
      aria-label={t('sermonAddBlockAt', { n: at + 1 })}
      onclick={() => (menuAt = menuAt === at ? null : at)}
    >{t('sermonAddBlock')}</button>
    {#if menuAt === at}
      <div class="sermon-menu-list" role="menu" aria-label={t('sermonAddBlock')}>
        {#each editableKinds as kind (kind)}
          <button type="button" role="menuitem" onclick={() => addBlock(kind, at)}>{kindLabel(kind)}</button>
        {/each}
      </div>
    {/if}
  </div>
{/snippet}

<!-- O atalho Ctrl+S vem dos campos internos; o contêiner não é interativo. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="sermon-editor" aria-label={t('sermonEditorLabel')} bind:this={root} onkeydown={onRootKeydown}>
  {#if loadError}
    <p class="panel-empty" role="alert">{loadError}</p>
    <button type="button" class="compact-control" onclick={onBack}>{t('sermonBackToList')}</button>
  {:else if !sermon}
    <p class="panel-empty" role="status">{t('sermonLoading')}</p>
  {:else}
    <div class="sermon-bar">
      <button type="button" class="compact-control" onclick={() => void back()}>← {t('sermonBackToList')}</button>
      <span class="sermon-status" class:error={status === 'error'} role="status" aria-live="polite">
        {#if status === 'saved'}{t('sermonSaved')}
        {:else if status === 'saving'}{t('sermonSaving')}
        {:else}
          {t('sermonSaveError')}
        {/if}
      </span>
      {#if status === 'error'}
        <button type="button" class="compact-control" onclick={() => void retry()}>{t('sermonRetry')}</button>
      {/if}
    </div>

    <label class="sermon-field">
      <span>{t('sermonTitleLabel')}</span>
      <input
        type="text"
        class="sermon-input sermon-title-input"
        data-focus="title"
        maxlength={MAX_TITLE_CHARS}
        value={sermon.title}
        oninput={onTitleInput}
      />
      {#if sermon.title.length >= MAX_TITLE_CHARS}
        <span class="panel-note" role="alert">{t('sermonLimitTitle', { max: MAX_TITLE_CHARS })}</span>
      {/if}
    </label>
    <label class="sermon-field">
      <span>{t('sermonBaseLabel')}</span>
      <input
        type="text"
        class="sermon-input"
        data-focus="base"
        maxlength={MAX_BASE_TEXT_CHARS}
        placeholder={t('sermonBasePlaceholder')}
        value={sermon.base_text}
        oninput={onBaseInput}
      />
      {#if sermon.base_text.length >= MAX_BASE_TEXT_CHARS}
        <span class="panel-note" role="alert">{t('sermonLimitBase', { max: MAX_BASE_TEXT_CHARS })}</span>
      {/if}
    </label>

    <div class="sermon-actions">
      <button type="button" class="compact-control" onclick={() => void exportMarkdown()}>{t('sermonExport')}</button>
      <button type="button" class="compact-control" aria-expanded={showVersions} onclick={() => void toggleVersions()}>{t('sermonVersions')}</button>
    </div>
    {#if exportMessage}<p class="panel-note" role="status">{exportMessage}</p>{/if}
    {#if notice}<p class="sermon-banner" role="alert">{notice}</p>{/if}

    {#if showVersions}
      <div class="sermon-versions" aria-label={t('sermonVersions')} role="group">
        {#if versions === null}
          <p class="panel-note">{t('sermonLoading')}</p>
        {:else if versionsError}
          <p class="panel-note" role="alert">{t('sermonVersionsError')}</p>
        {:else if !versions.length}
          <p class="panel-note">{t('sermonVersionsEmpty')}</p>
        {:else}
          <ul>
            {#each versions as version (version.id)}
              <li>
                <span>{formatDate(version.created_at, language)} · {version.title} · {t('sermonBlocks', { count: version.block_count })}</span>
                <button type="button" class="compact-control" onclick={() => (confirmRestore = version)}>{t('sermonRestore')}</button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if confirmRestore}
          <div class="sermon-confirm" role="alertdialog" aria-label={t('sermonRestoreTitle')}>
            <p>{t('sermonRestoreBody', { date: formatDate(confirmRestore.created_at, language) })}</p>
            <div class="sermon-confirm-actions">
              <button type="button" class="danger-action" onclick={() => void restore()}>{t('sermonRestoreConfirm')}</button>
              <button type="button" class="compact-control" onclick={() => (confirmRestore = null)}>{t('cancel')}</button>
            </div>
          </div>
        {/if}
      </div>
    {/if}

    {#if undo}
      <div class="sermon-undo" role="status">
        <span>{t('sermonBlockRemoved')}</span>
        <button type="button" class="compact-control" data-focus="undo" onclick={undoRemove}>{t('sermonUndo')}</button>
      </div>
    {/if}

    {#if !sermon.blocks.length}
      <p class="panel-note">{t('sermonNoBlocks')}</p>
      {@render addSlot(0)}
    {/if}
    <ol class="sermon-blocks">
      {#each sermon.blocks as block, index (block.id)}
        <!-- O Alt+↑/↓ chega dos controles do bloco; o item da lista não é interativo. -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <li class="sermon-block" data-block-id={block.id} onkeydown={(event) => onBlockKeydown(event, block.id)}>
          <div class="sermon-block-head">
            {#if isEditable(block)}
              <select
                class="sermon-select"
                data-focus="kind"
                aria-label={t('sermonBlockType', { n: index + 1 })}
                value={block.type}
                onchange={(event) => onKindChange(block.id, event)}
              >
                {#each editableKinds as kind (kind)}
                  <option value={kind}>{kindLabel(kind)}</option>
                {/each}
              </select>
              {#if block.type === 'heading'}
                <select
                  class="sermon-select"
                  data-focus="level"
                  aria-label={t('sermonHeadingLevel', { n: index + 1 })}
                  value={String(block.level)}
                  onchange={(event) => onLevelChange(block.id, event)}
                >
                  {#each [1, 2, 3] as level (level)}
                    <option value={String(level)}>{t('sermonLevel', { level })}</option>
                  {/each}
                </select>
              {/if}
            {:else}
              <span class="sermon-kind-tag">{kindLabel(block.type)}</span>
            {/if}
            <span class="sermon-block-tools">
              <button type="button" class="compact-control" data-focus="up" disabled={index === 0} aria-label={t('sermonMoveUp', { n: index + 1 })} title="Alt+↑" onclick={() => move(block.id, -1)}>↑</button>
              <button type="button" class="compact-control" data-focus="down" disabled={index === blockCount - 1} aria-label={t('sermonMoveDown', { n: index + 1 })} title="Alt+↓" onclick={() => move(block.id, 1)}>↓</button>
              <button type="button" class="compact-control" data-focus="dup" aria-label={t('sermonDuplicateBlock', { n: index + 1 })} onclick={() => duplicate(block.id)}>⧉</button>
              <button type="button" class="compact-control" data-focus="remove" aria-label={t('sermonRemoveBlock', { n: index + 1 })} onclick={() => remove(block.id)}>✕</button>
            </span>
          </div>
          {#if block.type === 'verse'}
            <p class="sermon-ref" data-focus="field" tabindex="-1">{referenceLabel(block.range, books)} · {block.module_abbr}</p>
            <p class="sermon-snapshot">{block.snapshot}</p>
          {:else if block.type === 'lexicon'}
            <p class="sermon-ref" data-focus="field" tabindex="-1">{block.strong_id} — {block.title}</p>
            <p class="sermon-snapshot">{block.snapshot}</p>
          {:else}
            <textarea
              class="sermon-textarea"
              class:heading-1={block.type === 'heading' && block.level === 1}
              data-focus="field"
              rows="1"
              aria-label={t('sermonBlockField', { kind: kindLabel(block.type), n: index + 1 })}
              value={blockText(block)}
              use:autosize={blockText(block)}
              oninput={(event) => onBlockInput(block.id, event)}
            ></textarea>
          {/if}
          {@render addSlot(index + 1)}
        </li>
      {/each}
    </ol>
  {/if}
</section>
