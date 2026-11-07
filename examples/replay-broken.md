一个渲染时会出错的界面（测试用）。
```tsx
import { Text, View } from "nsh";

type User = { profile: { name: string } };
const users: User[] = [];

export default function App() {
  return (
    <View class="flex-1 bg-background p-8">
      <Text class="text-foreground">你好，{users[0].profile.name}</Text>
    </View>
  );
}
```
