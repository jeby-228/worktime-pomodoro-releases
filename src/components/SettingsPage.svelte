<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { check as checkUpdate } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { Palette, Clock, Calendar, Keyboard, Monitor, Info } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { applyTheme, type ThemeChoice } from '$lib/theme';
  
  interface AppSettings {
    theme: string;
    pomodoro_minutes: number;
    lunch_start: string;
    lunch_end: string;
    work_end: string;
    eod_check_enabled: boolean;
    mini_mode: string;
    pet_id: string;
  }
  
  let settings = $state<AppSettings>({
    theme: 'system',
    pomodoro_minutes: 25,
    lunch_start: '12:00',
    lunch_end: '13:00',
    work_end: '17:30',
    eod_check_enabled: true,
    mini_mode: 'none',
    pet_id: 'tomato-kun',
  });
  
  let activeSection = $state('appearance');
  let isDirty = $state(false);
  let checking = $state(false);
  let updateAvailable = $state(false);
  let updateMessage = $state('');
  
  const sections = [
    { id: 'appearance', icon: Palette, labelKey: 'settings.appearance' },
    { id: 'pomodoro', icon: Clock, labelKey: 'settings.pomodoro' },
    { id: 'schedule', icon: Calendar, labelKey: 'settings.schedule' },
    { id: 'shortcuts', icon: Keyboard, labelKey: 'settings.shortcuts' },
    { id: 'mini_mode', icon: Monitor, labelKey: 'settings.mini_mode' },
    { id: 'about', icon: Info, labelKey: 'settings.about' },
  ];
  
  onMount(async () => {
    await loadSettings();
  });
  
  function chooseTheme(theme: ThemeChoice) {
    settings.theme = theme;
    applyTheme(theme);
    markDirty();
  }

  async function chooseCompanion(mode: string) {
    settings.mini_mode = mode;
    markDirty();
    await applyMiniMode(mode);
  }

  async function loadSettings() {
    try {
      const loaded = await invoke<AppSettings>('get_settings');
      settings = loaded;
      applyTheme(settings.theme as ThemeChoice);
      await applyMiniMode(settings.mini_mode);
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
      
      // Apply mini mode changes immediately
      await applyMiniMode(settings.mini_mode);
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  }

  async function applyMiniMode(mode: string) {
    try {
      const showMini = mode === 'mini' || mode === 'both';
      const showPet = mode === 'pet' || mode === 'both';
      
      if (showMini) {
        await invoke('show_minibar');
      } else {
        await invoke('hide_minibar');
      }
      
      if (showPet) {
        await invoke('show_pet');
      } else {
        await invoke('hide_pet');
      }
    } catch (err) {
      console.error('Failed to apply mini mode:', err);
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
      const update = await checkUpdate();
      
      if (update?.available) {
        updateAvailable = true;
        updateMessage = $t('settings.update_available', { version: update.version });
        
        // Ask user to install
        if (confirm($t('settings.update_confirm', { version: update.version }))) {
          await update.downloadAndInstall();
          // Prompt to restart
          if (confirm($t('settings.restart_confirm'))) {
            await relaunch();
          }
        }
      } else {
        updateMessage = $t('settings.no_updates');
      }
    } catch (error: any) {
      console.error('Update check failed:', error);
      updateMessage = $t('settings.update_check_failed');
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
        
      {:else if activeSection === 'pomodoro'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.pomodoro')}</h2>
          </div>
          
          <!-- Pomodoro Duration -->
          <div class="space-y-2">
            <label for="pomodoro-minutes" class="text-sm font-medium">{$t('settings.pomodoro_duration')}</label>
            <div class="flex items-center gap-3">
              <input
                id="pomodoro-minutes"
                type="number"
                bind:value={settings.pomodoro_minutes}
                oninput={markDirty}
                min="1"
                max="90"
                class="w-24 rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
              <span class="text-sm text-muted-foreground">{$t('time.minutes')}</span>
            </div>
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
        
      {:else if activeSection === 'mini_mode'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.mini_mode')}</h2>
          </div>
          
          <!-- Mini Mode -->
          <div class="space-y-2">
            <p class="text-sm font-medium">迷你模式</p>
            <div class="grid grid-cols-2 gap-3">
              <button
                onclick={() => chooseCompanion('none')}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors {settings.mini_mode === 'none' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.mini_mode_none')}
              </button>
              <button
                onclick={() => chooseCompanion('mini')}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors {settings.mini_mode === 'mini' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.mini_mode_bar')}
              </button>
              <button
                onclick={() => chooseCompanion('pet')}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors {settings.mini_mode === 'pet' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.mini_mode_pet')}
              </button>
              <button
                onclick={() => chooseCompanion('both')}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors {settings.mini_mode === 'both' ? 'border-primary bg-accent' : ''}"
              >
                {$t('settings.mini_mode_both')}
              </button>
            </div>
          </div>
          
          <!-- Pet Selection -->
          {#if settings.mini_mode === 'pet' || settings.mini_mode === 'both'}
            <div class="space-y-2">
              <p class="text-sm font-medium">{$t('settings.pet_selection')}</p>
              <div class="rounded-md border border-border p-4">
                <div class="mb-3 text-sm text-muted-foreground">
                  當前寵物：番茄君 (D-mono)
                </div>
              </div>
            </div>
          {/if}
        </div>
        
      {:else if activeSection === 'about'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.about')}</h2>
          </div>
          
          <div class="space-y-4">
            <div>
              <div class="text-sm font-medium">{$t('app.title')}</div>
              <div class="text-sm text-muted-foreground">v0.1.0</div>
            </div>
            
            <div>
              <div class="text-sm font-medium">{$t('settings.technology')}</div>
              <div class="text-sm text-muted-foreground">
                Tauri 2 · Rust · Astro · Svelte 5 · Tailwind CSS
              </div>
            </div>

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
