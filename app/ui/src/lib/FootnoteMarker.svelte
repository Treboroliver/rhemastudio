<script lang="ts">
  import type { Snippet } from 'svelte'

  interface Props {
    t: (key: string) => string
    children: Snippet
  }

  let { t, children }: Props = $props()
  const popupId = $props.id()
  let open = $state(false)
  let pointerPrevious: boolean | null = null

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      event.stopPropagation()
      open = false
    }
  }

  function onclick() {
    open = pointerPrevious === null ? !open : !pointerPrevious
    pointerPrevious = null
  }
</script>

<span class="note-wrap"><button
    type="button"
    class="note-marker"
    aria-label={t('footnote')}
    aria-expanded={open}
    aria-describedby={open ? popupId : undefined}
    onpointerdown={() => (pointerPrevious = open)}
    onfocus={() => (open = true)}
    onblur={() => (open = false)}
    {onkeydown}
    {onclick}
  >†</button>{#if open}
    <span class="note-popup" role="tooltip" id={popupId} tabindex="-1">{@render children()}</span>
  {/if}</span>
