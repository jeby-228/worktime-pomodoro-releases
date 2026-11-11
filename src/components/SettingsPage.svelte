<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Palette, Calendar, Keyboard, Info, Tags, ArrowUp, ArrowDown, Eye, EyeOff } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { applyTheme, type ThemeChoice } from '$lib/theme';
  import type { Category } from '$lib/types';
  
  interface AppSettings {
    theme: string;
    lunch_start: string;
    lunch_end: string;
    work_end: string;
    eod_check_enabled: boolean;
    auto_update: boolean;
    remember_last_category: boolean;
  }
  
  let settings = $state<AppSettings>({
    theme: 'system',
    lunch_start: '12:00',
    lunch_end: '13:00',
    work_end: '17:30',
    eod_check_enabled: true,
    auto_update: true,
    remember_last_category: true,
  });

  let categories = $state<Category[]>([]);
  let newCategoryName = $state('');
  let newCategoryCode = $state('');
  let categoryError = $state('');
  let activeSection = $state('appearance');
  let isDirty = $state(false);
  let checking = $state(false);
  let updateAvailable = $state(false);
  let updateMessage = $state('');
  
  const sections = [
    { id: 'appearance', icon: Palette, labelKey: 'settings.appearance' },
    { id: 'categories', icon: Tags, labelKey: 'categories.label' },
    { id: 'schedule', icon: Calendar, labelKey: 'settings.schedule' },
    { id: 'shortcuts', icon: Keyboard, labelKey: 'settings.shortcuts' },
    { id: 'about', icon: Info, labelKey: 'settings.about' },
  ];
  
  onMount(async () => {
    await Promise.all([loadSettings(), loadCategories()]);
  });

  async function loadCategories() {
    try {
      categories = await invoke<Category[]>('get_categories');
    } catch (err) {
      categoryError = String(err);
    }
  }

  async function addCategory() {
    categoryError = '';
    try {
      await invoke('create_category', {
        request: { name: newCategoryName, report_code: newCategoryCode || null },
      });
      newCategoryName = '';
      newCategoryCode = '';
      await loadCategories();
    } catch (err) {
      categoryError = String(err);
    }
  }

  async function saveCategory(category: Category) {
    categoryError = '';
    try {
      await invoke('update_category', {
        request: {
          id: category.id,
          name: category.name,
          report_code: category.report_code,
          hidden: category.hidden,
        },
      });
    } catch (err) {
      categoryError = String(err);
    }
    // A rejected rename goes back to the stored name.
    await loadCategories();
  }

  function toggleCategory(category: Category) {
    saveCategory({ ...category, hidden: !category.hidden });
  }

  async function moveCategory(index: number, offset: number) {
    const next = [...categories];
    const [moved] = next.splice(index, 1);
    next.splice(index + offset, 0, moved);
    categoryError = '';
    try {
      await invoke('reorder_categories', { ids: next.map((category) => category.id) });
      await loadCategories();
    } catch (err) {
      categoryError = String(err);
    }
  }
  
  function chooseTheme(theme: ThemeChoice) {
    settings.theme = theme;
    applyTheme(theme);
    markDirty();
  }

  async function loadSettings() {
    try {
      const loaded = await invoke<AppSettings>('get_settings');
      settings = loaded;
      applyTheme(settings.theme as ThemeChoice);
      isDirty = false;
    } catch (err) {
      console.error('Failed to load settings:', err);
    }
  }
  
  async function saveSettings() {
    try {
      await invoke('save_settings', { settings });
      isDirty = false;
      
      // Apply theme change immediately
      applyTheme(settings.theme as ThemeChoice);
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  }

  function markDirty() {
    isDirty = true;
  }
  

  async function checkForUpdates() {
    checking = true;
    updateMessage = '';
    updateAvailable = false;

    try {
      const update = await invoke<{
        available: boolean;
        latest_version: string;
      }>('check_github_release');

      if (update.available) {
        updateAvailable = true;
        updateMessage = $t('settings.update_available', { version: update.latest_version });
        if (confirm($t('settings.update_confirm', { version: update.latest_version }))) {
          updateMessage = $t('settings.update_installing');
          // Installs the signed update and restarts the app; only returns on error.
          await invoke('install_github_release');
        }
      } else {
        updateMessage = $t('settings.no_updates');
      }
    } catch (error: any) {
      console.error('Update check failed:', error);
      updateMessage = error?.toString?.() || $t('settings.update_check_failed');
    } finally {
      checking = false;
    }
  }
</script>

<div class="flex h-screen">
  <!-- Left sidebar: Section list -->
  <div class="w-64 border-r border-border bg-muted/30">
    <div class="p-6">
      <h1 class="text-2xl font-bold">{$t('settings.title')}</h1>
    </div>
    
    <nav class="space-y-1 px-3">
      {#each sections as section (section.id)}
        {@const Icon = section.icon}
        <button
          onclick={() => activeSection = section.id}
          class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors {activeSection === section.id ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'}"
        >
          <Icon class="size-4" />
          {$t(section.labelKey)}
        </button>
      {/each}
    </nav>
  </div>
  
  <!-- Right: Form content -->
  <div class="flex-1 overflow-y-auto">
    <div class="mx-auto max-w-2xl p-8">
      {#if activeSection === 'appearance'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.appearance')}</h2>
          </div>
          
          <!-- Theme -->
          <div class="space-y-2">
            <p class="text-sm font-medium">{$t('settings.theme')}</p>
            <div class="flex gap-3">
              <button
                onclick={() => chooseTheme('light')}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors {settings.theme === 'light' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.theme_light')}
              </button>
              <button
                onclick={() => chooseTheme('dark')}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors {settings.theme === 'dark' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.theme_dark')}
              </button>
              <button
                onclick={() => chooseTheme('system')}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors {settings.theme === 'system' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.theme_system')}
              </button>
            </div>
          </div>
        </div>
        
      {:else if activeSection === 'categories'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-1">{$t('categories.label')}</h2>
            <p class="text-sm text-muted-foreground">{$t('categories.settings_hint')}</p>
          </div>

          <div class="space-y-2">
            {#each categories as category, index (category.id)}
              <div class="flex items-center gap-2 rounded-md border border-border p-2 {category.hidden ? 'opacity-60' : ''}">
                <div class="flex shrink-0 flex-col">
                  <button
                    onclick={() => moveCategory(index, -1)}
                    disabled={index === 0}
                    aria-label={$t('categories.move_up')}
                    title={$t('categories.move_up')}
                    class="rounded p-0.5 text-muted-foreground hover:bg-accent disabled:opacity-30"
                  >
                    <ArrowUp size={14} />
                  </button>
                  <button
                    onclick={() => moveCategory(index, 1)}
                    disabled={index === categories.length - 1}
                    aria-label={$t('categories.move_down')}
                    title={$t('categories.move_down')}
                    class="rounded p-0.5 text-muted-foreground hover:bg-accent disabled:opacity-30"
                  >
                    <ArrowDown size={14} />
                  </button>
                </div>
                <input
                  type="text"
                  bind:value={category.name}
                  onchange={() => saveCategory(category)}
                  aria-label={$t('categories.label')}
                  class="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
                />
                <input
                  type="text"
                  bind:value={category.report_code}
                  onchange={() => saveCategory(category)}
                  aria-label={$t('categories.report_code')}
                  placeholder={$t('categories.report_code')}
                  class="w-28 shrink-0 rounded-md border border-border bg-background px-3 py-1.5 text-sm placeholder:text-muted-foreground"
                />
                <button
                  onclick={() => toggleCategory(category)}
                  aria-label={category.hidden ? $t('categories.show') : $t('categories.hide')}
                  title={category.hidden ? $t('categories.show') : $t('categories.hide')}
                  class="shrink-0 rounded-md p-2 text-muted-foreground hover:bg-accent"
                >
                  {#if category.hidden}
                    <EyeOff size={16} />
                  {:else}
                    <Eye size={16} />
                  {/if}
                </button>
                {#if category.hidden}
                  <span class="shrink-0 whitespace-nowrap text-xs text-muted-foreground">{$t('categories.hidden_badge')}</span>
                {/if}
              </div>
            {/each}
          </div>

          <form
            class="flex items-center gap-2"
            onsubmit={(event) => { event.preventDefault(); addCategory(); }}
          >
            <input
              type="text"
              bind:value={newCategoryName}
              placeholder={$t('categories.new_placeholder')}
              class="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground"
            />
            <input
              type="text"
              bind:value={newCategoryCode}
              aria-label={$t('categories.report_code')}
              placeholder={$t('categories.report_code_placeholder')}
              class="w-28 shrink-0 rounded-md border border-border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground"
            />
            <button
              type="submit"
              disabled={!newCategoryName.trim()}
              class="shrink-0 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {$t('categories.add')}
            </button>
          </form>

          {#if categoryError}
            <p class="text-sm text-destructive">{categoryError}</p>
          {/if}

          <div class="space-y-3 border-t border-border pt-4">
            <label class="flex items-center justify-between gap-4">
              <div>
                <div class="text-sm font-medium">{$t('categories.remember_last')}</div>
                <div class="text-xs text-muted-foreground">{$t('categories.remember_last_desc')}</div>
              </div>
              <input
                type="checkbox"
                bind:checked={settings.remember_last_category}
                onchange={markDirty}
                class="size-4"
              />
            </label>
          </div>
        </div>

      {:else if activeSection === 'schedule'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.schedule')}</h2>
          </div>
          
          <!-- Lunch Time -->
          <div class="space-y-2">
            <p class="text-sm font-medium">{$t('settings.lunch_time')}</p>
            <div class="flex items-center gap-3">
              <input
                type="time"
                bind:value={settings.lunch_start}
                oninput={markDirty}
                class="rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
              <span class="text-muted-foreground">-</span>
              <input
                type="time"
                bind:value={settings.lunch_end}
                oninput={markDirty}
                class="rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
            </div>
          </div>
          
          <!-- Work End Time -->
          <div class="space-y-2">
            <label for="work-end" class="text-sm font-medium">{$t('settings.work_end_time')}</label>
            <input
              id="work-end"
              type="time"
              bind:value={settings.work_end}
              oninput={markDirty}
              class="rounded-md border border-border bg-background px-3 py-2 text-sm"
            />
          </div>
          
          <!-- EOD Check -->
          <div class="space-y-2">
            <label class="flex items-center gap-3">
              <input
                type="checkbox"
                bind:checked={settings.eod_check_enabled}
                onchange={markDirty}
                class="size-4 rounded border-border"
              />
              <span class="text-sm font-medium">{$t('settings.eod_check_enabled')}</span>
            </label>
          </div>
        </div>
        
      {:else if activeSection === 'shortcuts'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.shortcuts')}</h2>
          </div>
          
          <div class="space-y-3">
            <div class="flex items-center justify-between rounded-md border border-border px-4 py-3">
              <span class="text-sm">{$t('timer.quick_input_hint')}</span>
              <kbd class="rounded bg-muted px-2 py-1 text-xs font-mono">Ctrl+Alt+T</kbd>
            </div>
            
            <div class="flex items-center justify-between rounded-md border border-border px-4 py-3">
              <span class="text-sm">{$t('timer.interrupt_hint')}</span>
              <kbd class="rounded bg-muted px-2 py-1 text-xs font-mono">Ctrl+Alt+I</kbd>
            </div>
          </div>
        </div>
        
      {:else if activeSection === 'about'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.about')}</h2>
          </div>
          
          <div class="space-y-4">
            <div>
              <div class="text-sm font-medium">{$t('app.title')}</div>
              <div class="text-sm text-muted-foreground">v1.2.5</div>
            </div>
            
            <div>
              <div class="text-sm font-medium">{$t('settings.technology')}</div>
              <div class="text-sm text-muted-foreground">
                Tauri 2 · Rust · Astro · Svelte 5 · Tailwind CSS
              </div>
            </div>

            <label class="flex items-center justify-between gap-4 pt-4 border-t border-border">
              <div>
                <div class="text-sm font-medium">{$t('settings.auto_update')}</div>
                <div class="text-xs text-muted-foreground">{$t('settings.auto_update_desc')}</div>
              </div>
              <input
                type="checkbox"
                bind:checked={settings.auto_update}
                onchange={markDirty}
                class="size-4"
              />
            </label>

            <!-- Check for Updates -->
            <div class="pt-4 border-t border-border">
              <div class="flex items-center justify-between">
                <div>
                  <div class="text-sm font-medium">{$t('settings.check_updates')}</div>
                  <div class="text-xs text-muted-foreground">{$t('settings.check_updates_desc')}</div>
                </div>
                <button
                  onclick={checkForUpdates}
                  disabled={checking}
                  class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                >
                  {checking ? $t('settings.checking') : $t('settings.check_now')}
                </button>
              </div>
              {#if updateMessage}
                <div class="mt-3 px-3 py-2 rounded-md text-sm {updateAvailable ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'}">
                  {updateMessage}
                </div>
              {/if}
            </div>
          </div>
        </div>
      {/if}
      
      <!-- Save button -->
      {#if isDirty}
        <div class="mt-8 flex justify-end gap-3">
          <button
            onclick={loadSettings}
            class="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            {$t('common.cancel')}
          </button>
          <button
            onclick={saveSettings}
            class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            {$t('common.save')}
          </button>
        </div>
      {/if}
    </div>
  </div>
</div>
