<script lang="ts">
  import type { AppSettings, ModuleInfo } from './modules'
  import { isReady, stateMessage } from './modules'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    modules: ModuleInfo[]
    folders: string[]
    defaultFolders: string[]
    progress: number
    scanCompleted: boolean
    loading: boolean
    error: string | null
    initialStep?: number
    defaultBibleId: string | null
    onChooseFolders: () => void
    onScan: () => void
    onFinish: (settings: Pick<AppSettings, 'module_folders' | 'default_bible_id'>) => void
    onCancel?: () => void
  }

  let {
    t,
    modules,
    folders,
    defaultFolders,
    progress,
    scanCompleted,
    loading,
    error,
    initialStep = 1,
    defaultBibleId,
    onChooseFolders,
    onScan,
    onFinish,
    onCancel
  }: Props = $props()
  let step = $state(1)
  let selectedBible = $state<string | null>(null)

  let bibles = $derived(modules.filter((module) => module.kind === 'Bible'))
  let readyBibles = $derived(bibles.filter(isReady))
  let commentaries = $derived(modules.filter((module) => module.kind === 'Commentary'))
  let dictionaries = $derived(modules.filter((module) => module.kind === 'Dictionary'))
  let protectedModules = $derived(modules.filter((module) => stateMessage(module) === 'protected'))
  let unsupportedModules = $derived(modules.filter((module) => stateMessage(module) === 'unsupported'))

  $effect(() => {
    step = initialStep
  })

  $effect(() => {
    if (defaultBibleId && readyBibles.some((module) => module.id === defaultBibleId)) {
      selectedBible = defaultBibleId
    }
  })

  $effect(() => {
    if (readyBibles.length && !readyBibles.some((module) => module.id === selectedBible)) {
      selectedBible = readyBibles[0].id
    }
  })

  $effect(() => {
    if (scanCompleted) step = 3
  })

  function addMoreFolders() {
    step = 2
    onChooseFolders()
  }

  function complete() {
    onFinish({ module_folders: folders, default_bible_id: selectedBible })
  }
</script>

<main class="setup-shell" aria-labelledby="setup-title">
  <section class="setup-card">
    <p class="eyebrow">{t('stepLabel', { current: step, total: 3 })}</p>
    {#if step === 1}
      <h1 id="setup-title">{t('welcomeTitle')}</h1>
      <p class="body-copy">{t('welcomeBody')}</p>
      <div class="actions">
        <button class="primary" onclick={() => (step = 2)}>{t('continue')}</button>
      </div>
    {:else if step === 2}
      <h1 id="setup-title">{t('locateTitle')}</h1>
      <p class="body-copy">{t('locateBody')}</p>
      {#if defaultFolders.length}
        <div class="folder-block">
          <h2>{t('defaultFolder')}</h2>
          {#each defaultFolders as folder}
            <p class="path-row" title={folder}>{folder}</p>
          {/each}
        </div>
      {/if}
      {#if folders.length}
        <div class="folder-block">
          <h2>{t('chosenFolders')}</h2>
          {#each folders as folder}
            <p class="path-row" title={folder}>{folder}</p>
          {/each}
        </div>
      {:else if !defaultFolders.length}
        <p class="body-copy">{t('noFolders')}</p>
      {/if}
      {#if loading}
        <div class="progress-area" aria-live="polite">
          <progress aria-label={t('progressBar')}></progress>
          <span>{t('scanProgress', { count: progress })}</span>
        </div>
      {/if}
      {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
      <div class="actions split">
        <button class="quiet" onclick={() => (step = 1)}>{t('back')}</button>
        <div class="button-row">
          <button class="secondary" onclick={onChooseFolders}>{t('chooseFolder')}</button>
          <button class="primary" disabled={loading} onclick={onScan}>{loading ? t('scanning') : t('scan')}</button>
        </div>
      </div>
    {:else}
      <h1 id="setup-title">{t('summaryTitle')}</h1>
      <p class="body-copy">
        {t('summaryCounts', { bibles: bibles.length, commentaries: commentaries.length, dictionaries: dictionaries.length })}
      </p>
      {#if readyBibles.length}
        <label class="field-label" for="default-bible">{t('chooseDefaultBible')}</label>
        <select id="default-bible" bind:value={selectedBible}>
          {#each readyBibles as bible}
            <option value={bible.id}>{bible.title} · {bible.abbreviation ?? ''}</option>
          {/each}
        </select>
        <div class="actions">
          <button class="secondary" onclick={addMoreFolders}>{t('chooseFolder')}</button>
        </div>
      {:else}
        <div class="empty-guidance">
          <p>{t('noReadyBible')}</p>
          <p>{t('noModulesHelp')}</p>
          <button class="secondary" onclick={() => (step = 2)}>{t('chooseFolder')}</button>
        </div>
      {/if}
      {#if protectedModules.length}
        <div class="folder-block">
          <h2>{t('protectedModules')}</h2>
          {#each protectedModules as module}<p class="module-line">🔒 {module.title}</p>{/each}
        </div>
      {/if}
      {#if unsupportedModules.length}
        <div class="folder-block">
          <h2>{t('unsupportedModules')}</h2>
          {#each unsupportedModules as module}<p class="module-line">{module.title}</p>{/each}
        </div>
      {/if}
      <div class="actions split">
        <button class="quiet" onclick={() => (step = 2)}>{t('back')}</button>
        <div class="button-row">
          {#if onCancel}<button class="quiet" onclick={onCancel}>{t('cancel')}</button>{/if}
          <button class="primary" onclick={complete}>{t('finishSetup')}</button>
        </div>
      </div>
    {/if}
  </section>
</main>
