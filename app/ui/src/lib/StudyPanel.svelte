<script lang="ts">
  import CommentaryTab from './CommentaryTab.svelte'
  import LexiconTab from './LexiconTab.svelte'
  import SermonTab from './SermonTab.svelte'
  import type { SermonBridge } from './sermonSession'
  import type { ModuleInfo } from './modules'
  import type { NodeActions } from './reader'
  import type { StrongRequest, StudyTab } from './study'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    language?: string
    books?: { number: number; abbreviation: string }[]
    open: boolean
    expanded?: boolean
    tab: StudyTab
    strongRequest: StrongRequest | null
    selection: { book: number; chapter: number; verse: number } | null
    commentaries: ModuleInfo[]
    activeCommentaryIds: string[]
    onTab: (tab: StudyTab) => void
    onToggleExpand?: () => void
    onToggleCommentary: (id: string, enabled: boolean) => void
    onVerse: NonNullable<NodeActions['onVerse']>
    onEntry: (id: string) => void
    onManageFolders: () => void
    onEscape: () => void
    bridge?: SermonBridge
    onInsertLexicon?: (key: string) => void
  }

  let {
    t,
    language = 'pt-BR',
    books = [],
    open,
    expanded = false,
    tab,
    strongRequest,
    selection,
    commentaries,
    activeCommentaryIds,
    onTab,
    onToggleExpand = () => undefined,
    onToggleCommentary,
    onVerse,
    onEntry,
    onManageFolders,
    onEscape,
    bridge,
    onInsertLexicon
  }: Props = $props()

  // A aba Referências só aparece na M4b.
  const tabs: StudyTab[] = ['lexicon', 'commentary', 'sermon']
  const tabKeys: Record<StudyTab, string> = {
    lexicon: 'tabLexicon',
    commentary: 'tabCommentary',
    sermon: 'tabSermon'
  }
  let root = $state<HTMLElement | null>(null)
  let tabButtons = $state<Record<string, HTMLButtonElement | undefined>>({})
  // A aba Sermão só é montada na primeira vez que é aberta (evita consultar o banco sem necessidade).
  let sermonVisited = $state(false)
  $effect(() => {
    if (tab === 'sermon') sermonVisited = true
  })

  export function focusContent() {
    const panel = root?.querySelector<HTMLElement>(`#study-panel-${tab}`)
    panel?.focus()
  }

  export function contains(element: Element | null): boolean {
    return !!element && !!root?.contains(element)
  }

  function onTabKeydown(event: KeyboardEvent, current: StudyTab) {
    const position = tabs.indexOf(current)
    let next: StudyTab | null = null
    if (event.key === 'ArrowRight') next = tabs[(position + 1) % tabs.length]
    else if (event.key === 'ArrowLeft') next = tabs[(position - 1 + tabs.length) % tabs.length]
    else if (event.key === 'Home') next = tabs[0]
    else if (event.key === 'End') next = tabs[tabs.length - 1]
    if (!next) return
    event.preventDefault()
    onTab(next)
    tabButtons[next]?.focus()
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && !event.defaultPrevented) {
      event.preventDefault()
      onEscape()
    }
  }
</script>

<!-- O Esc vem dos controles internos e só devolve o foco; o aside não é interativo. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside class="study-panel" class:expanded aria-label={t('studyPanel')} hidden={!open} bind:this={root} onkeydown={onKeydown}>
  <div class="study-tabs" role="tablist" aria-label={t('studyTabs')}>
    {#each tabs as item (item)}
      <button
        type="button"
        role="tab"
        id={`study-tab-${item}`}
        aria-selected={tab === item}
        aria-controls={`study-panel-${item}`}
        tabindex={tab === item ? 0 : -1}
        bind:this={tabButtons[item]}
        onclick={() => onTab(item)}
        onkeydown={(event) => onTabKeydown(event, item)}
      >{t(tabKeys[item])}</button>
    {/each}
    <button
      type="button"
      class="study-expand"
      aria-pressed={expanded}
      aria-label={expanded ? t('studyCollapse') : t('studyExpand')}
      onclick={onToggleExpand}
    >{expanded ? t('studyCollapse') : t('studyExpand')}</button>
  </div>
  <div
    class="study-body"
    role="tabpanel"
    id="study-panel-lexicon"
    aria-labelledby="study-tab-lexicon"
    tabindex="-1"
    hidden={tab !== 'lexicon'}
  >
    <LexiconTab {t} request={strongRequest} {onVerse} {onManageFolders} onInsert={onInsertLexicon} />
  </div>
  <div
    class="study-body"
    role="tabpanel"
    id="study-panel-commentary"
    aria-labelledby="study-tab-commentary"
    tabindex="-1"
    hidden={tab !== 'commentary'}
  >
    <CommentaryTab
      {t}
      {selection}
      {commentaries}
      activeIds={activeCommentaryIds}
      active={open && tab === 'commentary'}
      onToggle={onToggleCommentary}
      {onVerse}
      {onEntry}
      {onManageFolders}
    />
  </div>
  <div
    class="study-body"
    role="tabpanel"
    id="study-panel-sermon"
    aria-labelledby="study-tab-sermon"
    tabindex="-1"
    hidden={tab !== 'sermon'}
  >
    {#if sermonVisited}
      <SermonTab {t} {language} {books} {bridge} active={open && tab === 'sermon'} />
    {/if}
  </div>
</aside>
