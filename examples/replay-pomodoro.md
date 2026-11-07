一个番茄钟，带今日完成计数。
```tsx
import { cleanup, computed, effect, signal, Text, View } from "nsh";
import { Button } from "nsh/base";

const WORK = 25 * 60;

export default function App() {
  const left = signal(WORK);
  const running = signal(false);
  const done = signal(0);

  effect(() => {
    if (!running()) return;
    const id = setInterval(() => {
      left.update((s) => {
        if (s <= 1) {
          running.set(false);
          done.update((d) => d + 1);
          return WORK;
        }
        return s - 1;
      });
    }, 1000);
    cleanup(() => clearInterval(id));
  });

  const clock = computed(() => {
    const m = Math.floor(left() / 60).toString().padStart(2, "0");
    const s = (left() % 60).toString().padStart(2, "0");
    return `${m}:${s}`;
  });

  return (
    <View class="flex-1 items-center justify-center gap-8 bg-background p-8">
      <View class="items-center gap-2 rounded-(--radius) border border-border bg-card px-16 py-10">
        <Text class="text-sm text-muted-foreground">{running() ? "专注中" : "准备开始"}</Text>
        <Text class="text-7xl font-semibold text-card-foreground">{clock()}</Text>
        <Text class="text-sm text-muted-foreground">今天完成 {done()} 个番茄</Text>
      </View>
      <View class="flex-row gap-3">
        <Button onPress={() => running.update((r) => !r)} class="rounded-(--radius) bg-primary px-6 py-2 active:opacity-70">
          <Text class="text-primary-foreground">{running() ? "暂停" : "开始"}</Text>
        </Button>
        <Button onPress={() => { running.set(false); left.set(WORK); }} class="rounded-(--radius) border border-border px-6 py-2 active:bg-muted">
          <Text class="text-foreground">重置</Text>
        </Button>
        <Button onPress={() => { throw new Error("故意的运行时错误：测试修复流程"); }} class="rounded-(--radius) border border-destructive px-6 py-2 active:bg-muted">
          <Text class="text-destructive">制造错误</Text>
        </Button>
      </View>
    </View>
  );
}
```
