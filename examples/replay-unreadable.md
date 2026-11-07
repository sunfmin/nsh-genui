标题没有颜色类：深色模式下看不清（测试用）。
```tsx
import { Text, View } from "nsh";

export default function App() {
  return (
    <View class="flex-1 gap-2 bg-background p-8">
      <Text class="text-2xl font-semibold">没有颜色的标题</Text>
      <Text class="text-sm text-muted-foreground">有颜色的说明</Text>
    </View>
  );
}
```
