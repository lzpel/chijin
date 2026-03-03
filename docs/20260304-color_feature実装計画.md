# feature "color" 実装計画：C++側カラーマップによる面の色追跡

作成: 2026-03-04

## 1. 目的

`feature = "color"` を有効にすると、`Shape` がC++側に `NCollection_DataMap<TopoDS_Shape, RGB>` を保持し、ブール演算・`clean()`・`translated()` を通じて面ごとの色が自動的に引き継がれるようにする。

将来的には STEP の色付き読み書き（XDE）にも対応するが、本計画ではまず **ブール演算・変換における色リレー** に焦点を当てる。

### 1.1 実装の戦略は2通りある

>feature colorをつけてコンパイルするとpub struct Shape {
>pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
>}にpub colormap: HashMap<TShape*, RGB>が追加される。
>Shapeのclean()やブーリアン演算でそれが保持される。
>STEP読み込み書き込みでそれが保持される。（形全体への色はすべての面への色へ変換される）
>このライブラリの使用者がgltf出力などを書くときにcolormapを利用できる
>という修正は可能ですか？

可能です。ただし、いくつかの設計上の判断が必要です。整理します。

実現可能性: ✅ 可能
全体の構成はこうなります：

```rust
#[cfg(feature = "color")]
use std::collections::HashMap;
pub struct Shape {
    pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
    #[cfg(feature = "color")]
    pub colormap: HashMap<u64, [u8; 3]>,  // ← TShape* ではなく u64 ハッシュ
}
```

なぜ TShape* ではなく u64 か
TShape* は C++ のポインタなので Rust の HashMap のキーにするには FFI 越しにハッシュ値を取るのが自然です。OCCT には TopoDS_Shape の HashCode() がありますが、衝突の可能性があるため、より安全な方法は C++ 側で TopTools_DataMapOfShapeInteger（IsSame() ベースのマップ）を使うことです。

難易度の内訳
機能	難易度	理由
ブール演算での色リレー	中	Modified() は既に使っている。deep_copy前後の対応づけが必要

clean()
 での色リレー	中	ShapeUpgrade_UnifySameDomain も Modified() を持つ
STEP 読み込み（色付き）	高	STEPCAFControl_Reader + XCAFDoc_ColorTool + TDocStd_Document 等の新FFIが大量に必要
STEP 書き出し（色付き）	高	同上（STEPCAFControl_Writer）
Shape全体色 → 全Face色への展開	低	Rust側で全faceに同色をsetするだけ
2つの実装戦略
戦略1: C++側にカラーマップを持つ（推奨）

```cpp
// wrapper.h に追加
struct ColoredShape {
    TopoDS_Shape shape;
    // IsSame() ベースの OCCT 純正マップ
    NCollection_DataMap<TopoDS_Shape, std::array<uint8_t,3>,
                        TopTools_ShapeMapHasher> color_map;
};
```

C++ 側でブール演算時に Modified() を使って自動リレー
Rust 側は ColoredShape を cxx 経由で操作
メリット: IsSame() ベースなので衝突なし、deep_copy 後もC++内で対応づけ可能

**戦略2: Rust側にカラーマップを持つ**

```rust
pub colormap: Vec<([f64; 3], [u8; 3])>,  // (法線or重心, RGB)
```

C++ 側の変更が最小限
ブール演算ごとに Rust 側で face 列挙してリマップ
メリット: FFI 変更が少ない。デメリット: ポインタベースの正確な追跡ができない
最大のハードル: STEP I/O
正直に言うと、STEP の色付き読み書きが最も工数が大きいです。必要な新しい C++ バインディング：

TDocStd_Document, TDF_Label, XCAFDoc_ShapeTool, XCAFDoc_ColorTool,
STEPCAFControl_Reader, STEPCAFControl_Writer, Quantity_Color,
Handle(TDocStd_Document), XCAFApp_Application, ...
もし STEP I/O の色対応を後回しにして、まずブール演算 + clean() での色リレーだけを実装するなら、工数はかなり抑えられます。ユーザーが直方体を Shape::box_from_corners() で作り、手動で colormap を設定 → stretch → glTF出力、という流れなら STEP I/O は不要です。

## 2. 設計概要

### 2.1 データ構造

```
C++ 側
┌─────────────────────────────────────────────────┐
│ class ColorMap {                                │
│   NCollection_DataMap<TopoDS_Shape,             │
│                       std::array<uint8_t,3>,    │
│                       TopTools_ShapeMapHasher>   │
│      map_;  // IsSame() ベースの検索             │
│ };                                              │
└─────────────────────────────────────────────────┘
         ↑ UniquePtr で Rust が所有
┌─────────────────────────────────────────────────┐
│ Rust 側                                         │
│ pub struct Shape {                              │
│     pub(crate) inner: UniquePtr<TopoDS_Shape>,  │
│     #[cfg(feature = "color")]                   │
│     pub(crate) colors: UniquePtr<ColorMap>,      │
│ }                                               │
└─────────────────────────────────────────────────┘
```

**`TopTools_ShapeMapHasher`** を使うことで、`TopoDS_Shape` の `IsSame()` に基づいた検索が可能。`HashCode()` の衝突を `IsEqual()` が解決するため、一意性が保証される。

### 2.2 色リレーの仕組み

ブール演算 (`union` / `subtract` / `intersect`) において：

```
1. 演算前: 入力 Shape A の全 Face を列挙
2. 演算実行（BRepAlgoAPI_*）
3. deep_copy 前に、各入力 Face について:
   a. op.Modified(face) → 変形後 Face のリスト → 同じ色を割り当て
   b. op.IsDeleted(face) → 削除された場合はスキップ
   c. Modified() が空 & 未削除 → そのまま残った → 同じ色を割り当て
4. op.Generated(face) → 新規切断面 → デフォルト色 or 色なし
5. deep_copy 実行 → TShape ポインタが変わる
6. deep_copy 前後の Face を列挙順で対応づけ、色を移し替え
```

### 2.3 API

```rust
// 色の設定（ユーザーが手動で色を付ける）
#[cfg(feature = "color")]
impl Shape {
    /// 指定した面に色を設定する
    pub fn set_face_color(&mut self, face: &Face, rgb: [u8; 3]);

    /// 全面に同一色を設定する
    pub fn set_all_faces_color(&mut self, rgb: [u8; 3]);

    /// 指定した面の色を取得する（未設定なら None）
    pub fn face_color(&self, face: &Face) -> Option<[u8; 3]>;

    /// 面とその色のイテレータを返す
    pub fn colored_faces(&self) -> impl Iterator<Item = (Face, Option<[u8; 3]>)>;
}
```

## 3. 変更対象ファイル

### 3.1 Cargo.toml

```diff
 [features]
 default = ["bundled"]
 bundled = []
 prebuilt = []
+color = []
```

---

### 3.2 cpp/wrapper.h — ColorMap クラス追加

```cpp
#include <NCollection_DataMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>

namespace chijin {

using RGB = std::array<uint8_t, 3>;

class ColorMap {
public:
    void set(const TopoDS_Shape& face, uint8_t r, uint8_t g, uint8_t b);
    bool get(const TopoDS_Shape& face, uint8_t& r, uint8_t& g, uint8_t& b) const;
    void clear();
    int size() const;

    // ブール演算の色リレー
    // op の Modified() を使って old_map の色を new_map に転写する
    static std::unique_ptr<ColorMap> relay_boolean(
        BRepAlgoAPI_BooleanOperation& op,
        const TopoDS_Shape& input_shape,
        const ColorMap& old_map);

    // deep_copy 前後の色リマップ（面の列挙順で対応づけ）
    static std::unique_ptr<ColorMap> remap_after_copy(
        const TopoDS_Shape& before_copy,
        const TopoDS_Shape& after_copy,
        const ColorMap& src);

    // clean() (UnifySameDomain) の色リレー
    static std::unique_ptr<ColorMap> relay_clean(
        ShapeUpgrade_UnifySameDomain& unifier,
        const TopoDS_Shape& input_shape,
        const ColorMap& old_map);

private:
    NCollection_DataMap<TopoDS_Shape, RGB, TopTools_ShapeMapHasher> map_;
};

} // namespace chijin
```

---

### 3.3 cpp/wrapper.cpp — ColorMap 実装 + 既存関数の `_colored` バリアント

ブール演算関数は feature 有効時に色リレー付きバリアントを追加：

```cpp
// 既存の boolean_fuse / boolean_cut / boolean_common はそのまま維持

// 色リレー付きバリアント（feature "color" 用）
std::unique_ptr<BooleanShapeColored> boolean_fuse_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors);

std::unique_ptr<BooleanShapeColored> boolean_cut_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors);

std::unique_ptr<BooleanShapeColored> boolean_common_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors);

// clean の色リレー付きバリアント
std::unique_ptr<CleanedShapeColored> clean_shape_colored(
    const TopoDS_Shape& shape, const ColorMap& colors);

// translate の色リレー（TShape は translate 後も IsSame() == true なので単純コピー）
std::unique_ptr<ColorMap> translate_colormap(
    const ColorMap& colors);
```

---

### 3.4 src/ffi.rs — cxx ブリッジへの追加

```rust
// feature "color" 時のみ追加される型と関数
#[cfg(feature = "color")]
unsafe extern "C++" {
    type ColorMap;

    fn colormap_new() -> UniquePtr<ColorMap>;
    fn colormap_set(map: Pin<&mut ColorMap>, face: &TopoDS_Face, r: u8, g: u8, b: u8);
    fn colormap_get(map: &ColorMap, face: &TopoDS_Face, r: &mut u8, g: &mut u8, b: &mut u8) -> bool;
    fn colormap_size(map: &ColorMap) -> i32;

    // 色リレー付きブール演算
    type BooleanShapeColored;
    fn boolean_fuse_colored(...) -> UniquePtr<BooleanShapeColored>;
    fn boolean_cut_colored(...) -> UniquePtr<BooleanShapeColored>;
    fn boolean_common_colored(...) -> UniquePtr<BooleanShapeColored>;
    // ...
}
```

---

### 3.5 src/shape.rs — Shape 構造体の拡張

```rust
pub struct Shape {
    pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
    #[cfg(feature = "color")]
    pub(crate) colors: cxx::UniquePtr<ffi::ColorMap>,
}
```

既存の全メソッド（コンストラクタ、ブール演算、`clean()`、`translated()`、`deep_copy()`）に `#[cfg(feature = "color")]` 分岐を追加し、`colors` フィールドも適切に処理する。

**重要**: `feature = "color"` が無効の場合、既存のコードは一切変更されない。

---

### 3.6 新規ファイル: src/color.rs

`#[cfg(feature = "color")]` で保護されたモジュール。ユーザー向け API をここにまとめる。

---

### 3.7 テスト: tests/color.rs（新規）

```rust
#[cfg(feature = "color")]
mod color_tests {
    use chijin::Shape;
    use glam::DVec3;

    #[test]
    fn test_set_and_get_face_color() {
        let shape = Shape::box_from_corners(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 10.0),
        );
        // 各面に異なる色を設定
        let colors = [[255,0,0], [0,255,0], [0,0,255],
                      [255,255,0], [255,0,255], [0,255,255]];
        let mut shape = shape;
        for (face, color) in shape.faces().zip(colors.iter()) {
            shape.set_face_color(&face, *color);
        }
        // 取得して一致確認
        for (face, expected) in shape.faces().zip(colors.iter()) {
            assert_eq!(shape.face_color(&face), Some(*expected));
        }
    }

    #[test]
    fn test_boolean_preserves_colors() {
        let mut a = Shape::box_from_corners(...);
        a.set_all_faces_color([255, 0, 0]);  // 赤
        let b = Shape::box_from_corners(...);
        let result: Shape = a.subtract(&b).unwrap().into();
        // 元の面から引き継がれた面は赤のまま
        for face in result.faces() {
            if let Some(color) = result.face_color(&face) {
                assert_eq!(color, [255, 0, 0]);
            }
            // 新規切断面は色なし (None) でもよい
        }
    }

    #[test]
    fn test_clean_preserves_colors() { ... }

    #[test]
    fn test_translated_preserves_colors() { ... }

    #[test]
    fn test_stretch_preserves_colors() {
        // 直方体の6面に異なる色 → stretch → 6面の色が保持
    }
}
```

## 4. 実装順序

| フェーズ | 内容 | 見積り |
|---------|------|--------|
| **Phase 1** | `ColorMap` C++ クラス + FFI + 基本 API (`set`/`get`) | 小 |
| **Phase 2** | ブール演算の色リレー (`Modified()` + deep_copy 後リマップ) | 中 |
| **Phase 3** | `clean()` の色リレー (`UnifySameDomain::Generated/Modified`) | 中 |
| **Phase 4** | `translated()` / `deep_copy()` の色保持 | 小 |
| **Phase 5** | テスト + `examples/stretch_colored.rs` | 中 |
| **Phase 6** *(将来)* | STEP XDE 色付き読み書き | 大 |

## 5. 既存コードへの影響

- **`feature = "color"` 無効時**: 変更なし。既存の全テストがそのまま通る。
- **`feature = "color"` 有効時**: `Shape` のサイズが `UniquePtr<ColorMap>` 分（8バイト）増加。ブール演算に `Modified()` の追加呼び出しが入るが、パフォーマンスへの影響は軽微。
- **破壊的変更**: なし。全ての既存 API は互換性を維持。

## 6. 検証計画

### 自動テスト

```bash
# feature 無効で既存テストが通ること
cargo test

# feature 有効で色関連テストが通ること
cargo test --features color

# stretch_colored example の実行
cargo run --example stretch_colored --features "bundled,color"
```

### 手動検証

1. `examples/stretch_colored.rs` を実行し、出力ファイルを確認
2. 出力された STEP/BRep ファイルを FreeCAD 等で開き、面の色が preserve されていることを目視確認（Phase 6 以降）

## 7. 未決事項

1. **`clean()` の `Modified()` の信頼性**: `ShapeUpgrade_UnifySameDomain` の `Generated()` / `Modified()` がどの程度信頼できるか要調査。統合により消滅した面の色はどうするか（最初に見つかった色を使う？）⇒最初にみつかった色を使ってください
2. **BooleanShape の色**: `BooleanShape` 構造体はどうするか。 `shape` と `new_faces` それぞれに `ColorMap` を持たせるか、`shape` のみか⇒それぞれに持たせてください
3. **`cxx` と `NCollection_DataMap` の相性**: `cxx` は opaque C++ 型として `ColorMap` を扱えるが、`NCollection_DataMap` のテンプレートが cxx-build でコンパイルエラーにならないか要検証
4. **deep_copy 前後の面列挙順保証**: `TopExp_Explorer` の列挙順が `BRepBuilderAPI_Copy` 前後で保存される前提だが、OCCT ドキュメントに明示的な保証はない。実験的に検証する必要がある
