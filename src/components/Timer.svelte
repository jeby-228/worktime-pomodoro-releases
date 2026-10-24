<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Briefcase, Zap, Pause, Play, Square } from "lucide-svelte";
  import Button from "./ui/Button.svelte";
  import { formatSeconds } from "$lib/utils";

  let state = $state<any>(null);
  let currentSlot = $state<any>(null);
  let remainingSeconds = $state<number | null>(null);
  let elapsedSeconds = $state<number>(0);
  let interval: any = null;

  async function loadState() {
    try {
      state = await invoke("get_app_state");
      currentSlot = await invoke("get_current_slot");
      remainingSeconds = await invoke("get_remaining_seconds");
      elapsedSeconds = (await invoke("get_elapsed_seconds")) || 0;
    } catch (e) {
      console.error("Failed to load state:", e);
    }
  }

  async function startTask() {
    const taskName = prompt("輸入任務名稱：");
    if (!taskName) return;

    try {
      await invoke("start_task", {
        request: { task_name: taskName, slot_type: "work" },
      });
      await loadState();
    } catch (e: any) {
      alert(`開始任務失敗：${e}`);
    }
  }

  async function endTask() {
    const note = prompt("備註（選填）：");
    try {
      await invoke("end_task", { note: note || null });
      await loadState();
      window.dispatchEvent(new CustomEvent("task-started"));
    } catch (e: any) {
      alert(`結束任務失敗：${e}`);
    }
  }

  async function pauseTask() {
    try {
      await invoke("pause_task");
      await loadState();
    } catch (e: any) {
      alert(`暫停任務失敗：${e}`);
    }
  }

  async function resumeTask() {
    try {
      await invoke("resume_task");
      await loadState();
    } catch (e: any) {
      alert(`繼續任務失敗：${e}`);
    }
  }

  async function startInterrupt() {
    const who = prompt("是誰？（選填）");
    const reason = prompt("原因？（選填）");
    try {
      await invoke("start_interrupt", {
        request: { who: who || null, reason: reason || null },
      });
      await loadState();
    } catch (e: any) {
      alert(`插隊失敗：${e}`);
    }
  }

  async function endInterrupt() {
    try {
      await invoke("end_interrupt_and_resume");
      await loadState();
    } catch (e: any) {
      alert(`結束插隊失敗：${e}`);
    }
  }

  onMount(() => {
    loadState();
    interval = setInterval(async () => {
      if (state?.state === "working") {
        elapsedSeconds += 1;
        if (remainingSeconds !== null && remainingSeconds > 0) {
          remainingSeconds -= 1;
        }
      }
    }, 1000);
  });

  onDestroy(() => {
    if (interval) clearInterval(interval);
  });
</script>

<div class="flex flex-col items-center justify-center min-h-screen bg-background p-8">
  <div class="w-full max-w-md space-y-8">
    <!-- Timer Display -->
    <div class="text-center">
      {#if state?.state === "idle"}
        <div class="text-6xl font-mono font-bold text-muted-foreground">--:--</div>
        <div class="mt-4 text-lg text-muted-foreground">目前閒置</div>
      {:else if currentSlot}
        <div class="text-6xl font-mono font-bold text-foreground">
          {formatSeconds(elapsedSeconds)}
        </div>
        <div class="mt-4 text-xl font-medium text-foreground">
          {currentSlot.task_name}
        </div>
        {#if remainingSeconds !== null && remainingSeconds > 0}
          <div class="mt-2 text-sm text-muted-foreground">
            剩餘 {formatSeconds(remainingSeconds)}
          </div>
        {/if}
      {/if}
    </div>

    <!-- Progress Ring (placeholder) -->
    <div class="flex justify-center">
      <div class="w-48 h-48 rounded-full border-8 border-muted flex items-center justify-center">
        <div class="text-center">
          {#if state?.state === "working"}
            <Briefcase class="w-12 h-12 text-foreground mx-auto" />
            <div class="text-sm mt-2 text-foreground">工作中</div>
          {:else if state?.state === "interrupt"}
            <Zap class="w-12 h-12 text-foreground mx-auto" />
            <div class="text-sm mt-2 text-foreground">插隊中</div>
          {:else if state?.state === "paused"}
            <Pause class="w-12 h-12 text-muted-foreground mx-auto" />
            <div class="text-sm mt-2 text-muted-foreground">已暫停</div>
          {:else}
            <div class="text-4xl">💤</div>
          {/if}
        </div>
      </div>
    </div>

    <!-- Action Buttons -->
    <div class="flex flex-col gap-3">
      {#if state?.state === "idle"}
        <Button onclick={startTask} class="w-full bg-primary text-primary-foreground">
          <Briefcase class="w-4 h-4" />
          開始工作
        </Button>
      {:else if state?.state === "working"}
        <div class="flex gap-3">
          <Button onclick={pauseTask} class="flex-1 bg-secondary text-secondary-foreground">
            <Pause class="w-4 h-4" />
            暫停
          </Button>
          <Button onclick={endTask} class="flex-1 bg-primary text-primary-foreground">
            <Square class="w-4 h-4" />
            結束
          </Button>
        </div>
        <Button onclick={startInterrupt} class="w-full border-dashed border-2 bg-background">
          <Zap class="w-4 h-4" />
          插隊 (Ctrl+Alt+I)
        </Button>
      {:else if state?.state === "paused"}
        <div class="flex gap-3">
          <Button onclick={resumeTask} class="flex-1 bg-primary text-primary-foreground">
            <Play class="w-4 h-4" />
            繼續
          </Button>
          <Button onclick={endTask} class="flex-1 bg-secondary text-secondary-foreground">
            <Square class="w-4 h-4" />
            結束
          </Button>
        </div>
      {:else if state?.state === "interrupt"}
        <Button onclick={endInterrupt} class="w-full bg-primary text-primary-foreground">
          回到原本工作
        </Button>
      {/if}
    </div>

    <!-- Quick Access -->
    <div class="text-center text-sm text-muted-foreground">
      <div>Ctrl+Alt+T 快速輸入</div>
      <div>Ctrl+Alt+I 插隊</div>
    </div>
  </div>
</div>
