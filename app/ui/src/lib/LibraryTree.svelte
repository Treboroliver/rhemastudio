<script lang="ts">
  import type { ModuleInfo } from './modules'
  import { groupFor, isReady, stateMessage } from './modules'

  interface Props {
    t: (key: string) => string
    modules: ModuleInfo[]
    activeBibleId: string | null
    loading: boolean
    onSelect: (module: ModuleInfo) => void
    onManageFolders: () => void
    onRefresh: () => void
  }

  let { t, modules, activeBibleId, loading, onSelect, onManageFolders, onRefresh }: Props = $props()
  const groups = [
    { id: 'bibles', label: 'bibles' },
    { id: 'commentaries', label: 'commentaries' },
    { id: 'dictionaries', label: 'dictionaries' },
    { id: 'books', label: 'books' }
  ] as const

  function keyNavigation(event: KeyboardEvent, moduleId: string) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    const items = Array.from(document.querySelectorAll<HTMLButtonElement>('[data-library-item]'))
    const index = items.findIndex((item) => item.dataset.moduleId === moduleId)
    const next = event.key === 'ArrowDown' ? index + 1 : index - 1
    if (items[next]) {
      event.preventDefault()
      items[next].focus()
    }
  }
</script>

<aside class="library-panel" aria-label={t('library')}>
  <header class="library-heading">
    <h2>{t('library')}</h2>
    <div class="library-actions">
      <button aria-label={t('manageFolders')} title={t('manageFolders')} onclick={onManageFolders}>⚙</button>
      <button aria-label={t('refresh')} title={t('refresh')} disabled={loading} onclick={onRefresh}>↻</button>
    </div>
  </header>
  {#if loading}<p class="loading-label" aria-live="polite">{t('loading')}</p>{/if}
  {#each groups as group}
    <section class="library-group">
      <h3>{t(group.label)}</h3>
      <div role="tree" aria-label={t(group.label)}>
        {#each modules.filter((module) => groupFor(module.kind) === group.id) as module}
          {@const status = stateMessage(module)}
          {@const isBible = module.kind === 'Bible'}
          {@const meta = [module.abbreviation, module.language_name].filter((part) => part && part !== 'undefined').join(' · ')}
          {@const selectable = isBible && isReady(module)}
          <button
            class:active={module.id === activeBibleId}
            class="library-item"
            role="treeitem"
            aria-selected={module.id === activeBibleId}
            aria-disabled={!selectable}
            tabindex="0"
            data-library-item
            data-module-id={module.id}
            title={`${module.path}\n${status === 'protected' ? t('protectedMessage') : status === 'unsupported' ? t('unsupportedMessage') : status === 'corrupted' ? t('corruptedMessage') : ''}`}
            onclick={() => selectable && onSelect(module)}
            onkeydown={(event) => {
              keyNavigation(event, module.id)
              if (event.key === 'Enter' && selectable) onSelect(module)
            }}
          >
            <span class="item-main">
              <span class="item-title">{module.title}</span>
              {#if meta}<span class="item-meta">{meta}</span>{/if}
              {#if status === 'protected'}<span class="item-warning">🔒 {t('protectedMessage')}</span>{/if}
              {#if status === 'unsupported'}<span class="item-warning">{t('unsupportedMessage')}</span>{/if}
              {#if status === 'corrupted'}<span class="item-warning subtle">{t('corruptedMessage')}</span>{/if}
            </span>
          </button>
        {:else}
          <p class="empty-group">{t(group.id === 'books' ? 'emptyBooks' : 'emptyGroup')}</p>
        {/each}
      </div>
    </section>
  {/each}
</aside>
