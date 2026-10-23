<script lang="ts">
  import { onMount } from 'svelte';
  import { Timer, Calendar, Database, Settings, ChevronLeft, ChevronRight, Sun, Moon } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { applyTheme, type ThemeChoice } from '$lib/theme';
  import { invoke } from '@tauri-apps/api/core';
  
  interface Props {
    currentRoute?: string;
    collapsed?: boolean;
  }
  
  let { currentRoute = 'timer', collapsed = $bindable(false) }: Props = $props();
  
  const navItems = [
    { id: 'timer', icon: Timer, labelKey: 'nav.timer' },
    { id: 'today', icon: Calendar, labelKey: 'nav.today' },
    { id: 'records', icon: Database, labelKey: 'nav.records' },
    { id: 'settings', icon: Settings, labelKey: 'nav.settings' },
  ];
  
  let currentTheme = $state<ThemeChoice>('system');
  let isDark = $state(false);
  
  onMount(() => {
    loadTheme();
  });
  
  async function loadTheme() {
    try {
      const settings = await invoke<any>('get_settings');
      currentTheme = settings.theme || 'system';
      isDark = applyTheme(currentTheme);
    } catch (err) {
      console.error('Failed to load theme:', err);
      isDark = applyTheme(currentTheme);
    }
  }
  
  async function toggleTheme() {
    const themes: ThemeChoice[] = ['light', 'dark', 'system'];
    const currentIndex = themes.indexOf(currentTheme);
    const nextTheme = themes[(currentIndex + 1) % themes.length];
    
    try {
      const settings = await invoke<any>('get_settings');
      settings.theme = nextTheme;
      await invoke('save_settings', { settings });
      currentTheme = nextTheme;
      isDark = applyTheme(currentTheme);
    } catch (err) {
      console.error('Failed to toggle theme:', err);
    }
  }
  
  function navigate(route: string) {
    window.location.href = `/${route === 'timer' ? '' : route}`;
  }
</script>

<nav class="flex h-screen flex-col border-r border-border bg-sidebar transition-all duration-200 {collapsed ? 'w-16' : 'w-56'}">
  <!-- Logo / Title -->
  <div class="flex h-14 items-center border-b border-border px-4">
    {#if !collapsed}
      <div class="flex items-center gap-2">
        <Timer class="size-5 text-accent-foreground" />
        <span class="font-semibold text-foreground">{$t('app.title')}</span>
      </div>
    {:else}
      <Timer class="size-5 text-accent-foreground mx-auto" />
    {/if}
  </div>
  
  <!-- Nav Items -->
  <div class="flex-1 space-y-1 p-3">
    {#each navItems as item (item.id)}
      {@const Icon = item.icon}
      <button
        onclick={() => navigate(item.id)}
        class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors hover:bg-accent hover:text-accent-foreground {currentRoute === item.id ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'} {collapsed ? 'justify-center' : ''}"
        title={collapsed ? $t(item.labelKey) : undefined}
      >
        <Icon class="size-5 flex-shrink-0" />
        {#if !collapsed}
          <span>{$t(item.labelKey)}</span>
        {/if}
      </button>
    {/each}
  </div>
  
  <!-- Bottom actions -->
  <div class="border-t border-border p-3 space-y-2">
    <!-- Theme toggle -->
    <button
      onclick={toggleTheme}
      class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground {collapsed ? 'justify-center' : ''}"
      title={collapsed ? $t('settings.theme') : undefined}
    >
      {#if isDark}
        <Moon class="size-5 flex-shrink-0" />
      {:else}
        <Sun class="size-5 flex-shrink-0" />
      {/if}
      {#if !collapsed}
        <span>{$t('settings.theme')}</span>
      {/if}
    </button>
    
    <!-- Collapse toggle -->
    <button
      onclick={() => collapsed = !collapsed}
      class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground {collapsed ? 'justify-center' : ''}"
      title={collapsed ? $t('nav.expand') : $t('nav.collapse')}
    >
      {#if collapsed}
        <ChevronRight class="size-5" />
      {:else}
        <ChevronLeft class="size-5 flex-shrink-0" />
        <span class="flex-1 text-left">{$t('nav.collapse')}</span>
      {/if}
    </button>
  </div>
</nav>
