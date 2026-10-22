<script lang="ts">
  import Sidebar from '$components/Sidebar.svelte';
  
  interface Props {
    currentRoute?: string;
    children?: import('svelte').Snippet;
  }
  
  let { currentRoute = 'timer', children }: Props = $props();
  let sidebarCollapsed = $state(false);
</script>

<div class="flex h-screen overflow-hidden bg-background text-foreground">
  <Sidebar {currentRoute} bind:collapsed={sidebarCollapsed} />
  
  <main class="flex-1 overflow-auto">
    {@render children?.()}
  </main>
</div>
