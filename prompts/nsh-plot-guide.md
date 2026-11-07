<!-- Vendored from longbridge/nsh website/elements/plot.md (lines 1-326, without the Rust example) at commit 0a9fa7b (nsh 0.4.0). Replace it with the file of the nsh version in Cargo.toml. -->

---
title: Plot
description: "The Plot component of nsh/base/plot, its marks Line, Area, Bar, Rule, Dot, Cell, Grid, Pie, Arc, RadialLine, RadialGrid, Sankey and Treemap, its PlotAxis and PlotLabel: charts drawn natively from the app's rows, with the scales of the axes and their labels, texts at data points, the row, cell or slice under the pointer, motion and no look of their own."
group: base
demo: el-plot
---

# Plot

A plot is the frame of a chart: the rows of data it shows, the scales of its x and y axes, the pointer over it and its
motion. Its marks draw what it holds: a `Line` through a series, an `Area` between a series and a baseline, a `Bar`
per row in the bands of a band scale, a `Rule` per row from a start to an end (a candle's wick), a `Dot` per row where two of its values meet (a scatter's points, a bubble chart's bubbles), a `Cell` per row in the bands of two categories, colored by its value (a heatmap), a `Grid` of lines across, a `Pie` of the rows' shares (a donut with a hole), an
`Arc` between two angles, and a `RadialLine` around the center with the rings and spokes of a `RadialGrid` behind it (a
radar), a `Sankey` of flows between the rows, and a `Treemap` of nested rectangles, each leaf's area its value (a market map); a `PlotAxis` labels an edge with the ticks of its scale, and a `PlotLabel` writes texts at
points of the scales. Each mark is one canvas filling the plot, drawn natively from the rows: the app's rows are read once when
it sets them, then scaled, recorded and drawn in Rust, so a chart of a thousand points runs no JS per point. The pointer
is followed in Rust too: the hover's cross line, dots and tooltip move and fade on the platform's animator, and the app
hears of a new row, nothing more. The look is the app's: marks paint with CSS's paint properties of SVG shapes
(`stroke-*`, `fill-*`), `currentColor` where none is set, an area or a bar shows a background (a gradient,
`bg-linear-*`) inside its shape, and the hover's parts (each mark's dot among them) and the axes' labels, tick marks and
lines are the app's to style and fill.

```tsx tabs
import { mount, signal, Text, View } from "nsh";
import { Area, Grid, Line, Plot } from "nsh/base/plot";

const rows = [
  { day: "Mon", visitors: 1820 },
  { day: "Tue", visitors: 2140 },
  { day: "Wed", visitors: 1960 },
  { day: "Thu", visitors: 2480 },
  { day: "Fri", visitors: 2310 },
];

function App() {
  const hovered = signal<number | null>(null);
  return (
    <View class="gap-2 p-4">
      <Text class="text-sm text-muted-foreground tabular-nums">
        {hovered() == null ? "Visitors" : `${rows[hovered()!].day}: ${rows[hovered()!].visitors}`}
      </Text>
      <Plot
        data={rows}
        y="visitors"
        yScale={{ type: "linear", domain: [0, 3000] }}
        accessibilityLabel="Visitors per day, highest on Thursday"
        onHoverChange={({ index }) => hovered.set(index)}
        crossLineClass="w-px h-full bg-border"
        dotClass="size-2.5 rounded-full border-2 border-background bg-chart-1"
        class="h-40"
      >
        <Grid y={[0, 1500, 3000]} class="stroke-border" />
        <Area class="fill-chart-1/20" />
        <Line class="stroke-chart-1 stroke-2" />
      </Plot>
    </View>
  );
}

mount(App);
```

`Plot`, `Line`, `Area`, `Bar`, `Rule`, `Dot`, `Cell`, `Grid`, `Pie`, `Arc`, `RadialLine`, `RadialGrid`, `Sankey`, `Treemap`, `PlotAxis` and `PlotLabel` come from `nsh/base/plot`, a module of [`nsh/base`](../modules/nsh-base.md) of their
own (in Rust, `nsh_base::plot`). In TypeScript a plot's props are generic over the type of its rows: field names are
checked against them, and a function of a row gets the row's type. In Rust, `plot()` makes one with a setter per prop,
the rows a `Rows` (columns of numbers by key), the values a `RowAccessor` (a key or a function of the row's index), and
the marks take the typed paint methods (`.stroke_color(..)`, `.fill_color(..)`, `.stroke_width(..)`).

## Plot

| Prop | Rust | Type | Default | Description |
|---|---|---|---|---|
| `data` | `.data(..)` | `T[]` | (required) | The rows, one object per point. The marks read their values once, when the app sets the rows; a row without a value (a field that is not a number, `null`) is skipped. |
| `x` | `.x(..)` | `keyof T \| (d: T, index: number) => number` | the row's index | What places a row along the x axis on a `linear` x scale. A `point` scale places the rows in order. |
| `y` | `.y(..)` | `keyof T \| (d: T, index: number) => number` | none | What the marks draw unless they name their own. |
| `xScale` | `.x_scale(..)` | `Scale` | `{ type: "point" }` | How the x axis places rows: `point` (in order, evenly from the left edge to the right one; one row in the middle), `linear` with an optional `domain`, or `band` (a band per row, for bars, [Bar](#bar); or a band per category, for cells, [Cell](#cell)). With a `band` y scale it defaults to `linear`, for bars across. `reverse: true` runs it from the right edge ([Reversed scales](#reversed-scales)). |
| `yScale` | `.y_scale(..)` | `Scale` | `{ type: "linear" }` | How the y axis places values: the domain's smallest value at the bottom edge, its largest at the top. Without a `domain`, the extent of what the marks draw. `band` lays a band per row from the top edge down, for bars across. `reverse: true` runs it the other way: the smallest value at the top edge, the rows from the bottom edge up ([Reversed scales](#reversed-scales)). |
| `y2Scale` | `.y2_scale(..)` | `LinearScale` | none | A second y scale, `{ type: "linear", domain?, reverse? }`, for the marks that name it (`scale="y2"`): a dual-axis chart's right-hand scale ([Dual axes](#dual-axes)). Without a `domain`, the extent of the values on it. |
| `alignTicks` | `.align_ticks(..)` | `boolean` | `true` | While neither y scale names a domain and marks are on both, both are widened to the same round steps so their ticks fall on the same lines ([Dual axes](#dual-axes)). `false` keeps each the extent of its values. |
| `interactive` | `.interactive(..)` | `boolean` | `true` | The plot follows the pointer and the keyboard: it reports the row under it and shows the hover's parts, and it is a stop of Tab whose arrow keys move the hover ([Keyboard](#keyboard)). `false` draws the marks only (a sparkline in a list), and presses on it go to the views around it. Whether the plot offers the keyboard at all is set when it is made. |
| `motion` | `.motion(..)` | `PlotMotion` | none | The timing of the appear, the hover's fade in and out, and the spring the hover glides on when the keys move it (below). |
| `appearGeneration` | `.appear_generation(..)` | `number \| string` | none | Opts in to the appear: the marks draw in when the plot is first shown, and again whenever this changes (another symbol, another period). Inside a [`PlotAppearScope`](#plotappearscope), a generation that finished there shows whole at once. |
| `accessibilityLabel` | `.accessibility_label(..)` | `string` | none | What screen readers say of the chart: what it shows and what stands out. |
| `accessibilityValue` | `.accessibility_value(..)` | `{ min?, max?, now?, text? }` | none | What screen readers hear as its value: the app's words for the row under the pointer. |
| `crossLineClass`, `crossLineStyle` | `.cross_line_class(..)`, `.cross_line_style(..)` | `string`, `StyleProp<ViewStyle>` | none | The cross line: a view as tall as the plot, centered on the row under the pointer. On a band scale it sits in the row's band, as wide as the band (across the plot on a band y scale). |
| `dotClass`, `dotStyle` | `.dot_class(..)`, `.dot_style(..)` | `string`, `StyleProp<ViewStyle>` | none | The hover's dots: a view per `Line`, `Area` and `RadialLine`, centered on its point at the row under the pointer, and a `Dot`'s as big as its dot under the pointer. A mark's `hoverDotClass` / `hoverDotStyle` replace them for its own dot. |
| `tooltipContent` | `.tooltip_content(..)` | `JSX.Element` | none | The app's tooltip, placed by `tooltipPlacement` and moved and faded with the cross line. |
| `tooltipPlacement` | `.tooltip_placement(..)` | `"top" \| "pointer"` | `"top"` | Where the tooltip goes: at the top of the plot, centered on the cross line; or beside the pointer, on the side of the cross line toward the plot's middle and below the pointer in the upper half, above it in the lower one. At the top it stays between the plot's left and right edges (its offset from the top edge is the app's: a tag under the plot takes the plot's height and more); beside the pointer it stays inside the plot. With the keyboard, it goes beside the first line's or area's point at the row (the middle of a bar's band, the middle of a pie's slice). |
| `tooltipOffset` | `.tooltip_offset(..)` | `number` | `0` | The distance between the tooltip and what places it: the top edge for `"top"`, the cross line and the pointer for `"pointer"`. |
| `colorScale` | `.color_scale(..)` | `ColorScale` | none | The marks' colors: of the categories their `color` reads, as d3's `scaleOrdinal` (`{ type: "ordinal", domain?, range, unknown? }`, [Colors by category](#colors-by-category)), or of the numbers a `Cell`'s `value` reads along a ramp, as d3's `scaleSequential` and `scaleDiverging` (`{ type: "sequential" \| "diverging", … }`, [Colors along a ramp](#colors-along-a-ramp)). |
| `children` | `.child(..)` | `JSX.Element` | none | The marks, drawn in order over each other, and whatever the app lays over them. |
| `class`, `style` | style methods | `string`, `StyleProp<ViewStyle>` | none | The plot's size and look; the marks inherit its paint properties, as CSS inherits them. `focus:` / `focus-visible:` classes and `focusStyle` draw its focus ring. |

| Event | Rust | Payload | When |
|---|---|---|---|
| `onHoverChange` | `.on_hover_change(..)` | `{ index: number \| null }` | The pointer came over another row: the index of the row nearest to it along the x axis (along the y axis when only it has bands), of the [pie](#pie-and-arc)'s slice under it, of the [dot](#dot) whose center is nearest to it, of the [cell](#cell) under it (across the gap between two cells, the nearer one), or of a [radial line](#radialline-and-radialgrid)'s row nearest to it by angle; `null` once it leaves the plot (over a pie, its slices; over a radial line, its outer radius). |

`Scale` is `{ type: "point"; reverse? } | { type: "linear"; domain?: [number, number]; reverse? } | { type: "band";
domain?: string[]; paddingInner?; paddingOuter?; maxBandWidth?; bandCount?; groupPadding?; reverse? }`. A `linear`
scale's `domain` is the extent of its two values, in either order; a domain of one value puts it in the middle. A
`band` scale's `domain` is its bands' categories, in order ([Cell](#cell)); its other options are in [Bar](#bar).

A function of a row (`y={(d) => d.high - d.low}`) is called once per row when the rows or the function change, in the
next turn, never while the plot draws or follows the pointer: keep it pure (the same row, the same number). A field
name is read with no JS at all.

### Reversed scales

`reverse: true` runs a scale from its other edge, as visx's scales take `reverse` and d3's take a range written
backwards. On the x axis the first row, or the domain's smallest value, is at the right edge; on the y axis the
domain's smallest value is at the top edge and the rows run from the bottom edge up. Everything placed on the scale
follows it: the marks, the bars' base and the way they grow, an area's baseline, the grid lines, the `PlotAxis` ticks,
the row the hover finds, the arrow keys ([Keyboard](#keyboard)) and the moves to a new domain. Bars hang from the top
edge on a reversed `linear` y scale, and bars across grow leftward from the right edge on a reversed `linear` x
scale:

```tsx
<Plot
  data={depths}
  y="depth"
  xScale={{ type: "band", paddingInner: 0.2 }}
  yScale={{ type: "linear", domain: [0, 40], reverse: true }}
  class="h-32"
>
  <Bar class="fill-chart-2" cornerRadii={{ bottomLeft: 3, bottomRight: 3 }} />
</Plot>
```

With the plot's appear, lines and areas draw in from the edge the x scale starts at, the right one when it is
reversed.

### Dual axes

<!-- demo: el-plot-dual -->

A plot has a second y scale for values of other units: volumes beside a price, revenue beside a margin. Every part
that places values along y takes `scale`: `"y"` (the default, the plot's `yScale`) or `"y2"` (its `y2Scale`). A
`PlotAxis` on the right with `scale="y2"` labels it:

```tsx
<Plot data={days} xScale={{ type: "band", paddingInner: 0.3 }} class="mx-10 mb-6 h-44">
  <Grid class="stroke-border" />
  <Bar value="volume" class="fill-chart-2/50" />
  <Line y="close" scale="y2" class="stroke-chart-1 stroke-2" />
  <PlotAxis placement="left" format={(v) => `${v}M`} labelClass="pr-2 text-xs text-muted-foreground" />
  <PlotAxis placement="right" scale="y2" labelClass="pl-2 text-xs text-muted-foreground" />
</Plot>
```

Each scale's domain is its own: the app's `domain`, or the extent of the values on it, and each moves to a new one
with `motion.update`. While neither scale names a domain, the plot lays both out in the same number of round steps (1,
2 or 5 times a power of ten, a step for about every 32 points of height): when both hold zero, zero falls on the same
line on both; otherwise a scale that starts at zero (volumes) gains its extra steps upward and another (a price) on
both sides. The axes' ticks are those steps, and a `Grid` with neither `x` nor `y` draws its lines at them, so one
grid serves both axes. `alignTicks={false}` turns this off; a `domain` the app names leaves both scales as they are.

The hover finds one row; each mark's dot sits on its own scale, and the read-out is the app's, which reads both values
of the row `onHoverChange` reports. Bars on both scales stand side by side in each band, and a `stack` piles only the
bars on its scale. `y2Scale` is linear: a plot whose y axis has bands has no second y scale.

## Line, Area and Grid

| Mark | Prop | Type | Default | Description |
|---|---|---|---|---|
| `Line` | `y` | `string \| (d, index) => number` | the plot's `y` | What it draws: one point per row with a value, the rows without one skipped. |
| `Line`, `Area` | `data`, `x` | `T[]`, `string \| (d, index) => number` | the plot's rows | Rows of a series of its own (other x values), placed on the plot's scales by its own `x` (a row's index without). |
| `Line`, `Area` | `curve` | `"natural" \| "linear" \| "stepAfter"` | `"natural"` | How it connects its points: a smooth curve, straight segments, or steps that hold each value until the next row. An area's bottom edge follows the same curve. |
| `Line` | `dot`, `dotClass`, `dotStyle` | `boolean`, `string`, `StyleProp<ViewStyle>` | `false` | A dot on every point: a view centered on it, its size, background and border the app's. |
| `Line`, `Area` | `hoverDotClass`, `hoverDotStyle` | `string`, `StyleProp<ViewStyle>` | the plot's `dotClass`, `dotStyle` | Its hover dot, on its point at the row under the pointer: each series' dot in its own color. Each replaces the plot's for this mark (a mark's color over the plot's size and ring); `hidden` shows none. |
| `Area` | `backgroundClass`, `backgroundStyle` | `string`, `StyleProp<ViewStyle>` | none | Its background: a box around its shape (from its first point to its last, from its highest point to its baseline, or to its bottom edge's lowest point on a band) whose background (`bg-*`, `bg-linear-to-b from-… to-…`, `backgroundImage`) shows only inside the shape, under its `fill`. With one, `fill` is `none` where none is set. |
| `Area` | `y1` | `string \| (d, index) => number` | the plot's `y` | What makes its top edge. |
| `Area` | `y0` | `number \| string \| (d, index) => number` | the y scale's low edge | Its bottom edge: a number is a baseline on the y scale; a field or a function gives each row its own bottom value, which makes the area a band (a day's low to its high, a forecast's range). Without one, the area closes to the edge where the y scale's smallest values are: the bottom edge, the top one on a reversed scale (an area hanging from the top). A row without a value at either edge is skipped. |
| `Line`, `Area`, `Grid` | `scale` | `"y" \| "y2"` | `"y"` | The y scale its values go along: the plot's `yScale` or its second one, `y2Scale` ([Dual axes](#dual-axes)). |
| `Grid` | `x` | `number[]` | none | Where its vertical lines go, as values of the x scale (row indices on a `point` scale). |
| `Grid` | `y` | `number[]` | none | Where its horizontal lines go, as values of the y scale. With neither `x` nor `y`, the lines follow the ticks of its scale, where a `PlotAxis` without `ticks` puts them (along x for bars across). |
| `Grid` | `ticks` | `number` | the axis' default | About how many lines it draws with neither `x` nor `y`, as a `PlotAxis`'s `ticks`. |
| `Line`, `Area` | `color` | `string \| (d, index) => string` | none | The category of its rows, its color from the plot's `colorScale`: its first row's category gives the line's `stroke` (the area's `fill`) where nothing sets one ([Colors by category](#colors-by-category)). |

Each mark fills the plot and paints with its [SVG paint properties](../modules/style-reference.md#svg-paint), its own
over those of the plot, as CSS inherits them:

| Mark | Paints with | Where none is set |
|---|---|---|
| `Line` | `stroke`, `strokeWidth`, `strokeDasharray` | `currentColor`, 1 wide, solid |
| `Area` | `fill` (over its `background` part); its top edge `stroke`, `strokeWidth`, `strokeDasharray` | `fill` `currentColor` (`none` beside a background); no stroke |
| `Grid` | `stroke`, `strokeWidth`, `strokeDasharray` | `currentColor`, 1 wide, solid; each line on whole device pixels, so a thin one stays sharp |

The values a `Line` or an `Area` draws, an area's `y0` too (its baseline, or each row's bottom value), make the y
scale's domain when the plot has none; a grid's do not. A mark draws again only when what it draws changes (its rows,
values, curve, the plot's size or scales, its paint); the pointer moves the hover's parts and never draws a mark again.

A band is an `Area` whose `y0` reads the rows: its top edge runs through `y1`, its bottom edge back through `y0`, both
with the area's `curve`, and the appear draws both in from the leading edge. Its hover dot sits on the top edge; a
`Line` over it marks the middle value:

```tsx
<Plot data={days} y="close">
  <Area y1="high" y0="low" class="fill-chart-1/15" hoverDotClass="hidden" />
  <Line class="stroke-chart-1 stroke-2" />
</Plot>
```

`fill` is CSS's, as in a browser: a color. A gradient under a line is an area's background, the way a page shows a
gradient in a shape (a box's `background-image` under `clip-path`): `backgroundClass="bg-linear-to-b from-chart-1/20
to-transparent"` fades from the area's highest point to its baseline, in the theme's tokens and the appearance's
colors like any view's background.

## Bar

A `Bar` draws a bar per row in the row's band of a `band` scale: upright on a band x scale, across on a band y scale
(the x scale then `linear`). Each bar runs from `base` to the row's value on the other scale, so a negative value
grows the other way, and a row without a value has no bar. On a reversed value scale the bars grow down from the top
edge or leftward from the right one, and a reversed band scale puts the first row's band at the right or the bottom
([Reversed scales](#reversed-scales)).

```tsx
import { Bar, Grid, Plot } from "nsh/base/plot";

const sales = [
  { month: "Jan", online: 42, store: 31 },
  { month: "Feb", online: 51, store: 28 },
  { month: "Mar", online: 48, store: 36 },
];

<Plot
  data={sales}
  xScale={{ type: "band", paddingInner: 0.3, paddingOuter: 0.15, groupPadding: 0.1 }}
  yScale={{ type: "linear", domain: [0, 60] }}
  class="h-40"
>
  <Grid y={[20, 40, 60]} class="stroke-border" />
  <Bar value="online" class="fill-chart-1" cornerRadii={{ topLeft: 4, topRight: 4 }} />
  <Bar value="store" class="fill-chart-2" cornerRadii={{ topLeft: 4, topRight: 4 }} />
</Plot>
```

Bars outside a stack stand side by side in each band, in the order of the marks; bars with the same `stack` pile up,
each row's bar starting where the one before it in the stack ends:

```tsx
<Bar value="online" stack="sales" class="fill-chart-1" />
<Bar value="store" stack="sales" class="fill-chart-2" cornerRadii={{ topLeft: 4, topRight: 4 }} label={(d) => `${d.online + d.store}`} />
```

| Prop | Type | Default | Description |
|---|---|---|---|
| `value` | `string \| (d, index) => number` | the plot's `y` | What each bar shows. |
| `base` | `number \| string \| (d, index) => number` | `0` | The value its bars grow from, on the value scale: one for every row, or a field or function giving each row its own, so each bar spans its row's base to its value (a row without one has no bar). It counts in the domain. A stack grows from its first bar's, row by row. |
| `stack` | `string` | none | The stack it joins: the bars with the same `stack` pile up in order (a row without a value adds nothing). |
| `scale` | `"y" \| "y2"` | `"y"` | The y scale its upright bars' values go along ([Dual axes](#dual-axes)); bars on both stand side by side, and a stack piles only the bars on its scale. |
| `cornerRadii` | `number \| { topLeft?, topRight?, bottomRight?, bottomLeft? }` | `0` | The radius of each corner of its bars; `{ topLeft: 4, topRight: 4 }` rounds the tips of upright bars. A radius larger than half a bar fits it. |
| `stagger` | `number` | `0` | With the plot's appear, the bars start one after another, row by row, over this share of it (0 to 0.95), each growing for the rest; 0 grows them together. |
| `inactiveOpacity` | `number` | none | The opacity of its bars away from the row under the pointer while the pointer is over the plot, so that row stands out: it fades in and out with the hover and passes from bar to bar with it. |
| `label` | `string \| (d, index) => string` | none | The text at each bar's value end: a field (its text, or its number) or a function returning the text the app formats. |
| `labelClass`, `labelStyle` | `string`, `StyleProp<TextStyle>` | none | The `label` part: a text centered on the bar, above an upright bar's end (below one growing down), after a bar across (before one growing back). Its padding is its distance from the bar. |
| `backgroundClass`, `backgroundStyle` | `string`, `StyleProp<ViewStyle>` | none | Its background: each bar's box, whose background (`bg-linear-to-t from-… to-…`, from the bottom of the bar to its top) shows only inside the bar, under its `fill`. With one, `fill` is `none` where none is set. |
| `color` | `string \| (d, index) => string` | none | Each bar's category, its color from the plot's `colorScale`: the bar's `fill` where nothing sets one ([Colors by category](#colors-by-category)). |

The band scale lays out a band per row of the plot, in order, or a band per category of its `domain`, as d3's `scaleBand` does:

| Option | Default | Description |
|---|---|---|
| `domain` | none | The categories of its bands, in order (d3's `scaleBand().domain()`): a [`Cell`](#cell) goes in the band of its row's category, and a `PlotAxis` writes each band's category; a category written twice keeps its first place. Without one, the bands are the rows, or over cells the categories the cells name along the axis, in the order the rows bring them. |
| `paddingInner` | `0` | The gap between neighbouring bands, a share of each band's step (0 to 1). |
| `paddingOuter` | `0` | The gap before the first band and after the last, a share of a step. Half of `paddingInner` puts the bands' middles at equal steps across the plot. |
| `maxBandWidth` | none | The widest a band may be, in points: a few bands across a wide plot stay narrow, each in the middle of its place. |
| `bandCount` | the rows | The bands the axis is laid out for when more than the rows: the rows take the leading ones and keep their width and place as rows come. |
| `groupPadding` | `0` | The gap between the bars side by side in a band, a share of each bar's place. |
| `reverse` | `false` | The bands run from the other edge: the first row's at the right of the x axis, at the bottom of the y axis; the bars side by side in a band keep their order. |

A bar paints with its `fill` (`fill-*`; inherited from the plot as CSS inherits it), `currentColor` where none is set
(its category's color with `color`), and shows no hover dot. On a band scale, the hover finds the band under the pointer, and the plot's cross line sits in
that band, as wide as it: `crossLineClass="w-full h-full bg-foreground/5"` tints the band, `w-px` keeps a line in its
middle. The bars' values and base make the value scale's domain when the plot has none. With the plot's appear, each
bar grows out of its stack's base; the motion, the fading and the gliding run on the platform's animator, and the
pointer draws no bar again.

### Ranges

<!-- demo: el-plot-ranges -->

With `base` a field or a function, each bar spans two values of its row: a waterfall's step from the running total
before it to the one after, a candle's body from its open to its close, a task from its start to its end. A bar grows
from its own base as it appears, and its label sits at its value end, above a bar that rises and below one that falls.

```tsx
const bridge = [
  { step: "Revenue", from: 0, to: 120, kind: "total" },
  { step: "Costs", from: 120, to: 72, kind: "down" },
  { step: "Other", from: 72, to: 81, kind: "up" },
  { step: "Profit", from: 0, to: 81, kind: "total" },
];

<Plot
  data={bridge}
  xScale={{ type: "band", paddingInner: 0.3 }}
  colorScale={{ type: "ordinal", domain: ["total", "up", "down"], range: ["#64748b", "#16a34a", "#dc2626"] }}
  class="h-40"
>
  <Bar value="to" base="from" color="kind" label={(d) => `${d.to - d.from}`} labelClass="text-xs pb-1" />
</Plot>
```

A funnel is bars across, centered: `base={(d) => -d.users / 2}` and `value={(d) => d.users / 2}` on a band y scale.

