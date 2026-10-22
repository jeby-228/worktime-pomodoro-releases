<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { check as checkUpdate } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { Settings, Palette, Clock, Calendar, Keyboard, Monitor, Info } from 'lucide-svelte';
  import { t, setLocale, currentLocale } from '$lib/i18n';
  
  interface AppSettings {
    language: string;
    theme: string;
    pomodoro_length_minutes: number;
    idle_threshold_minutes: number;
    lunch_start_time: string;
    lunch_end_time: string;
    work_end_time: string;
    eod_check_enabled: boolean;
    eod_check_time: string;
    companion_mode: string;
    current_pet_path: string | null;
  }
  
  let settings = $state<AppSettings>({
    language: 'zh-TW',
    theme: 'system',
    pomodoro_length_minutes: 25,
    idle_threshold_minutes: 5,
    lunch_start_time: '12:00',
    lunch_end_time: '13:00',
    work_end_time: '17:30',
    eod_check_enabled: true,
    eod_check_time: '17:20',
    companion_mode: 'none',
    current_pet_path: null,
  });
  
  let activeSection = $state('general');
  let isDirty = $state(false);
  let checking = $state(false);
  let updateAvailable = $state(false);
  let updateMessage = $state('');
  
  const sections = [
    { id: 'general', icon: Settings, labelKey: 'settings.general' },
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
  
  async function loadSettings() {
    try {
      const loaded = await invoke<AppSettings>('get_settings');
      settings = loaded;
      isDirty = false;
    } catch (err) {
      console.error('Failed to load settings:', err);
    }
  }
  
  async function saveSettings() {
    try {
      await invoke('save_settings', { settings });
      isDirty = false;
      
      // Apply language change immediately
      if (settings.language !== 'system') {
        await setLocale(settings.language);
      }
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  }
  
  function markDirty() {
    isDirty = true;
  }
  
  async function importPet() {
    try {
      const selected = await open({
        filters: [{ name: 'Codex Pet', extensions: ['codex-pet.zip', 'zip'] }],
        multiple: false,
      });
      
      if (selected) {
        settings.current_pet_path = selected as string;
        markDirty();
      }
    } catch (err) {
      console.error('Failed to import pet:', err);
    }
  }
  
  function exportDefault() {
    // TODO: export default pet
    console.log('Export default pet');
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
      {#each sections as section}
        <button
          onclick={() => activeSection = section.id}
          class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors"
          class:bg-accent={activeSection === section.id}
          class:text-accent-foreground={activeSection === section.id}
          class:text-muted-foreground={activeSection !== section.id}
          class:hover:bg-accent/50={activeSection !== section.id}
        >
          <svelte:component this={section.icon} class="size-4" />
          {$t(section.labelKey)}
        </button>
      {/each}
    </nav>
  </div>
  
  <!-- Right: Form content -->
  <div class="flex-1 overflow-y-auto">
    <div class="mx-auto max-w-2xl p-8">
      {#if activeSection === 'general'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.general')}</h2>
          </div>
          
          <!-- Language -->
          <div class="space-y-2">
            <label class="text-sm font-medium">{$t('settings.language')}</label>
            <select
              bind:value={settings.language}
              onchange={markDirty}
              class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
            >
              <option value="system">{$t('settings.language_follow_system')}</option>
              <option value="zh-TW">繁體中文</option>
              <option value="zh-CN">简体中文</option>
              <option value="en">English</option>
              <option value="vi">Tiếng Việt</option>
              <option value="th">ไทย</option>
            </select>
          </div>
        </div>
        
      {:else if activeSection === 'appearance'}
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold mb-4">{$t('settings.appearance')}</h2>
          </div>
          
          <!-- Theme -->
          <div class="space-y-2">
            <label class="text-sm font-medium">{$t('settings.theme')}</label>
            <div class="flex gap-3">
              <button
                onclick={() => { settings.theme = 'light'; markDirty(); }}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.theme === 'light'}
                class:bg-accent={settings.theme === 'light'}
              >
                {$t('settings.theme_light')}
              </button>
              <button
                onclick={() => { settings.theme = 'dark'; markDirty(); }}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.theme === 'dark'}
                class:bg-accent={settings.theme === 'dark'}
              >
                {$t('settings.theme_dark')}
              </button>
              <button
                onclick={() => { settings.theme = 'system'; markDirty(); }}
                class="flex flex-1 items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.theme === 'system'}
                class:bg-accent={settings.theme === 'system'}
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
            <label class="text-sm font-medium">{$t('settings.pomodoro_duration')}</label>
            <div class="flex items-center gap-3">
              <input
                type="number"
                bind:value={settings.pomodoro_length_minutes}
                oninput={markDirty}
                min="1"
                max="90"
                class="w-24 rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
              <span class="text-sm text-muted-foreground">{$t('time.minutes')}</span>
            </div>
          </div>
          
          <!-- Idle Threshold -->
          <div class="space-y-2">
            <label class="text-sm font-medium">{$t('settings.idle_threshold')}</label>
            <div class="flex items-center gap-3">
              <input
                type="number"
                bind:value={settings.idle_threshold_minutes}
                oninput={markDirty}
                min="1"
                max="30"
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
            <label class="text-sm font-medium">{$t('settings.lunch_time')}</label>
            <div class="flex items-center gap-3">
              <input
                type="time"
                bind:value={settings.lunch_start_time}
                oninput={markDirty}
                class="rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
              <span class="text-muted-foreground">-</span>
              <input
                type="time"
                bind:value={settings.lunch_end_time}
                oninput={markDirty}
                class="rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
            </div>
          </div>
          
          <!-- Work End Time -->
          <div class="space-y-2">
            <label class="text-sm font-medium">{$t('settings.work_end_time')}</label>
            <input
              type="time"
              bind:value={settings.work_end_time}
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
            {#if settings.eod_check_enabled}
              <input
                type="time"
                bind:value={settings.eod_check_time}
                oninput={markDirty}
                class="ml-7 rounded-md border border-border bg-background px-3 py-2 text-sm"
              />
            {/if}
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
          
          <!-- Companion Mode -->
          <div class="space-y-2">
            <label class="text-sm font-medium">迷你模式</label>
            <div class="grid grid-cols-2 gap-3">
              <button
                onclick={() => { settings.companion_mode = 'none'; markDirty(); }}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.companion_mode === 'none'}
                class:bg-accent={settings.companion_mode === 'none'}
              >
                {$t('settings.mini_mode_none')}
              </button>
              <button
                onclick={() => { settings.companion_mode = 'mini'; markDirty(); }}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.companion_mode === 'mini'}
                class:bg-accent={settings.companion_mode === 'mini'}
              >
                {$t('settings.mini_mode_bar')}
              </button>
              <button
                onclick={() => { settings.companion_mode = 'pet'; markDirty(); }}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.companion_mode === 'pet'}
                class:bg-accent={settings.companion_mode === 'pet'}
              >
                {$t('settings.mini_mode_pet')}
              </button>
              <button
                onclick={() => { settings.companion_mode = 'both'; markDirty(); }}
                class="flex items-center justify-center rounded-md border px-4 py-3 text-sm transition-colors"
                class:border-primary={settings.companion_mode === 'both'}
                class:bg-accent={settings.companion_mode === 'both'}
              >
                {$t('settings.mini_mode_both')}
              </button>
            </div>
          </div>
          
          <!-- Pet Selection -->
          {#if settings.companion_mode === 'pet' || settings.companion_mode === 'both'}
            <div class="space-y-2">
              <label class="text-sm font-medium">{$t('settings.pet_selection')}</label>
              <div class="rounded-md border border-border p-4">
                <div class="mb-3 text-sm text-muted-foreground">
                  {$t('settings.pet_current')}: {settings.current_pet_path || '番茄君 (預設)'}
                </div>
                <div class="flex gap-3">
                  <button
                    onclick={importPet}
                    class="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
                  >
                    {$t('settings.pet_import')}
                  </button>
                  <button
                    onclick={exportDefault}
                    class="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
                  >
                    匯出預設
                  </button>
                </div>
                <div class="mt-2 text-xs text-muted-foreground">
                  {$t('settings.pet_import_hint')}
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
