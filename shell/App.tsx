// The app's shell: the prompt, the generations so far, and the sandbox that shows the current one.
// The Rust host compiles this module at start (`src/main.rs`) and answers its `host.call("generate")`.
import { host, nativeComponent, effect, cleanup, keymap, mount, Scroll, signal, Storage, Text, View } from "nsh";
import { Button, Dialog, Input, TextView } from "nsh/base";

/** One program for the sandbox: `id` changes on every load, so running the same code again reloads it. */
type Program = { id: number; js: string };

/** The sandbox: a second NSH engine (its own QuickJS VM, limits, no capabilities) in a native view of the host. */
const Sandbox = nativeComponent<{ program: Program | null }, { error: { message: string } }>("genui.sandbox", {
  props: ["program"],
  events: ["error"],
});

/** What `generate` answers. */
type Generation = {
  tsx: string;
  js: string;
  summary: string;
  model: string;
  seconds: number;
  attempts: number;
  promptTokens: number;
  completionTokens: number;
};

type Version = Generation & { id: number; instruction: string };

const EXAMPLES = [
  "番茄钟：开始/暂停/重置，显示今天完成了几个",
  "记账本：添加收支、分类筛选，顶部显示本月结余",
  "汇率换算：人民币、美元、日元、欧元互换，可交换方向",
  "一周天气卡片，带温度折线图",
  "待办看板：三列（待办/进行中/完成），可以把卡片往右移",
];

const versions = signal<Version[]>([]);
const currentId = signal<number | null>(null);
const current = () => versions().find((v) => v.id === currentId());
let nextId = 1;

/** The key's state as the host reports it: never the key itself, which stays in the keychain. */
type KeyState = { hasKey: boolean; hint: string | null; models: string[] };
const keyState = signal<KeyState>({ hasKey: false, hint: null, models: ["deepseek-v4-pro", "deepseek-flash"] });
const model = signal("deepseek-v4-pro");
const settingsOpen = signal(false);

const message = (e: unknown) => String((e as Error)?.message ?? e);

function loadSettings() {
  host
    .call<KeyState>("settings.get")
    .then((s) => {
      keyState.set(s);
      if (!s.hasKey) settingsOpen.set(true);
    })
    .catch((e) => console.error("settings.get", message(e)));
  Storage.getItem("model")
    .then((m) => m && model.set(m))
    .catch(() => {});
}

function chooseModel(m: string) {
  model.set(m);
  Storage.setItem("model", m).catch(() => {});
}

/** The DeepSeek API key and the model. The key is checked by the host with DeepSeek before it is kept. */
function Settings() {
  const draft = signal("");
  const saving = signal(false);
  const error = signal<string | null>(null);
  const save = async () => {
    if (saving() || !draft().trim()) return;
    saving.set(true);
    error.set(null);
    try {
      const { hint } = await host.call<{ hint: string }>("settings.setKey", { key: draft() }, { timeout: 20000 });
      keyState.update((s) => ({ ...s, hasKey: true, hint }));
      draft.set("");
      settingsOpen.set(false);
    } catch (e) {
      error.set(message(e));
    } finally {
      saving.set(false);
    }
  };
  const clear = async () => {
    try {
      await host.call("settings.clearKey");
      keyState.update((s) => ({ ...s, hasKey: false, hint: null }));
    } catch (e) {
      error.set(message(e));
    }
  };
  return (
    <Dialog
      open={settingsOpen()}
      onOpenChange={(e) => settingsOpen.set(e.open)}
      class="m-auto w-[520px] gap-5 rounded-(--radius) border border-border bg-popover p-6"
      backdropClass="bg-black/40"
    >
      <Text class="text-lg font-semibold text-popover-foreground">设置</Text>

      <View class="gap-2">
        <View class="flex-row items-center justify-between">
          <Text class="text-sm font-medium text-popover-foreground">DeepSeek API key</Text>
          <Text class="text-xs text-muted-foreground">
            {keyState().hasKey ? `已保存：${keyState().hint}` : "未设置"}
          </Text>
        </View>
        <Input
          value={draft()}
          onChange={({ value }) => draft.set(value)}
          onSubmit={save}
          masked
          autoFocus
          placeholder={keyState().hasKey ? "填写新的 key 以替换" : "sk-…"}
          class="rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground"
        />
        <Text class="text-xs text-muted-foreground">
          在 platform.deepseek.com 创建。保存前会用它调一次 DeepSeek 的 /models 接口验证；保存在 macOS 钥匙串里，界面和生成的代码都读不到明文。
        </Text>
        <If when={error()}>
          {(e) => <Text class="text-xs text-destructive">{e()}</Text>}
        </If>
      </View>

      <View class="gap-2">
        <Text class="text-sm font-medium text-popover-foreground">模型</Text>
        <View class="flex-row gap-2">
          <Each of={keyState().models}>
            {(m) => (
              <Button
                onPress={() => chooseModel(m())}
                class="rounded-(--radius) border px-3 py-1.5 active:opacity-70"
                classList={{ "border-primary bg-primary": model() === m(), "border-border": model() !== m() }}
              >
                <Text
                  class="text-xs"
                  classList={{ "text-primary-foreground": model() === m(), "text-foreground": model() !== m() }}
                >
                  {m()}
                </Text>
              </Button>
            )}
          </Each>
        </View>
        <Text class="text-xs text-muted-foreground">pro 写得更好，flash 更快更便宜。</Text>
      </View>

      <View class="flex-row items-center gap-2">
        <If when={keyState().hasKey}>
          <Button onPress={clear} class="rounded-(--radius) px-3 py-2 active:opacity-70">
            <Text class="text-sm text-destructive">清除 key</Text>
          </Button>
        </If>
        <View class="flex-1" />
        <Button
          onPress={() => settingsOpen.set(false)}
          class="rounded-(--radius) border border-border px-4 py-2 active:bg-muted"
        >
          <Text class="text-sm text-foreground">关闭</Text>
        </Button>
        <Button
          disabled={saving() || !draft().trim()}
          onPress={save}
          class="rounded-(--radius) bg-primary px-4 py-2 active:opacity-70 disabled:opacity-40"
        >
          <Text class="text-sm font-medium text-primary-foreground">{saving() ? "验证中…" : "验证并保存"}</Text>
        </Button>
      </View>
    </Dialog>
  );
}

function App() {
  const prompt = signal("");
  const busy = signal(false);
  const status = signal("");
  const elapsed = signal(0);
  const stream = signal("");
  const tab = signal<"preview" | "code">("preview");
  const failure = signal<string | null>(null);
  const runtimeError = signal<string | null>(null);
  const program = signal<Program | null>(null);
  let runs = 0;

  const show = (v: Version) => {
    currentId.set(v.id);
    runtimeError.set(null);
    program.set({ id: ++runs, js: v.js });
  };

  // The host streams the model's reply and its steps while a call runs.
  host.on<{ text: string }>("delta", ({ text }) => stream.update((s) => s + text));
  host.on("reset", () => stream.set(""));
  host.on<{ text: string }>("status", ({ text }) => status.set(text));

  // Development controls (`src/dev.rs`, `GENUI_DEV=1`): the host forwards stdin's commands.
  host.on<{ line: string }>("dev", ({ line }) => {
    const [command, ...rest] = line.split(" ");
    const arg = rest.join(" ");
    if (command === "prompt") prompt.set(arg);
    else if (command === "generate") generate(arg === "edit" || arg === "fix" ? arg : "new");
    else if (command === "settings") settingsOpen.set(arg !== "close");
    else if (command === "tab") tab.set(arg === "code" ? "code" : "preview");
    else console.warn("unknown dev command", line);
  });

  effect(() => {
    if (!busy()) return;
    const started = Date.now();
    elapsed.set(0);
    const id = setInterval(() => elapsed.set(Math.round((Date.now() - started) / 1000)), 250);
    cleanup(() => clearInterval(id));
  });

  /** `mode` "new" starts over, "edit" changes the current version, "fix" repairs it with the sandbox's error. */
  const generate = async (mode: "new" | "edit" | "fix") => {
    const base = current();
    const instruction = mode === "fix" ? (base?.instruction ?? "") : prompt().trim();
    if (busy() || !instruction || (mode !== "new" && !base)) return;
    if (!keyState().hasKey) {
      settingsOpen.set(true);
      return;
    }
    busy.set(true);
    failure.set(null);
    stream.set("");
    status.set("连接 DeepSeek…");
    tab.set("code");
    try {
      const g = await host.call<Generation>(
        "generate",
        {
          instruction,
          base: mode === "new" ? null : base!.tsx,
          error: mode === "fix" ? runtimeError() : null,
          model: model(),
        },
        { timeout: 0 },
      );
      const v: Version = { ...g, id: nextId++, instruction: mode === "fix" ? `修复：${instruction}` : instruction };
      versions.update((list) => [v, ...list]);
      show(v);
      tab.set("preview");
      if (mode !== "fix") prompt.set("");
    } catch (e) {
      failure.set(message(e));
    } finally {
      busy.set(false);
      status.set("");
    }
  };

  loadSettings();
  keymap.bind("mod+,", () => settingsOpen.set(true));
  keymap.bind("mod+enter", () => generate(current() ? "edit" : "new"), { when: "always" });

  return (
    <View class="flex-1 flex-row bg-background">
      {/* Sidebar */}
      <View class="w-96 gap-4 border-r border-border bg-card p-5">
        <View class="gap-1">
          <Text class="text-xl font-semibold text-foreground">GenUI</Text>
          <Text class="text-xs text-muted-foreground">DeepSeek 写 TSX → 宿主编译检查 → 沙箱里原生渲染</Text>
        </View>

        <Input
          value={prompt()}
          onChange={({ value }) => prompt.set(value)}
          multiline
          placeholder="描述你想要的界面…（⌘↩ 生成）"
          class="h-32 rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground"
        />

        <View class="flex-row gap-2">
          <Button
            disabled={busy() || !prompt().trim()}
            onPress={() => generate("new")}
            class="flex-1 items-center rounded-(--radius) bg-primary px-3 py-2 active:opacity-70 disabled:opacity-40"
          >
            <Text class="text-sm font-medium text-primary-foreground">新生成</Text>
          </Button>
          <Button
            disabled={busy() || !prompt().trim() || !current()}
            onPress={() => generate("edit")}
            class="flex-1 items-center rounded-(--radius) border border-border px-3 py-2 active:bg-muted disabled:opacity-40"
          >
            <Text class="text-sm font-medium text-foreground">在当前版本上改</Text>
          </Button>
        </View>

        <If when={busy()}>
          <Text class="text-xs text-muted-foreground">
            {status()} · {elapsed()}s · 已收到 {stream().length} 字符
          </Text>
        </If>
        <If when={failure()}>
          {(message) => (
            <View class="rounded-(--radius) border border-destructive p-3">
              <Text class="text-xs text-destructive">{message()}</Text>
            </View>
          )}
        </If>

        <View class="gap-2">
          <Text class="text-xs font-medium text-muted-foreground">试试这些</Text>
          <Each of={EXAMPLES}>
            {(example) => (
              <Button
                onPress={() => prompt.set(example())}
                class="rounded-(--radius) bg-muted px-3 py-2 active:opacity-70"
              >
                <Text class="text-xs text-foreground">{example()}</Text>
              </Button>
            )}
          </Each>
        </View>

        <Text class="text-xs font-medium text-muted-foreground">版本</Text>
        <Scroll class="flex-1">
          <View class="gap-1">
            <Each of={versions()} key={(v) => v.id}>
              {(v) => (
                <Button
                  onPress={() => show(v())}
                  class="gap-0.5 rounded-(--radius) px-3 py-2 active:bg-muted"
                  classList={{ "bg-muted": currentId() === v().id }}
                >
                  <Text class="line-clamp-2 text-xs text-foreground">
                    #{v().id} {v().instruction}
                  </Text>
                  <Text class="text-[10px] text-muted-foreground">
                    {v().model} · {v().seconds.toFixed(1)}s · {v().attempts} 次 · {v().completionTokens} tokens
                  </Text>
                </Button>
              )}
            </Each>
          </View>
        </Scroll>

        <Button
          onPress={() => settingsOpen.set(true)}
          class="flex-row items-center justify-between rounded-(--radius) border border-border px-3 py-2 active:bg-muted"
        >
          <Text class="text-xs text-foreground">设置（⌘,）</Text>
          <Text class="text-xs" classList={{ "text-muted-foreground": keyState().hasKey, "text-destructive": !keyState().hasKey }}>
            {model()} · {keyState().hasKey ? keyState().hint : "未设置 key"}
          </Text>
        </Button>
      </View>

      {/* Main area */}
      <View class="flex-1">
        <View class="h-12 flex-row items-center gap-1 border-b border-border px-3">
          <Button
            onPress={() => tab.set("preview")}
            class="rounded-(--radius) px-3 py-1.5 active:opacity-70"
            classList={{ "bg-muted": tab() === "preview" }}
          >
            <Text class="text-sm text-foreground">预览</Text>
          </Button>
          <Button
            onPress={() => tab.set("code")}
            class="rounded-(--radius) px-3 py-1.5 active:opacity-70"
            classList={{ "bg-muted": tab() === "code" }}
          >
            <Text class="text-sm text-foreground">代码</Text>
          </Button>
          <View class="flex-1" />
          <If when={current()}>
            {(v) => <Text class="truncate text-xs text-muted-foreground">{v().summary}</Text>}
          </If>
        </View>

        <If when={runtimeError()}>
          {(message) => (
            <View class="flex-row items-center gap-3 border-b border-destructive px-4 py-2">
              <Text class="line-clamp-3 flex-1 text-xs text-destructive">运行出错：{message()}</Text>
              <Button
                disabled={busy()}
                onPress={() => generate("fix")}
                class="rounded-(--radius) bg-destructive px-3 py-1.5 active:opacity-70 disabled:opacity-40"
              >
                <Text class="text-xs text-primary-foreground">让 DeepSeek 修复</Text>
              </Button>
            </View>
          )}
        </If>

        <View class="flex-1">
          <Sandbox
            program={program()}
            onError={({ message }) => runtimeError.set(message)}
            class="absolute inset-0"
          />
          <If when={!program()}>
            <View class="absolute inset-0 items-center justify-center gap-2 bg-background">
              <Text class="text-lg text-foreground">左边描述一个界面，DeepSeek 会把它写出来</Text>
              <Text class="text-sm text-muted-foreground">生成的代码跑在独立的 QuickJS 沙箱里，没有网络和文件权限</Text>
            </View>
          </If>
          <If when={tab() === "code"}>
            <View class="absolute inset-0 bg-background">
              <Scroll class="flex-1">
                <TextView
                  markdown={"```tsx\n" + (busy() ? stream() : (current()?.tsx ?? stream())) + "\n```"}
                  options={{ streaming: busy() }}
                  class="p-4"
                />
              </Scroll>
            </View>
          </If>
        </View>
      </View>
      <Settings />
    </View>
  );
}

mount(App);
