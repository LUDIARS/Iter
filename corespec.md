# Relay Graph Editor — ClaudeCode 実装指示書

> **目的**: コンパイルエラーから関連コードをリレーショナルに解析し、グラフビュー上のノードとして可視化する軽量テキストエディタを構築する。
> **読み手**: ClaudeCode (実装AI)
> **言語**: C++ (C++17以上)
> **ビルド**: CMake 3.20+
> **ターゲット**: Linux (X11) を最優先。Win32は後続フェーズ。

---

## 目次

1. [プロジェクト構造](#phase-0-プロジェクト構造)
2. [Phase 1: ウィンドウ基盤 + グラフキャンバス](#phase-1-ウィンドウ基盤--グラフキャンバス)
3. [Phase 2: ノードレンダリング + インタラクション](#phase-2-ノードレンダリング--インタラクション)
4. [Phase 3: エディタ統合 (Scintilla)](#phase-3-エディタ統合-scintilla)
5. [Phase 4: エッジ描画 + アニメーション](#phase-4-エッジ描画--アニメーション)
6. [Phase 5: エラーパーサー + Orchestrator](#phase-5-エラーパーサー--orchestrator)
7. [Phase 6: AST解析エンジン (libclang)](#phase-6-ast解析エンジン-libclang)
8. [Phase 7: アセンブリ解析 + キャッシュ](#phase-7-アセンブリ解析--キャッシュ)
9. [Phase 8: グラフレイアウトエンジン](#phase-8-グラフレイアウトエンジン)
10. [Phase 9: 統合 + CLI](#phase-9-統合--cli)

---

## Phase 0: プロジェクト構造

まずこのディレクトリ構造を作成せよ。空のファイルでよい。

```
relay-editor/
├── CMakeLists.txt
├── README.md
├── src/
│   ├── main.cpp
│   ├── core/
│   │   ├── orchestrator.h
│   │   ├── orchestrator.cpp
│   │   ├── error_parser.h
│   │   ├── error_parser.cpp
│   │   ├── types.h              # 共通型定義
│   │   └── config.h             # 設定定数
│   ├── graph/
│   │   ├── graph_view.h
│   │   ├── graph_view.cpp
│   │   ├── graph_node.h
│   │   ├── graph_node.cpp
│   │   ├── graph_edge.h
│   │   ├── graph_edge.cpp
│   │   ├── graph_layout.h
│   │   ├── graph_layout.cpp
│   │   ├── animation.h
│   │   └── animation.cpp
│   ├── editor/
│   │   ├── editor_view.h
│   │   ├── editor_view.cpp
│   │   ├── scintilla_wrapper.h
│   │   └── scintilla_wrapper.cpp
│   ├── analysis/
│   │   ├── ast_analyzer.h
│   │   ├── ast_analyzer.cpp
│   │   ├── assembly_analyzer.h
│   │   ├── assembly_analyzer.cpp
│   │   └── symbol_cache.h
│   ├── platform/
│   │   ├── window.h             # 抽象インターフェース
│   │   ├── window_x11.h
│   │   ├── window_x11.cpp
│   │   ├── renderer.h           # 描画抽象
│   │   ├── renderer_cairo.h
│   │   └── renderer_cairo.cpp
│   └── cache/
│       ├── cache_manager.h
│       └── cache_manager.cpp
├── third_party/
│   └── .gitkeep               # Scintilla等をここに配置
└── tests/
    ├── test_error_parser.cpp
    ├── test_ast_analyzer.cpp
    └── test_graph_layout.cpp
```

### CMakeLists.txt 初期テンプレート

```cmake
cmake_minimum_required(VERSION 3.20)
project(relay-editor LANGUAGES CXX C)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)  # libclang用

# --- 依存 ---
find_package(PkgConfig REQUIRED)
pkg_check_modules(CAIRO REQUIRED cairo)
pkg_check_modules(X11 REQUIRED x11)
find_package(SQLite3 REQUIRED)
# libclang: Phase 6で有効化
# find_package(Clang REQUIRED)

# --- ソース ---
file(GLOB_RECURSE SOURCES src/*.cpp)

add_executable(relay-editor ${SOURCES})
target_include_directories(relay-editor PRIVATE
    src/
    ${CAIRO_INCLUDE_DIRS}
    ${X11_INCLUDE_DIRS}
)
target_link_libraries(relay-editor PRIVATE
    ${CAIRO_LIBRARIES}
    ${X11_LIBRARIES}
    SQLite::SQLite3
)
```

### `src/core/types.h` — 共通型定義

これは全フェーズで使う。最初に定義せよ。

```cpp
#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace relay {

// --- ノード種別 ---
enum class NodeType : uint8_t {
    FUNCTION,
    TYPE,
    VARIABLE,
    INCLUDE,
    ERROR_SOURCE,  // エラー発生箇所
};

// --- エッジ種別 ---
enum class EdgeType : uint8_t {
    CALL,       // 関数呼び出し
    REFERENCE,  // 参照
    INCLUDE,    // #include
    INHERIT,    // 継承
    ERROR_PATH, // エラー伝搬パス
};

// --- エラー情報 ---
struct ErrorInfo {
    std::string file_path;
    uint32_t    line;
    uint32_t    column;
    std::string error_code;  // "CS0246", "C2065" 等
    std::string message;
};

// --- グラフノード ---
struct GraphNode {
    uint32_t    id;
    std::string file_path;
    uint32_t    line;
    uint32_t    column;
    std::string symbol_name;
    NodeType    type;
    bool        is_error = false;

    // レンダリング状態
    float x = 0, y = 0;        // グラフ座標
    float width = 180, height = 100;  // 縮小時サイズ
    bool  expanded = false;
};

// --- グラフエッジ ---
struct GraphEdge {
    uint32_t source_id;
    uint32_t target_id;
    EdgeType type;
    bool     on_error_path = false;
};

// --- リレーグラフ ---
struct RelayGraph {
    std::vector<GraphNode> nodes;
    std::vector<GraphEdge> edges;
};

// --- 2D座標 ---
struct Vec2 {
    float x = 0, y = 0;
    Vec2 operator+(const Vec2& o) const { return {x + o.x, y + o.y}; }
    Vec2 operator-(const Vec2& o) const { return {x - o.x, y - o.y}; }
    Vec2 operator*(float s) const { return {x * s, y * s}; }
};

// --- RGBA色 ---
struct Color {
    float r, g, b, a;
    static Color hex(uint32_t hex, float alpha = 1.0f) {
        return {
            ((hex >> 16) & 0xFF) / 255.0f,
            ((hex >> 8) & 0xFF) / 255.0f,
            (hex & 0xFF) / 255.0f,
            alpha
        };
    }
};

}  // namespace relay
```

### `src/core/config.h` — 設定定数

```cpp
#pragma once
#include "types.h"

namespace relay::config {

// --- ビジュアル ---
constexpr uint32_t BG_COLOR         = 0x0D1117;  // グラフ背景
constexpr uint32_t NODE_BG          = 0x161B22;  // ノード背景
constexpr uint32_t NODE_BORDER      = 0x30363D;
constexpr uint32_t ERROR_BORDER     = 0xE94560;  // エラーノード枠
constexpr uint32_t EDGE_CALL        = 0x4A90D9;  // 呼び出しエッジ
constexpr uint32_t EDGE_INCLUDE     = 0x2ECC71;  // includeエッジ
constexpr uint32_t EDGE_INHERIT     = 0x8E8EA0;  // 継承エッジ
constexpr uint32_t EDGE_ERROR       = 0xE94560;  // エラーパスエッジ
constexpr uint32_t TEXT_PRIMARY     = 0xE6EDF3;
constexpr uint32_t TEXT_SECONDARY   = 0x8B949E;

// --- アニメーション ---
constexpr float ANIM_EXPAND_MS     = 200.0f;   // ノード拡大時間
constexpr float ANIM_EASE_OUT      = 0.25f;    // ease-outパラメータ
constexpr float HOVER_SHADOW_GROW  = 4.0f;     // ホバー影拡張px

// --- ノードサイズ ---
constexpr float NODE_COLLAPSED_W   = 180.0f;
constexpr float NODE_COLLAPSED_H   = 100.0f;
constexpr float NODE_EXPANDED_W    = 640.0f;
constexpr float NODE_EXPANDED_H    = 420.0f;
constexpr float NODE_CORNER_RADIUS = 8.0f;

// --- ズーム ---
constexpr float ZOOM_MIN           = 0.1f;
constexpr float ZOOM_MAX           = 5.0f;
constexpr float ZOOM_SPEED         = 0.1f;

// --- グラフレイアウト ---
constexpr float LAYOUT_NODE_GAP_X  = 60.0f;
constexpr float LAYOUT_NODE_GAP_Y  = 40.0f;

}  // namespace relay::config
```

**完了条件**: `cmake -B build && cmake --build build` がエラーなしで通ること (main.cppに空のmain関数)。

---

## Phase 1: ウィンドウ基盤 + グラフキャンバス

### 目標
X11ウィンドウを開き、Cairoでダークグレー背景を描画する。マウスのパン・ズームが動作する。

### 実装指示

#### `src/platform/window.h` — ウィンドウ抽象

```cpp
#pragma once
#include <functional>
#include <string>
#include "core/types.h"

namespace relay {

struct MouseEvent {
    float x, y;
    int   button;  // 0=none, 1=left, 2=middle, 3=right
    float scroll_y;
    bool  pressed;
    bool  released;
    bool  dragging;
};

struct KeyEvent {
    int  keycode;
    bool pressed;
    bool ctrl, shift, alt;
};

class Window {
public:
    virtual ~Window() = default;
    virtual bool create(int width, int height, const std::string& title) = 0;
    virtual bool poll_events() = 0;  // false = 閉じる要求
    virtual void swap_buffers() = 0;
    virtual int  width() const = 0;
    virtual int  height() const = 0;

    // コールバック
    std::function<void(const MouseEvent&)> on_mouse;
    std::function<void(const KeyEvent&)>   on_key;
    std::function<void(int, int)>          on_resize;
};

}  // namespace relay
```

#### `src/platform/window_x11.h` / `.cpp`

X11ウィンドウを実装せよ。以下の要件:

- `XCreateSimpleWindow` でウィンドウ生成
- `cairo_xlib_surface_create` でCairoサーフェスを取得
- `XNextEvent` ループで `Expose`, `ButtonPress`, `ButtonRelease`, `MotionNotify`, `KeyPress`, `ClientMessage` (WM_DELETE_WINDOW) を処理
- マウスイベントは `MouseEvent` に変換して `on_mouse` コールバックに渡す
- **ダブルバッファリング**: バックバッファに描画してからXにフリップ

#### `src/graph/graph_view.h` / `.cpp`

グラフビューのメインクラス。

```cpp
#pragma once
#include "core/types.h"
#include "platform/renderer.h"

namespace relay {

class GraphView {
public:
    void set_graph(RelayGraph graph);
    void handle_mouse(const MouseEvent& e);
    void handle_key(const KeyEvent& e);
    void update(float dt_ms);
    void render(Renderer& renderer);

private:
    RelayGraph graph_;

    // カメラ (パン + ズーム)
    Vec2  camera_offset_ = {0, 0};
    float zoom_ = 1.0f;

    // マウス状態
    bool  panning_ = false;
    Vec2  pan_start_;
    Vec2  camera_start_;

    // 座標変換
    Vec2 screen_to_graph(Vec2 screen) const;
    Vec2 graph_to_screen(Vec2 graph) const;
};

}  // namespace relay
```

**パン**: 中ボタンドラッグで `camera_offset_` を移動。
**ズーム**: ホイールで `zoom_` を変更。マウスカーソル位置を中心にズーム。

```
zoom_new = clamp(zoom_ + scroll_y * ZOOM_SPEED, ZOOM_MIN, ZOOM_MAX)
// カーソル位置を中心にズーム
camera_offset_ = mouse_pos - (mouse_pos - camera_offset_) * (zoom_new / zoom_old)
```

#### `src/platform/renderer.h` — 描画抽象

```cpp
#pragma once
#include "core/types.h"
#include <string>

namespace relay {

class Renderer {
public:
    virtual ~Renderer() = default;

    virtual void begin_frame(int width, int height) = 0;
    virtual void end_frame() = 0;

    // プリミティブ
    virtual void fill_rect(float x, float y, float w, float h, Color color) = 0;
    virtual void fill_rounded_rect(float x, float y, float w, float h,
                                   float radius, Color color) = 0;
    virtual void stroke_rounded_rect(float x, float y, float w, float h,
                                     float radius, Color color, float line_width) = 0;
    virtual void draw_text(float x, float y, const std::string& text,
                           float size, Color color) = 0;

    // ベジェ曲線
    virtual void draw_bezier(Vec2 p0, Vec2 p1, Vec2 p2, Vec2 p3,
                             Color color, float line_width) = 0;

    // クリップ
    virtual void push_clip(float x, float y, float w, float h) = 0;
    virtual void pop_clip() = 0;

    // 変換
    virtual void push_transform(Vec2 offset, float scale) = 0;
    virtual void pop_transform() = 0;
};

}  // namespace relay
```

#### `src/platform/renderer_cairo.h` / `.cpp`

Cairoで上記インターフェースを実装。

- `cairo_t*` を保持
- `fill_rounded_rect` は `cairo_arc` 4つで角丸矩形を描画
- `draw_bezier` は `cairo_curve_to`
- `push_transform` は `cairo_save` + `cairo_translate` + `cairo_scale`
- フォント: `cairo_select_font_face("monospace", ...)`

#### `src/main.cpp`

```cpp
#include "platform/window_x11.h"
#include "platform/renderer_cairo.h"
#include "graph/graph_view.h"
#include "core/config.h"

int main(int argc, char** argv) {
    relay::WindowX11 window;
    window.create(1280, 720, "Relay Graph Editor");

    relay::RendererCairo renderer(/* cairo_t from window */);
    relay::GraphView graph_view;

    // テスト用ダミーグラフ
    relay::RelayGraph test_graph;
    // ... ダミーノード3つ追加 ...
    graph_view.set_graph(std::move(test_graph));

    window.on_mouse = [&](const relay::MouseEvent& e) {
        graph_view.handle_mouse(e);
    };

    while (window.poll_events()) {
        renderer.begin_frame(window.width(), window.height());
        renderer.fill_rect(0, 0, window.width(), window.height(),
                          relay::Color::hex(relay::config::BG_COLOR));
        graph_view.render(renderer);
        renderer.end_frame();
        window.swap_buffers();
    }
    return 0;
}
```

### 完了条件
- ダーク背景のウィンドウが表示される
- 中ボタンドラッグでパン、ホイールでズーム可能
- ダミーの矩形ノード3つがグラフ座標上に表示される

---

## Phase 2: ノードレンダリング + インタラクション

### 目標
縮小状態のノードカードをレンダリングし、ホバー・クリックによるインタラクションを実装する。

### 実装指示

#### `src/graph/graph_node.h` / `.cpp`

ノードの描画とヒットテストを担当。

```cpp
#pragma once
#include "core/types.h"
#include "platform/renderer.h"

namespace relay {

class GraphNodeRenderer {
public:
    // 縮小状態のノードを描画
    void render_collapsed(Renderer& renderer, const GraphNode& node,
                         float hover_t, float focus_t);

    // ヒットテスト (グラフ座標)
    bool hit_test(const GraphNode& node, Vec2 point) const;
};

}  // namespace relay
```

**縮小ノードの描画仕様**:

```
┌─────────────────────────────┐  ← 角丸8px
│ ● ファイル名:行番号    [種別] │  ← ヘッダ行 (フォント12px)
│─────────────────────────────│  ← 区切り線 (1px, #30363D)
│ symbol_name                 │  ← シンボル名 (フォント11px, 白)
│ コードプレビュー 2行          │  ← 薄い灰色テキスト (フォント10px)
└─────────────────────────────┘

背景: #161B22
ボーダー: #30363D (通常) / エッジ色 (ホバー時)
エラーノード: ボーダー #E94560 + 外側glow
```

**ホバーエフェクト**: `hover_t` (0.0〜1.0) で以下を補間:
- ボーダー色: `NODE_BORDER` → エッジ接続色
- 影: なし → `box-shadow 0 4px 12px rgba(0,0,0,0.5)`
- スケール: 1.0 → 1.02 (微拡大)

**フォーカスエフェクト**: 他ノードが拡大中のとき `focus_t` で:
- opacity: 1.0 → 0.3
- blur代わりにscale: 1.0 → 0.95 (Cairoにblurがないため)

#### `src/graph/graph_view.cpp` に追加

- `hovered_node_id_` を追跡。`MotionNotify` で毎フレームヒットテスト。
- `selected_node_id_` をクリックで設定。
- `hover_t_` は `update(dt)` で滑らかに補間 (lerp, 8ms/frame)。

### 完了条件
- 3つのダミーノードが縮小カードとして描画される
- マウスホバーで影とボーダーが変化する (アニメーション付き)
- エラーノードが赤いグローを持つ

---

## Phase 3: エディタ統合 (Scintilla)

### 目標
ノードクリック時に拡大アニメーションし、拡大後のノード内にScintillaエディタを表示する。

### 準備

```bash
# Scintillaをthird_partyに取得
cd third_party
wget https://www.scintilla.org/scintilla550.tgz
tar xzf scintilla550.tgz
cd scintilla/gtk
make
```

CMakeLists.txtにScintillaを追加:

```cmake
# third_party/scintilla
add_subdirectory(third_party/scintilla)  # or 手動でソース列挙
target_link_libraries(relay-editor PRIVATE scintilla)
```

### 実装指示

#### `src/graph/animation.h` / `.cpp`

```cpp
#pragma once
#include <functional>

namespace relay {

class Animation {
public:
    void start(float duration_ms);
    void update(float dt_ms);
    float progress() const;  // 0.0 ~ 1.0 (ease-out適用済み)
    bool  is_active() const;

private:
    float elapsed_ = 0;
    float duration_ = 200;
    bool  active_ = false;

    // ease-out: 1 - (1 - t)^3
    static float ease_out(float t);
};

// 2値間の補間
template<typename T>
T lerp_anim(const T& a, const T& b, float t) {
    return a + (b - a) * t;
}

}  // namespace relay
```

#### `src/editor/scintilla_wrapper.h` / `.cpp`

Scintillaをラップするクラス。

```cpp
#pragma once
#include <string>
#include <cstdint>

namespace relay {

class ScintillaWrapper {
public:
    bool initialize(/* platform-specific parent handle */);
    void destroy();

    void load_file(const std::string& path);
    void goto_line(uint32_t line);
    void highlight_line(uint32_t line, uint32_t color_rgb);
    void set_read_only(bool readonly);

    void resize(int x, int y, int width, int height);
    void set_visible(bool visible);

private:
    void* sci_ptr_ = nullptr;  // ScintillaObject*
    void send_message(unsigned int msg, uintptr_t wparam, intptr_t lparam);
};

}  // namespace relay
```

**Scintilla初期設定** (initializeで実行):

```cpp
// ダークテーマ
send(SCI_STYLESETBACK, STYLE_DEFAULT, 0x161B22);
send(SCI_STYLESETFORE, STYLE_DEFAULT, 0xE6EDF3);
send(SCI_STYLECLEARALL, 0, 0);

// 行番号
send(SCI_SETMARGINTYPEN, 0, SC_MARGIN_NUMBER);
send(SCI_SETMARGINWIDTHN, 0, 48);
send(SCI_STYLESETBACK, STYLE_LINENUMBER, 0x0D1117);
send(SCI_STYLESETFORE, STYLE_LINENUMBER, 0x8B949E);

// C/C++ シンタックスハイライト
send(SCI_SETLEXER, SCLEX_CPP, 0);
// ... キーワード設定 ...

// エラー行マーカー
send(SCI_MARKERDEFINE, 0, SC_MARK_BACKGROUND);
send(SCI_MARKERSETBACK, 0, 0x3D1E28);  // 暗い赤背景
```

#### `src/graph/graph_view.cpp` — 拡大ロジック追加

ノードクリック時のシーケンス:

```
1. selected_node_id_ = クリックされたノード
2. expand_anim_.start(ANIM_EXPAND_MS)
3. 毎フレーム update() で:
   t = expand_anim_.progress()
   ノードの描画パラメータを補間:
     x:      lerp(node.x, viewport_center_x - EXPANDED_W/2, t)
     y:      lerp(node.y, viewport_center_y - EXPANDED_H/2, t)
     width:  lerp(NODE_COLLAPSED_W, NODE_EXPANDED_W, t)
     height: lerp(NODE_COLLAPSED_H, NODE_EXPANDED_H, t)
   他ノード:
     opacity: lerp(1.0, 0.3, t)
     scale:   lerp(1.0, 0.95, t)
4. アニメーション完了時:
   scintilla_wrapper_.load_file(node.file_path)
   scintilla_wrapper_.goto_line(node.line)
   scintilla_wrapper_.highlight_line(node.line, ERROR_BORDER)
   scintilla_wrapper_.set_visible(true)
   scintilla_wrapper_.resize(展開後のノード矩形内部)
```

**縮小時** (ESCまたはノード外ダブルクリック):

```
1. scintilla_wrapper_.set_visible(false)
2. collapse_anim_.start(ANIM_EXPAND_MS)
3. 逆補間で元の位置・サイズに戻す
```

### 完了条件
- ノードクリックで200msの滑らかな拡大アニメーション
- 拡大後にScintillaエディタが表示される
- ファイルの内容が読み込まれ、エラー行がハイライトされる
- ESCで縮小アニメーションが逆再生される
- 拡大中、他のノードが半透明になる

---

## Phase 4: エッジ描画 + アニメーション

### 目標
ノード間をベジェ曲線で接続し、種別ごとの色・アニメーションを実装する。

### 実装指示

#### `src/graph/graph_edge.h` / `.cpp`

```cpp
#pragma once
#include "core/types.h"
#include "platform/renderer.h"

namespace relay {

class GraphEdgeRenderer {
public:
    void render(Renderer& renderer, const GraphEdge& edge,
                const GraphNode& source, const GraphNode& target,
                float time_sec, float focus_opacity);

private:
    // ベジェ制御点の算出
    void calc_bezier_points(const GraphNode& src, const GraphNode& dst,
                           Vec2& p0, Vec2& p1, Vec2& p2, Vec2& p3);

    // エラーパスのパーティクル描画
    void render_error_particles(Renderer& renderer,
                               Vec2 p0, Vec2 p1, Vec2 p2, Vec2 p3,
                               float time_sec);
};

}  // namespace relay
```

**ベジェ制御点の算出**:

```
ソースノードの右端中央 → ターゲットノードの左端中央
p0 = (src.x + src.width, src.y + src.height/2)
p3 = (dst.x, dst.y + dst.height/2)
dx = abs(p3.x - p0.x) * 0.5
p1 = (p0.x + dx, p0.y)
p2 = (p3.x - dx, p3.y)
```

**エッジの描画仕様**:

| 種別 | 色 | 線幅 | スタイル |
|------|------|------|----------|
| ERROR_PATH | #E94560 | 3px | 実線 + パーティクル |
| CALL | #4A90D9 | 2px | 実線 |
| INCLUDE | #2ECC71 | 1px | 破線 (dash 6,4) |
| INHERIT | #8E8EA0 | 1px | 点線 (dash 2,4) |

**エラーパスのパーティクル**: ベジェ曲線上を `t = fmod(time_sec * 0.5, 1.0)` で移動する小さな円 (r=3px, #E94560)。3つのパーティクルを等間隔で配置 (`t`, `t+0.33`, `t+0.66`)。

**接続ポイントの円**: エッジの始点・終点に小さな円 (r=4px)。ホバー時に微拡大 (r=6px)。

**フォーカス連動**: 拡大ノードがある場合、そのノードに接続されたエッジのみ `opacity=1.0`、他は `opacity=0.15`。

### 完了条件
- ノード間がベジェ曲線で接続される
- 種別ごとに色・線スタイルが異なる
- エラーパスにパーティクルアニメーションが流れる
- ノード拡大時にフォーカス外エッジが薄くなる

---

## Phase 5: エラーパーサー + Orchestrator

### 目標
コンパイラ出力文字列を解析し、ErrorInfo構造体を生成する。

### 実装指示

#### `src/core/error_parser.h` / `.cpp`

```cpp
#pragma once
#include "types.h"
#include <string>
#include <vector>
#include <regex>

namespace relay {

class ErrorParser {
public:
    std::vector<ErrorInfo> parse(const std::string& compiler_output);

private:
    // 各形式のパーサー
    bool try_gcc_clang(const std::string& line, ErrorInfo& out);
    bool try_msvc(const std::string& line, ErrorInfo& out);
    bool try_unity_csharp(const std::string& line, ErrorInfo& out);
};

}  // namespace relay
```

**対応フォーマット**:

```
# GCC/Clang
src/main.cpp:42:10: error: use of undeclared identifier 'foo'

# MSVC
src\main.cpp(42): error C2065: 'foo': undeclared identifier

# Unity C#
Assets/Scripts/Player.cs(42,10): error CS0246: The type or namespace 'Health'...
```

**正規表現パターン**:

```cpp
// GCC/Clang
std::regex gcc_re(R"(^(.+?):(\d+):(\d+):\s*(error|warning):\s*(.+)$)");

// MSVC
std::regex msvc_re(R"(^(.+?)\((\d+)\):\s*(error|warning)\s+(\w+):\s*(.+)$)");

// Unity C#
std::regex unity_re(R"(^(.+?)\((\d+),(\d+)\):\s*(error|warning)\s+(\w+):\s*(.+)$)");
```

#### `src/core/orchestrator.h` / `.cpp`

全体を統括するクラス。

```cpp
#pragma once
#include "types.h"
#include "error_parser.h"

namespace relay {

class Orchestrator {
public:
    // エラー文字列からグラフを構築
    RelayGraph build_graph_from_error(const std::string& error_string);

    // stdinパイプからの連続入力 (将来的にファイル監視)
    void watch_stdin();

private:
    ErrorParser parser_;
    // ASTAnalyzer ast_;  // Phase 6で有効化
};

}  // namespace relay
```

Phase 5時点では `build_graph_from_error` はエラー箇所のみをノードとして返す (AST解析はPhase 6)。

### テスト

```cpp
// tests/test_error_parser.cpp
void test_gcc_format() {
    ErrorParser parser;
    auto errors = parser.parse("src/main.cpp:42:10: error: use of undeclared identifier 'foo'");
    assert(errors.size() == 1);
    assert(errors[0].file_path == "src/main.cpp");
    assert(errors[0].line == 42);
    assert(errors[0].column == 10);
}

void test_unity_format() {
    ErrorParser parser;
    auto errors = parser.parse(
        "Assets/Scripts/Player.cs(42,10): error CS0246: The type or namespace 'Health' could not be found");
    assert(errors.size() == 1);
    assert(errors[0].error_code == "CS0246");
}

void test_multiline() {
    ErrorParser parser;
    std::string output = R"(
src/a.cpp:10:5: error: undeclared 'x'
src/b.cpp:20:3: error: no matching function
src/a.cpp:15:1: warning: unused variable
)";
    auto errors = parser.parse(output);
    assert(errors.size() == 2);  // warningはフィルタ可能にする
}
```

### 完了条件
- GCC/Clang, MSVC, Unity C# 形式のエラーが正しくパースされる
- 複数行入力から複数エラーを抽出できる
- テストが全て通る

---

## Phase 6: AST解析エンジン (libclang)

### 目標
エラー箇所のシンボルをAST解析し、関連するファイル・行を特定してグラフのノード・エッジを生成する。

### 準備

```bash
# libclangのインストール
sudo apt install libclang-dev llvm-dev

# CMakeLists.txt に追加
find_package(Clang REQUIRED CONFIG)
target_link_libraries(relay-editor PRIVATE clang)
```

### 実装指示

#### `src/analysis/ast_analyzer.h` / `.cpp`

```cpp
#pragma once
#include "core/types.h"
#include <string>
#include <vector>
#include <clang-c/Index.h>

namespace relay {

class ASTAnalyzer {
public:
    ASTAnalyzer();
    ~ASTAnalyzer();

    // compile_commands.json のパスを設定
    void set_compilation_database(const std::string& build_dir);

    // 指定ファイル・行のシンボルから関連ノード・エッジを構築
    // 既存のRelayGraphに追加する形式
    void analyze_error_location(const ErrorInfo& error, RelayGraph& graph);

private:
    CXIndex index_ = nullptr;
    std::string build_dir_;

    // TUをパースしてキャッシュ
    CXTranslationUnit get_or_parse_tu(const std::string& file_path);

    // 指定位置のカーソルを取得
    CXCursor get_cursor_at(CXTranslationUnit tu,
                           const std::string& file, uint32_t line, uint32_t col);

    // カーソルから参照先・呼び出し先を収集
    struct SymbolRef {
        std::string file;
        uint32_t    line;
        uint32_t    column;
        std::string name;
        NodeType    node_type;
        EdgeType    edge_type;
    };
    std::vector<SymbolRef> collect_references(CXCursor cursor);

    // AST走査コールバック
    static CXChildVisitResult visitor_callback(
        CXCursor cursor, CXCursor parent, CXClientData data);

    // TUキャッシュ
    std::unordered_map<std::string, CXTranslationUnit> tu_cache_;
};

}  // namespace relay
```

**AST解析フロー**:

```
1. ErrorInfo { file="Player.cpp", line=42, col=10 } を受け取る
2. get_or_parse_tu("Player.cpp")
   - tu_cache_ にあればそれを使う
   - なければ compile_commands.json からフラグを取得して
     clang_parseTranslationUnit() でパース
3. get_cursor_at(tu, "Player.cpp", 42, 10)
   - clang_getLocation() + clang_getCursor()
4. collect_references(cursor)
   - clang_getCursorReferenced() で定義先を取得
   - clang_visitChildren() で子カーソルを走査
     - CXCursor_CallExpr → 呼び出し先のファイル:行を取得、CALL エッジ
     - CXCursor_DeclRefExpr → 参照先を取得、REFERENCE エッジ
     - CXCursor_TypeRef → 型の定義先を取得、REFERENCE エッジ
     - CXCursor_InclusionDirective → INCLUDE エッジ
     - CXCursor_CXXBaseSpecifier → INHERIT エッジ
5. 各 SymbolRef を GraphNode として追加、エッジも追加
6. 重複ノード (同一ファイル:行) はマージ
```

**compile_commands.json の読み込み**:

```cpp
void ASTAnalyzer::set_compilation_database(const std::string& build_dir) {
    build_dir_ = build_dir;
    // clang_CompilationDatabase_fromDirectory() を使用
}
```

#### Orchestrator の更新

```cpp
RelayGraph Orchestrator::build_graph_from_error(const std::string& error_string) {
    auto errors = parser_.parse(error_string);
    RelayGraph graph;

    for (const auto& err : errors) {
        // エラーノードを作成
        GraphNode error_node;
        error_node.id = next_id_++;
        error_node.file_path = err.file_path;
        error_node.line = err.line;
        error_node.column = err.column;
        error_node.symbol_name = err.message;
        error_node.type = NodeType::ERROR_SOURCE;
        error_node.is_error = true;
        graph.nodes.push_back(error_node);

        // AST解析で関連ノードを追加
        ast_.analyze_error_location(err, graph);
    }

    return graph;
}
```

### 完了条件
- `compile_commands.json` があるプロジェクトに対してAST解析が動作する
- エラー箇所から呼び出し先・参照先が自動的にノードとして追加される
- グラフに複数ノード・エッジが生成される
- TUのキャッシュが動作する (同一ファイルの再パースが発生しない)

---

## Phase 7: アセンブリ解析 + キャッシュ

### 目標
ソース行とアセンブリアドレスの対応を解析・キャッシュし、エラー行のアセンブリを表示可能にする。

### 実装指示

#### `src/analysis/assembly_analyzer.h` / `.cpp`

```cpp
#pragma once
#include "core/types.h"
#include <string>
#include <vector>
#include <unordered_map>

namespace relay {

struct AssemblyLine {
    uint64_t    address;
    std::string instruction;
    std::string source_file;
    uint32_t    source_line;
};

class AssemblyAnalyzer {
public:
    // オブジェクトファイルを解析
    bool analyze_object(const std::string& obj_path);

    // 指定ソース行に対応するアセンブリ行を取得
    std::vector<AssemblyLine> get_assembly_for_line(
        const std::string& source_file, uint32_t line);

private:
    // llvm-objdump -d --line-numbers の出力をパース
    bool parse_objdump_output(const std::string& output);

    // DWARF行テーブル (addr → source location)
    std::unordered_map<uint64_t, std::pair<std::string, uint32_t>> line_map_;

    // 全アセンブリ行
    std::vector<AssemblyLine> asm_lines_;
};

}  // namespace relay
```

**実行コマンド**:

```cpp
// llvm-objdump でディスアセンブル + 行情報
std::string cmd = "llvm-objdump -d -l --no-show-raw-insn " + obj_path;
// popen() で実行し出力を取得
```

**出力パース例**:

```
; /path/to/source.cpp:42
  4010a0: push   rbp
  4010a1: mov    rbp, rsp
  4010a4: sub    rsp, 16
```

#### `src/cache/cache_manager.h` / `.cpp`

```cpp
#pragma once
#include <string>
#include <sqlite3.h>

namespace relay {

class CacheManager {
public:
    bool open(const std::string& db_path);
    void close();

    // AST キャッシュ
    bool has_ast_cache(const std::string& file_path, uint64_t mtime, uint64_t flags_hash);
    void store_ast_cache(const std::string& file_path, uint64_t mtime,
                        uint64_t flags_hash, const std::string& serialized_data);
    std::string load_ast_cache(const std::string& file_path);

    // アセンブリ キャッシュ
    bool has_asm_cache(const std::string& obj_path, uint64_t mtime);
    void store_asm_cache(const std::string& obj_path, uint64_t mtime,
                        const std::string& serialized_data);
    std::string load_asm_cache(const std::string& obj_path);

    // グラフレイアウト キャッシュ
    void store_layout(const std::string& graph_hash, const std::string& layout_json);
    std::string load_layout(const std::string& graph_hash);

private:
    sqlite3* db_ = nullptr;
    void create_tables();
};

}  // namespace relay
```

**SQLiteスキーマ**:

```sql
CREATE TABLE IF NOT EXISTS ast_cache (
    file_path   TEXT PRIMARY KEY,
    mtime       INTEGER,
    flags_hash  INTEGER,
    data        BLOB,
    updated_at  INTEGER DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS asm_cache (
    obj_path    TEXT PRIMARY KEY,
    mtime       INTEGER,
    data        BLOB,
    updated_at  INTEGER DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS layout_cache (
    graph_hash  TEXT PRIMARY KEY,
    layout_json TEXT,
    updated_at  INTEGER DEFAULT (strftime('%s','now'))
);
```

**キャッシュ無効化**: `stat()` でファイルの `mtime` を確認。変更があればキャッシュを破棄して再解析。

### 完了条件
- `llvm-objdump` の出力がパースされ、ソース行 → アセンブリ行のマッピングが取得できる
- SQLiteにAST/アセンブリのキャッシュが保存・読み込みされる
- 同一ファイルの再解析時にキャッシュヒットしてスキップされる

---

## Phase 8: グラフレイアウトエンジン

### 目標
ノードを自動配置するレイアウトエンジンを実装する。

### 実装指示

#### `src/graph/graph_layout.h` / `.cpp`

```cpp
#pragma once
#include "core/types.h"

namespace relay {

class GraphLayout {
public:
    // Sugiyama階層レイアウト (DAG向け)
    void layout_sugiyama(RelayGraph& graph);

    // Force-directed レイアウト (循環グラフ向け)
    void layout_force_directed(RelayGraph& graph, int iterations = 100);

    // 自動選択: DAGならSugiyama、循環があればforce-directed
    void auto_layout(RelayGraph& graph);

private:
    // --- Sugiyama ---
    // 1. 層割り当て (longest path)
    std::vector<std::vector<uint32_t>> assign_layers(const RelayGraph& graph);
    // 2. 交差最小化 (barycenter heuristic)
    void minimize_crossings(std::vector<std::vector<uint32_t>>& layers,
                           const RelayGraph& graph);
    // 3. X座標の割り当て
    void assign_x_coordinates(std::vector<std::vector<uint32_t>>& layers,
                             RelayGraph& graph);

    // --- Force-directed ---
    struct ForceState {
        std::vector<Vec2> velocities;
    };
    Vec2 calc_repulsion(const GraphNode& a, const GraphNode& b);
    Vec2 calc_attraction(const GraphNode& src, const GraphNode& dst);

    // --- 循環検出 ---
    bool has_cycle(const RelayGraph& graph);
};

}  // namespace relay
```

**Sugiyamaの簡易実装**:

```
1. assign_layers: トポロジカルソートでノードに層番号を割り当て
   - エラーノードは中央層に配置
   - 依存先は左、依存元は右

2. minimize_crossings: 各層内でバリセンター法を2-3回反復
   - ノードの y 座標 = 接続ノードの y 平均

3. assign_x_coordinates:
   - layer * (NODE_COLLAPSED_W + LAYOUT_NODE_GAP_X)
   - 層内の y 位置は barycenter 結果を使用
```

**Force-directedの簡易実装**:

```
反発力: F_rep = k_rep / dist^2 (方向: ノード間)
引力:   F_att = k_att * dist   (エッジ接続ノード間のみ)
減衰:   velocity *= 0.95

反復100回で安定化。各反復でノードのx,yを更新。
```

### 完了条件
- DAGグラフがSugiyamaで左→右に整列される
- 循環グラフがforce-directedで配置される
- エラーノードが視覚的に中央付近に配置される

---

## Phase 9: 統合 + CLI

### 目標
全コンポーネントを統合し、CLIから使用可能にする。

### 実装指示

#### CLI インターフェース

```
# 直接エラー文字列を指定
relay-editor --error "src/main.cpp:42:10: error: undeclared 'foo'"

# パイプ入力 (コンパイラ出力を直接渡す)
make 2>&1 | relay-editor --pipe

# compile_commands.json のパスを指定
relay-editor --build-dir ./build --error "..."

# アセンブリも表示
relay-editor --show-asm --error "..."
```

#### `src/main.cpp` — 最終版

```cpp
#include <iostream>
#include <string>
#include <sstream>
#include "core/orchestrator.h"
#include "graph/graph_view.h"
#include "graph/graph_layout.h"
#include "platform/window_x11.h"
#include "platform/renderer_cairo.h"
#include "cache/cache_manager.h"

struct Args {
    std::string error_string;
    std::string build_dir = ".";
    bool        pipe_mode = false;
    bool        show_asm = false;
};

Args parse_args(int argc, char** argv) {
    Args args;
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        if (arg == "--error" && i + 1 < argc) args.error_string = argv[++i];
        else if (arg == "--build-dir" && i + 1 < argc) args.build_dir = argv[++i];
        else if (arg == "--pipe") args.pipe_mode = true;
        else if (arg == "--show-asm") args.show_asm = true;
    }
    return args;
}

int main(int argc, char** argv) {
    auto args = parse_args(argc, argv);

    // パイプモード: stdinから読み込み
    if (args.pipe_mode) {
        std::ostringstream ss;
        ss << std::cin.rdbuf();
        args.error_string = ss.str();
    }

    if (args.error_string.empty()) {
        std::cerr << "Usage: relay-editor --error \"<error>\" [--build-dir <dir>] [--show-asm]\n";
        std::cerr << "       <compiler> 2>&1 | relay-editor --pipe [--build-dir <dir>]\n";
        return 1;
    }

    // キャッシュ初期化
    CacheManager cache;
    cache.open(args.build_dir + "/.relay-cache.db");

    // グラフ構築
    Orchestrator orchestrator;
    orchestrator.set_build_dir(args.build_dir);
    orchestrator.set_cache(&cache);
    auto graph = orchestrator.build_graph_from_error(args.error_string);

    // レイアウト (キャッシュ済みがあれば使用)
    GraphLayout layout;
    layout.auto_layout(graph);

    // ウィンドウ + レンダリング
    WindowX11 window;
    window.create(1280, 720, "Relay Graph Editor");
    RendererCairo renderer(/* ... */);
    GraphView graph_view;
    graph_view.set_graph(std::move(graph));

    window.on_mouse = [&](const MouseEvent& e) { graph_view.handle_mouse(e); };
    window.on_key = [&](const KeyEvent& e) { graph_view.handle_key(e); };

    // メインループ
    auto last_time = /* clock */;
    while (window.poll_events()) {
        auto now = /* clock */;
        float dt = /* elapsed ms */;
        last_time = now;

        graph_view.update(dt);

        renderer.begin_frame(window.width(), window.height());
        renderer.fill_rect(0, 0, window.width(), window.height(),
                          Color::hex(config::BG_COLOR));
        graph_view.render(renderer);
        renderer.end_frame();
        window.swap_buffers();
    }

    cache.close();
    return 0;
}
```

### キーバインド一覧

| キー | 動作 |
|------|------|
| ESC | 拡大ノードを縮小 / アプリ終了 (何も拡大されていない場合) |
| Space | Fit All (全ノードが収まるビューにアニメーション) |
| F | フォーカスモード (エラーパスのみ表示) |
| A | アセンブリビューの切り替え (拡大ノード時) |
| Tab | 次のエラーノードにフォーカス |
| Ctrl+Q | 終了 |

### 完了条件
- `make 2>&1 | relay-editor --pipe --build-dir ./build` で動作する
- エラー箇所とその関連コードがグラフとして表示される
- ノードクリックで拡大、コード確認、ESCで縮小が全て動作する
- エッジのアニメーションが動作する
- キャッシュにより2回目の起動が高速

---

## 実装順序のまとめ

```
Phase 0  プロジェクト骨格        → ビルドが通る
Phase 1  ウィンドウ+キャンバス    → ダーク背景+パン/ズーム
Phase 2  ノードカード描画        → ホバーエフェクト付きカード
Phase 3  Scintilla統合          → 拡大/縮小アニメーション+エディタ
Phase 4  エッジ描画              → ベジェ曲線+パーティクル
Phase 5  エラーパーサー          → GCC/MSVC/Unity対応
Phase 6  AST解析                → libclangで関連コード自動発見
Phase 7  アセンブリ+キャッシュ    → SQLiteキャッシュ
Phase 8  グラフレイアウト         → Sugiyama/Force-directed
Phase 9  統合+CLI               → パイプ入力で完全動作
```

各Phaseは独立してビルド・テスト可能。前のPhaseが完了してから次に進むこと。
各Phaseの完了条件を全て満たしてから次のPhaseに進むこと。
