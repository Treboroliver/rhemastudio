<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import ptBr from './i18n/pt-BR.json'
  import en from './i18n/en.json'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import LibraryTree from './lib/LibraryTree.svelte'
  import ParallelBiblePane from './lib/ParallelBiblePane.svelte'
  import CommandBar from './lib/CommandBar.svelte'
  import { isReady, type AppSettings, type ModuleInfo } from './lib/modules'
  import {
    addBibleColumn,
    applyTheme,
    bibleForShortcut,
    restoreBibleColumns,
    resizeBibleFont,
    removeBibleColumn
  } from './lib/appearance'
  import {
    asApiError,
    type AlignedChaptersDto,
    History,
    nextChapter,
    previousChapter,
    type ApiError,
    type BookInfo,
    type Passage,
    type ResolveDto
  } from './lib/reader'
  import './app.css'

  const dictionaries = { 'pt-BR': ptBr, en }
  const defaults: AppSettings = {
    schema_version: 2,
    module_folders: [],
    default_bible_id: null,
    selected_bible_ids: [],
    interface_language: 'pt-BR',
    theme: 'dark',
    show_strong_numbers: false,
    bible_font_size: 20,
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
  let selectedBibleIds = $state<string[]>([])
  let books = $state<BookInfo[]>([])
  let passage = $state<Passage | null>(null)
  let alignedChapters = $state<AlignedChaptersDto | null>(null)
  let chapterError = $state<ApiError | null>(null)
  let chapterLoading = $state(false)
  let showCommand = $state(false)
  const history = new History()
  let chapterSequence = 0
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let triedFallback = false

  const readyBibles = $derived(
    modules.filter((module) => module.kind === 'Bible' && isReady(module))
  )
  const parallelBibles = $derived(
    selectedBibleIds
      .map((id) => readyBibles.find((module) => module.id === id))
      .filter((module): module is ModuleInfo => !!module)
  )
  const activeBible = $derived(readyBibles.find((module) => module.id === activeBibleId) ?? null)
  const currentBook = $derived(books.find((book) => book.number === passage?.book) ?? null)
  const chapterTitle = $derived(
    currentBook && passage ? `${currentBook.name} ${passage.chapter}` : null
  )
  const canPrevious = $derived(
    !!passage && previousChapter(passage, books) !== null
  )
  const canNext = $derived(!!passage && nextChapter(passage, books) !== null)

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
      selectedBibleIds = restoreBibleColumns(
        saved.selected_bible_ids,
        cached,
        saved.default_bible_id
      )
      defaultFolders = defaultsFound
      pendingFolders = saved.module_folders
      activeBibleId = selectedBibleIds.includes(saved.default_bible_id ?? '')
        ? saved.default_bible_id
        : selectedBibleIds[0] ?? null
      settings = {
        ...saved,
        selected_bible_ids: selectedBibleIds,
        default_bible_id: activeBibleId
      }
      if (saved.last_passage) {
        passage = {
          book: saved.last_passage.book,
          chapter: saved.last_passage.chapter,
          verse: saved.last_passage.verse ?? null
        }
        history.push(passage)
      }
      await loadBooks()
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
      const restored = restoreBibleColumns(selectedBibleIds, scanned, activeBibleId)
      const nextActive = restored.includes(activeBibleId ?? '') ? activeBibleId : restored[0] ?? null
      const changed = restored.join('|') !== selectedBibleIds.join('|') || nextActive !== activeBibleId
      selectedBibleIds = restored
      activeBibleId = nextActive
      if (changed && !setupIsFirstRun) {
        await saveSettings({ selected_bible_ids: restored, default_bible_id: nextActive })
      }
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
          selected_bible_ids: selected.default_bible_id ? [selected.default_bible_id] : [],
          first_use_completed: true
        }
      })
      settings = updated
      activeBibleId = updated.default_bible_id
      selectedBibleIds = updated.selected_bible_ids
      pendingFolders = updated.module_folders
      showSetup = false
      setupIsFirstRun = true
    } catch (cause) {
      error = errorMessage(cause, t('scanFailed'))
    }
  }

  async function saveSettings(changes: Partial<AppSettings>) {
    error = null
    try {
      const updated = await invoke<AppSettings>('settings_set', {
        settings: { ...settings, ...changes }
      })
      settings = updated
      selectedBibleIds = updated.selected_bible_ids
      activeBibleId = updated.default_bible_id
    } catch (cause) {
      error = errorMessage(cause, t('loadError'))
    }
  }

  async function updateColumns(ids: string[], activeId: string | null) {
    const next = ids.slice(0, 4)
    await saveSettings({ selected_bible_ids: next, default_bible_id: activeId })
  }

  async function selectBible(module: ModuleInfo) {
    if (!isReady(module) || module.kind !== 'Bible') return
    let next = [...selectedBibleIds]
    if (!next.includes(module.id)) {
      if (next.length < 4) next = addBibleColumn(next, module.id)
      else {
        error = t('parallelLimit')
        return
      }
    }
    await updateColumns(next, module.id)
  }

  async function addBible() {
    const candidate = readyBibles.find((module) => !selectedBibleIds.includes(module.id))
    if (!candidate || selectedBibleIds.length >= 4) return
    await updateColumns(addBibleColumn(selectedBibleIds, candidate.id), candidate.id)
  }

  async function removeBible(id: string) {
    const next = removeBibleColumn(selectedBibleIds, id)
    const nextActive = activeBibleId === id ? next[0] ?? null : activeBibleId
    await updateColumns(next, nextActive)
  }

  async function activateBible(id: string) {
    if (selectedBibleIds.includes(id) && activeBibleId !== id) {
      await saveSettings({ default_bible_id: id })
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
      const restored = restoreBibleColumns(selectedBibleIds, modules, activeBibleId)
      if (restored.join('|') !== selectedBibleIds.join('|')) {
        selectedBibleIds = restored
        activeBibleId = restored.includes(activeBibleId ?? '') ? activeBibleId : restored[0] ?? null
        await saveSettings({ selected_bible_ids: restored, default_bible_id: activeBibleId })
      }
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

  async function loadBooks() {
    try {
      books = await invoke<BookInfo[]>('book_names', { lang: settings.interface_language })
    } catch {
      books = []
    }
  }

  async function loadChapter() {
    const current = passage
    const ids = [...selectedBibleIds]
    if (!ids.length || !current) {
      alignedChapters = null
      chapterError = null
      chapterLoading = false
      return
    }
    const id = ++chapterSequence
    chapterLoading = true
    try {
      const loaded = await invoke<AlignedChaptersDto>('get_chapters', {
        moduleIds: ids,
        book: current.book,
        chapter: current.chapter
      })
      if (id !== chapterSequence) return
      if (
        loaded.columns.length &&
        loaded.columns.every((column) => column.error?.code === 'book_not_in_module') &&
        !triedFallback &&
        usingDefaultPassage
      ) {
        triedFallback = true
        passage = { book: 40, chapter: 1, verse: null }
        return
      }
      alignedChapters = loaded
      chapterError = null
    } catch (cause) {
      if (id !== chapterSequence) return
      const apiError = asApiError(cause)
      alignedChapters = null
      chapterError = apiError
    } finally {
      if (id === chapterSequence) chapterLoading = false
    }
  }

  let usingDefaultPassage = false

  function scheduleSave() {
    clearTimeout(saveTimer)
    saveTimer = setTimeout(async () => {
      const current = passage
      if (!current || !isTauri()) return
      try {
        settings = await invoke<AppSettings>('set_last_passage', {
          passage: { book: current.book, chapter: current.chapter, verse: current.verse }
        })
      } catch (cause) {
        error = errorMessage(cause, t('loadError'))
      }
    }, 500)
  }

  function navigate(target: Passage, record = true) {
    usingDefaultPassage = false
    passage = { ...target }
    if (record) history.push(passage)
    scheduleSave()
  }

  function selectVerse(verse: number) {
    if (!passage) return
    passage = { ...passage, verse }
    history.replaceCurrent(passage)
    scheduleSave()
  }

  function goPrevious() {
    const target = passage && previousChapter(passage, books)
    if (target) navigate(target)
  }

  function goNext() {
    const target = passage && nextChapter(passage, books)
    if (target) navigate(target)
  }

  function goBack() {
    const target = history.back()
    if (target) navigate(target, false)
  }

  function goForward() {
    const target = history.forward()
    if (target) navigate(target, false)
  }

  const resolveText = (text: string) =>
    invoke<ResolveDto>('resolve_reference', { text, lang: settings.interface_language })

  function onWindowKeydown(event: KeyboardEvent) {
    if (showSetup) return
    const key = event.key
    if ((event.ctrlKey || event.metaKey) && key.toLowerCase() === 'k') {
      event.preventDefault()
      showCommand = !showCommand
    } else if (showCommand) {
      return
    } else if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 's') {
      event.preventDefault()
      void saveSettings({ show_strong_numbers: !settings.show_strong_numbers })
    } else if (event.altKey && /^[1-4]$/.test(key)) {
      const target = bibleForShortcut(selectedBibleIds, key)
      if (target) {
        event.preventDefault()
        void activateBible(target)
      }
    } else if (event.ctrlKey && (key === '=' || key === '+')) {
      event.preventDefault()
      void saveSettings({ bible_font_size: resizeBibleFont(settings.bible_font_size, 1) })
    } else if (event.ctrlKey && key === '-') {
      event.preventDefault()
      void saveSettings({ bible_font_size: resizeBibleFont(settings.bible_font_size, -1) })
    } else if (event.ctrlKey && key === '0') {
      event.preventDefault()
      void saveSettings({ bible_font_size: resizeBibleFont(settings.bible_font_size, 0) })
    } else if (event.ctrlKey && key === 'ArrowLeft') {
      event.preventDefault()
      goPrevious()
    } else if (event.ctrlKey && key === 'ArrowRight') {
      event.preventDefault()
      goNext()
    } else if (event.altKey && key === 'ArrowLeft') {
      event.preventDefault()
      goBack()
    } else if (event.altKey && key === 'ArrowRight') {
      event.preventDefault()
      goForward()
    }
  }

  onMount(loadLibrary)

  $effect(() => {
    if (activeBible && books.length && !passage) {
      usingDefaultPassage = true
      passage = { book: 1, chapter: 1, verse: null }
      history.push(passage)
    }
  })

  $effect(() => {
    void [selectedBibleIds.join('|'), passage?.book, passage?.chapter, showSetup, books.length, modules]
    if (!showSetup) untrack(() => void loadChapter())
  })

  $effect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = settings.interface_language
      applyTheme(settings.theme, document.documentElement)
    }
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
      <div class="topbar-actions">
        <label class="sr-only" for="theme-select">{t('theme')}</label>
        <select
          id="theme-select"
          class="theme-select"
          aria-label={t('theme')}
          value={settings.theme}
          onchange={(event) => {
            const value = event.currentTarget.value
            if (value === 'dark' || value === 'light' || value === 'auto') {
              void saveSettings({ theme: value })
            }
          }}
        >
          <option value="dark">{t('themeDark')}</option>
          <option value="light">{t('themeLight')}</option>
          <option value="auto">{t('themeAuto')}</option>
        </select>
        <button class="command-open" onclick={() => (showCommand = true)} aria-label={t('commandBar')}>
          {t('commandOpen')} <kbd>Ctrl+K</kbd>
        </button>
      </div>
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
      {#if loading}
        <section class="reading-pane"><p>{t('loading')}</p></section>
      {:else if error}
        <section class="reading-pane"><div class="error-banner" role="alert">{error}</div></section>
      {:else}
        <ParallelBiblePane
          {t}
          columns={parallelBibles}
          aligned={alignedChapters}
          title={chapterTitle}
          bookName={currentBook?.name ?? null}
          error={chapterError}
          loading={chapterLoading}
          selectedVerse={passage?.verse ?? null}
          {activeBibleId}
          {canPrevious}
          {canNext}
          fontSize={settings.bible_font_size}
          showStrong={settings.show_strong_numbers}
          canAdd={selectedBibleIds.length < 4 && readyBibles.some((module) => !selectedBibleIds.includes(module.id))}
          onAdd={addBible}
          onRemove={removeBible}
          onActivate={activateBible}
          onSelectVerse={selectVerse}
          onPrevious={goPrevious}
          onNext={goNext}
          onFontSize={(direction) => void saveSettings({ bible_font_size: resizeBibleFont(settings.bible_font_size, direction) })}
          onToggleStrong={() => void saveSettings({ show_strong_numbers: !settings.show_strong_numbers })}
        />
      {/if}
    </main>
    {#if showCommand}
      <CommandBar
        {t}
        bibles={readyBibles}
        {selectedBibleIds}
        resolve={resolveText}
        onGo={(target) => navigate(target)}
        onSelectBible={selectBible}
        onClose={() => (showCommand = false)}
      />
    {/if}
  </div>
{/if}

<svelte:window onkeydown={onWindowKeydown} />
