<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import ptBr from './i18n/pt-BR.json'
  import en from './i18n/en.json'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import LibraryTree from './lib/LibraryTree.svelte'
  import type { AppSettings, ModuleInfo } from './lib/modules'
  import './app.css'

  const dictionaries = { 'pt-BR': ptBr, en }
  const defaults: AppSettings = {
    module_folders: [],
    default_bible_id: null,
    interface_language: 'pt-BR',
    theme: 'dark',
    first_use_completed: false
  }

  let settings = $state<AppSettings>(defaults)
  let modules = $state<ModuleInfo[]>([])
  let defaultFolders = $state<string[]>([])
  let pendingFolders = $state<string[]>([])
  let progress = $state(0)
  let loading = $state(true)
  let scanning = $state(false)
  let scanCompleted = $state(false)
  let error = $state<string | null>(null)
  let showSetup = $state(false)
  let setupStep = $state(1)
  let setupIsFirstRun = $state(true)
  let activeBibleId = $state<string | null>(null)

  const t = (key: string, values: Record<string, string | number> = {}) => {
    const table = dictionaries[settings.interface_language] as Record<string, string>
    const template = table[key] ?? ptBr[key as keyof typeof ptBr] ?? key
    return template.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''))
  }

  const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
  const errorMessage = (cause: unknown, fallback: string) => {
    if (typeof cause === 'string' && cause.trim()) return cause
    if (cause instanceof Error && cause.message) return cause.message
    return fallback
  }

  async function loadLibrary() {
    if (!isTauri()) {
      loading = false
      error = t('loadError')
      return
    }
    try {
      const [saved, cached, defaultsFound] = await Promise.all([
        invoke<AppSettings>('settings_get'),
        invoke<ModuleInfo[]>('library_list'),
        invoke<string[]>('library_default_folders')
      ])
      settings = saved
      modules = cached
      defaultFolders = defaultsFound
      pendingFolders = saved.module_folders
      activeBibleId = saved.default_bible_id
      showSetup = !saved.first_use_completed
      setupStep = saved.first_use_completed ? 1 : 1
    } catch (cause) {
      error = errorMessage(cause, t('loadError'))
    } finally {
      loading = false
    }
  }

  async function chooseFolders() {
    scanCompleted = false
    try {
      const chosen = await invoke<string[]>('pick_folders')
      if (chosen.length) pendingFolders = [...new Set([...pendingFolders, ...chosen])]
    } catch (cause) {
      error = errorMessage(cause, t('scanFailed'))
    }
  }

  async function scanLibrary() {
    scanning = true
    progress = 0
    scanCompleted = false
    error = null
    let unlisten: UnlistenFn | undefined
    try {
      unlisten = await listen<number>('library:progress', (event) => {
        progress = event.payload
      })
      const scanned = await invoke<ModuleInfo[]>('library_scan', {
        paths: pendingFolders.length ? pendingFolders : null
      })
      modules = scanned
      scanCompleted = true
      setupStep = 3
    } catch (cause) {
      error = errorMessage(cause, t('scanFailed'))
    } finally {
      unlisten?.()
      scanning = false
    }
  }

  async function completeSetup(
    selected: Pick<AppSettings, 'module_folders' | 'default_bible_id'>
  ) {
    try {
      const folders = selected.module_folders.length
        ? selected.module_folders
        : defaultFolders
      const updated = await invoke<AppSettings>('settings_set', {
        settings: {
          ...settings,
          module_folders: folders,
          default_bible_id: selected.default_bible_id,
          first_use_completed: true
        }
      })
      settings = updated
      activeBibleId = updated.default_bible_id
      pendingFolders = updated.module_folders
      showSetup = false
      setupIsFirstRun = true
    } catch (cause) {
      error = errorMessage(cause, t('scanFailed'))
    }
  }

  async function selectBible(module: ModuleInfo) {
    try {
      const updated = await invoke<AppSettings>('settings_set', {
        settings: { ...settings, default_bible_id: module.id }
      })
      settings = updated
      activeBibleId = module.id
    } catch (cause) {
      error = errorMessage(cause, t('loadError'))
    }
  }

  async function refreshLibrary() {
    if (!isTauri()) return
    scanning = true
    error = null
    try {
      modules = await invoke<ModuleInfo[]>('library_scan', {
        paths: settings.module_folders.length ? settings.module_folders : null
      })
    } catch (cause) {
      error = errorMessage(cause, t('scanFailed'))
    } finally {
      scanning = false
    }
  }

  function manageFolders() {
    pendingFolders = [...settings.module_folders]
    scanCompleted = false
    setupIsFirstRun = false
    setupStep = 2
    showSetup = true
  }

  onMount(loadLibrary)

  $effect(() => {
    if (typeof document !== 'undefined') document.documentElement.lang = settings.interface_language
  })
</script>

<svelte:head>
  <title>{t('appName')}</title>
  <meta name="description" content={t('tagline')} />
</svelte:head>

{#if showSetup}
  <FirstRunWizard
    {t}
    {modules}
    folders={pendingFolders}
    {defaultFolders}
    {progress}
    {scanCompleted}
    loading={scanning}
    {error}
    initialStep={setupStep}
    defaultBibleId={activeBibleId}
    onChooseFolders={chooseFolders}
    onScan={scanLibrary}
    onFinish={completeSetup}
    onCancel={setupIsFirstRun ? undefined : () => (showSetup = false)}
  />
{:else}
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <div class="brand-mark" aria-hidden="true">R</div>
        <div><h1>{t('appName')}</h1><p>{t('tagline')}</p></div>
      </div>
      <span class="offline-badge">{t('offline')}</span>
    </header>
    <main class="workspace">
      <LibraryTree
        {t}
        {modules}
        {activeBibleId}
        loading={scanning}
        onSelect={selectBible}
        onManageFolders={manageFolders}
        onRefresh={refreshLibrary}
      />
      <section class="reading-pane" aria-live="polite">
        {#if loading}
          <p>{t('loading')}</p>
        {:else if error}
          <div class="error-banner" role="alert">{error}</div>
        {:else}
          <div class="empty-reading">
            <div class="empty-book" aria-hidden="true">▤</div>
            <h2>{t('selectBible')}</h2>
            <p>{t('selectBibleHint')}</p>
          </div>
        {/if}
      </section>
    </main>
  </div>
{/if}
