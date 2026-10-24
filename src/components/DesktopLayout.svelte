<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Sidebar from '$components/Sidebar.svelte';
  import { t } from '$lib/i18n';
  
  interface Props {
    currentRoute?: string;
    children?: import('svelte').Snippet;
  }
  
  let { currentRoute = 'timer', children }: Props = $props();
  let sidebarCollapsed = $state(false);

  onMount(() => {
    if (!import.meta.env.PROD) return;
    if (sessionStorage.getItem('worktime-update-checked')) return;
    sessionStorage.setItem('worktime-update-checked', '1');

    void (async () => {
      try {
        const settings = await invoke<{ auto_update?: boolean }>('get_settings');
        if (settings.auto_update === false) return;
        const update = await invoke<{ available: boolean; latest_version: string }>(
          'check_github_release',
        );
        if (!update.available) return;
        const version = update.latest_version;
        if (!confirm($t('settings.update_confirm', { version }))) return;
        const result = await invoke<string>('install_github_release');
        if (result === 'opened-dmg') {
          alert($t('settings.update_open_dmg'));
        }
      } catch (err) {
        console.error('Automatic update check failed:', err);
      }
    })();
  });
</script>

<div class="flex h-screen overflow-hidden bg-background text-foreground">
  <Sidebar {currentRoute} bind:collapsed={sidebarCollapsed} />
  
  <main class="flex-1 overflow-auto">
    {@render children?.()}
  </main>
</div>
