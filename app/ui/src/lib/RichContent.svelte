<script lang="ts">
  import NodeView from './NodeView.svelte'
  import { splitParagraphs, type NodeActions, type UiNode } from './reader'

  interface Props {
    nodes: UiNode[]
    t: (key: string, values?: Record<string, string | number>) => string
    actions?: NodeActions
  }

  let { nodes, t, actions }: Props = $props()
  const paragraphs = $derived(splitParagraphs(nodes))
</script>

<div class="rich-content">
  {#each paragraphs as group}
    <p class="rich-paragraph"><NodeView nodes={group} {t} {actions} /></p>
  {/each}
</div>
