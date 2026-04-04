# OCCT / Pure Rust デュアルバックエンド計画

## 目標

同一リポジトリ内で OCCT バックエンドと Pure Rust バックエンドを共存させ、
`feature = ["pure"]` で切り替え可能にする。両者の相互変換・比較テストも行う。

---

## 現状分析

### 現在の型とOCCT依存度

| 型 | OCCT依存 | 備考 |
|---|---|---|
| `Solid` | ◎ inner = `UniquePtr<ffi::TopoDS_Shape>` | |
| `Face` | ◎ inner = `UniquePtr<ffi::TopoDS_Face>` | |
| `Edge` | ◎ inner = `UniquePtr<ffi::TopoDS_Edge>` | |
| `FaceIterator` | ◎ `TopExp_Explorer` ラップ | |
| `EdgeIterator` | ◎ 同上 | |
| `ApproximationSegmentIterator` | △ データは`Vec<f64>`だが生成がOCCT | |
| `Shape` trait | △ メソッドシグネチャは汎用だが`FaceIterator`を返す | |
| `Boolean` | ◎ ffi呼び出し | |
| `Mesh` | ✗ 純粋なデータ構造 | そのまま共用可能 |
| `Color` | ✗ 純粋なデータ構造 | そのまま共用可能 |
| `Error` | △ バリアント名がOCCT寄り | 拡張で対応 |
| I/O関数 | ◎ STEP/BRep はOCCT前提 | pure版は別フォーマット |

### 既存の `Shape` trait (shape.rs:203-230)

```rust
pub trait Shape: Sized {
    fn translate(self, translation: DVec3) -> Self;
    fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
    fn scaled(&self, center: DVec3, factor: f64) -> Self;
    fn mirrored(&self, plane_origin: DVec3, plane_normal: DVec3) -> Self;
    fn clean(&self) -> Result<Self, Error>;
    fn volume(&self) -> f64;
    fn contains(&self, point: DVec3) -> bool;
    fn is_null(&self) -> bool;
    fn shell_count(&self) -> u32;
    fn bounding_box(&self) -> [DVec3; 2];
    fn faces(&self) -> FaceIterator;  // ← OCCT固有型を返している
    fn edges(&self) -> EdgeIterator;  // ← 同上
    fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;
    fn to_svg(&self, direction: DVec3, tolerance: f64) -> Result<String, Error>;
}
```

**問題点**: `faces()` が `FaceIterator`（OCCT固有）を返すため、trait を直接共有できない。

---

## 設計方針

### A. ディレクトリ構造

```
src/
├── lib.rs              # feature gateで occt/pure を切替、共通型を re-export
├── traits.rs           # バックエンド非依存の trait 定義
├── common/
│   ├── mod.rs
│   ├── mesh.rs         # Mesh (現 mesh.rs、変更なし)
│   ├── color.rs        # Color (現 color.rs、変更なし)
│   └── error.rs        # Error (両バックエンド共通に拡張)
├── occt/
│   ├── mod.rs          # pub use solid::Solid; etc.
│   ├── solid.rs        # 現 solid.rs → OcctSolid (+ #[cfg(test)] mod test)
│   ├── face.rs         # 現 face.rs → OcctFace  (+ #[cfg(test)] mod test)
│   ├── edge.rs         # 現 edge.rs → OcctEdge  (+ #[cfg(test)] mod test)
│   ├── shape.rs        # 現 shape.rs の impl 部分 (Boolean, SVG, Vec<Solid> impl)
│   ├── iterators.rs    # 現 iterators.rs
│   ├── ffi.rs          # 現 ffi.rs
│   ├── io.rs           # 現 io.rs (STEP/BRep)
│   └── stream.rs       # 現 stream.rs
└── pure/
    ├── mod.rs
    ├── solid.rs         # PureSolid (+ #[cfg(test)] mod test)
    ├── face.rs          # PureFace  (+ #[cfg(test)] mod test)
    ├── edge.rs          # PureEdge  (+ #[cfg(test)] mod test)
    ├── shape.rs         # Boolean ops (mesh-based CSG or half-edge)
    ├── iterators.rs     # Vec-based iterators
    └── io.rs            # STL/独自フォーマット

tests_common/                # バックエンド非依存テスト (trait契約の検証)
├── main.rs              # mod宣言 + #[test] 関数
├── primitives.rs        # box/sphere/cylinder の volume, bbox
├── transforms.rs        # translate/rotate/scale
└── helpers.rs           # 共通ヘルパー

tests_cross/                 # クロステスト (--features occt,pure)
├── main.rs
├── volume.rs            # 同一プリミティブの volume 比較
├── bbox.rs              # bounding_box 比較
└── roundtrip.rs         # occt→pure→occt 変換精度
```

### B. Trait 設計 (traits.rs)

```rust
/// バックエンド非依存の Face trait
pub trait FaceTrait {
    fn normal_at_center(&self) -> DVec3;
    fn center_of_mass(&self) -> DVec3;
    fn tshape_id(&self) -> u64;        // pure版ではインデックスなど
}

/// バックエンド非依存の Edge trait
pub trait EdgeTrait {
    fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3>;
}

/// バックエンド非依存の Solid trait
pub trait SolidTrait: Sized + Clone {
    type Face: FaceTrait;
    type Edge: EdgeTrait;

    // --- Constructors ---
    fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Self;
    fn sphere(center: DVec3, radius: f64) -> Self;
    fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Self;
    fn cone(p: DVec3, dir: DVec3, r1: f64, r2: f64, h: f64) -> Self;
    fn torus(p: DVec3, dir: DVec3, r1: f64, r2: f64) -> Self;

    // --- Transforms ---
    fn translate(self, translation: DVec3) -> Self;
    fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
    fn scaled(&self, center: DVec3, factor: f64) -> Self;
    fn mirrored(&self, plane_origin: DVec3, plane_normal: DVec3) -> Self;

    // --- Queries ---
    fn volume(&self) -> f64;
    fn bounding_box(&self) -> [DVec3; 2];
    fn contains(&self, point: DVec3) -> bool;
    fn is_null(&self) -> bool;

    // --- Topology ---
    fn faces(&self) -> Vec<Self::Face>;
    fn edges(&self) -> Vec<Self::Edge>;

    // --- Mesh ---
    fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;
}
```

**ポイント**:
- `faces()` は `Vec<Self::Face>` を返す（イテレータ型の差を吸収）
- Associated type で Face/Edge を各バックエンドが独自定義
- 既存の `Shape` trait（`Vec<Solid>` 用）は各バックエンドの impl 内で維持

### C. Feature フラグ設計 (Cargo.toml)

```toml
[features]
default = ["occt", "color"]
occt = ["dep:cxx"]          # OCCT バックエンド (既存)
pure = []                   # Pure Rust バックエンド
color = []

[[test]]
name = "common"
path = "tests_common/main.rs"

[[test]]
name = "cross"
path = "tests_cross/main.rs"
required-features = ["occt", "pure"]
```

```rust
// lib.rs
#[cfg(feature = "occt")]
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;

// デフォルトで使う型のエイリアス
#[cfg(all(feature = "occt", not(feature = "pure")))]
pub use occt::{Solid, Face, Edge};

#[cfg(all(feature = "pure", not(feature = "occt")))]
pub use pure::{Solid, Face, Edge};

// 両方有効なら両方公開（相互テスト用）
#[cfg(all(feature = "occt", feature = "pure"))]
pub use occt::{Solid as OcctSolid, Face as OcctFace, Edge as OcctEdge};
#[cfg(all(feature = "occt", feature = "pure"))]
pub use pure::{Solid as PureSolid, Face as PureFace, Edge as PureEdge};
```

### D. 相互変換

```rust
// conversion.rs (feature = ["occt", "pure"] のとき有効)

/// OCCT → Pure: メッシュ経由で変換
pub fn occt_to_pure(solid: &occt::Solid) -> pure::Solid {
    let mesh = solid.mesh_with_tolerance(0.01).unwrap();
    pure::Solid::from_mesh(&mesh)
}

/// Pure → OCCT: B-Rep データから再構築、またはメッシュ経由
pub fn pure_to_occt(solid: &pure::Solid) -> occt::Solid {
    // 方法1: Face ポリゴンから再構築
    // 方法2: STEP 文字列経由
    todo!()
}
```

### E. テスト戦略

3層構成:

#### 1. バックエンド固有 unit test → `src/occt/*.rs`, `src/pure/*.rs` 内の `#[cfg(test)] mod test`

各バックエンドの内部実装を `pub(crate)` レベルでテスト。

```rust
// src/occt/solid.rs
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn box_is_solid() {
        let s = Solid::box_from_corners(DVec3::ZERO, DVec3::ONE);
        assert!(!s.is_null());
    }
}
```

```rust
// src/pure/solid.rs
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn box_volume_analytic() {
        let s = Solid::box_from_corners(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
        assert!((s.volume() - 24.0).abs() < 1e-12);
    }
}
```

#### 2. バックエンド非依存テスト → `tests_common/`

trait の契約をジェネリックに検証。どのバックエンドでも通るべきテスト。

```
tests_common/
├── main.rs          # mod宣言 + テスト関数呼び出し
├── primitives.rs    # box/sphere/cylinder の volume, bbox
├── transforms.rs    # translate/rotate/scale の検証
└── helpers.rs       # 共通ヘルパー
```

```toml
# Cargo.toml
[[test]]
name = "common"
path = "tests_common/main.rs"
```

```rust
// tests_common/primitives.rs
use cadrum::{Solid, SolidTrait};

pub fn test_box_volume<S: SolidTrait>() {
    let s = S::box_from_corners(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
    assert!((s.volume() - 24.0).abs() < 1e-6);
}

pub fn test_sphere_volume<S: SolidTrait>() {
    let s = S::sphere(DVec3::ZERO, 1.0);
    assert!((s.volume() - 4.0 / 3.0 * std::f64::consts::PI).abs() < 1e-3);
}
```

```rust
// tests_common/main.rs
mod primitives;
mod transforms;

#[test]
fn box_volume() {
    // feature に応じて現在有効な Solid 型でテスト
    primitives::test_box_volume::<cadrum::Solid>();
}
```

#### 3. クロステスト → `tests_cross/`

両バックエンドを同時に有効にし、結果を比較。

```
tests_cross/
├── main.rs          # mod宣言
├── volume.rs        # 同一プリミティブの volume 比較
├── bbox.rs          # bounding_box 比較
└── roundtrip.rs     # occt→pure→occt 変換の精度テスト
```

```toml
# Cargo.toml
[[test]]
name = "cross"
path = "tests_cross/main.rs"
required-features = ["occt", "pure"]
```

```rust
// tests_cross/volume.rs
use cadrum::{OcctSolid, PureSolid, SolidTrait};

#[test]
fn box_volume_matches() {
    let occt = OcctSolid::box_from_corners(DVec3::ZERO, DVec3::ONE);
    let pure = PureSolid::box_from_corners(DVec3::ZERO, DVec3::ONE);
    assert!((occt.volume() - pure.volume()).abs() < 1e-6);
}

#[test]
fn roundtrip_occt_pure_occt() {
    let original = OcctSolid::sphere(DVec3::ZERO, 1.0);
    let pure = occt_to_pure(&original);
    let back = pure_to_occt(&pure);
    assert!((original.volume() - back.volume()).abs() < 0.01);
}
```

#### 実行方法

```bash
# OCCT unit test のみ
cargo test --features occt

# Pure unit test のみ
cargo test --features pure

# バックエンド非依存テスト (現在有効な方で実行)
cargo test --test common --features occt

# クロステスト (両方必要)
cargo test --test cross --features occt,pure
```

---

## 実装フェーズ

### Phase 1: リファクタリング（OCCT機能を壊さない）

1. `src/common/` を作成、`Mesh`, `Color`, `Error` を移動
2. `src/traits.rs` に `SolidTrait`, `FaceTrait`, `EdgeTrait` を定義
3. `src/occt/` を作成、既存コードを移動
4. `lib.rs` を書き換え。`feature = "occt"` (デフォルト) で既存と同じ API を維持
5. **既存テスト・examples が全て通ることを確認**

### Phase 2: Pure Rust バックエンド（最小実装）

1. `src/pure/solid.rs` — 内部表現の設計
   - 候補: Half-edge データ構造、または Mesh + CSG tree
   - 最小: 各プリミティブを解析的に保持（box = 6平面、sphere = 中心+半径 etc.）
2. プリミティブコンストラクタ: `box_from_corners`, `sphere`, `cylinder`
3. `volume()`, `bounding_box()` の解析解
4. `mesh_with_tolerance()` — プリミティブごとのメッシュ生成
5. `translate`, `rotate`, `scaled`, `mirrored` — アフィン変換行列を保持

### Phase 3: Pure Boolean 演算

- メッシュベース CSG (例: [bsp-rs](https://github.com/nicksenger/bsp-rs))
- または自前の half-edge ベース CSG
- これが最も難易度が高い。Phase 2 完了後に方針決定

### Phase 4: 相互変換 & クロステスト

1. `occt_to_pure`: OCCT mesh → Pure Solid
2. `pure_to_occt`: Pure B-Rep → Face polygon → OCCT rebuild
3. CI でクロステスト実行 (`cargo test --features occt,pure`)

---

## 注意点・リスク

- **Pure Rust で OCCT と同等の精度を出すのは非常に困難**。NURBS, fillet, chamfer などは Phase 2 のスコープ外。最初はプリミティブ + Boolean に絞る
- **既存 API の互換性**: Phase 1 で `use cadrum::Solid` が壊れないよう feature default で occt を維持
- **ビルド時間**: `feature = "occt"` がない場合、cxx/cmake/OCCT のビルドを完全にスキップできるのが Pure の大きなメリット
- **`cxx` 依存**: `cxx` を optional dependency にする必要がある（`dep:cxx`）
- **`faces()` の戻り値型**: trait で `Vec<Self::Face>` にすると OCCT 版で一時 Vec 確保が発生するが、互換性のためやむを得ない。パフォーマンスが問題なら `Box<dyn Iterator>` も検討

---

## まず着手すべきこと

**Phase 1 のステップ 1-2**: `traits.rs` の trait 定義と `common/` の分離。
これが設計の核であり、ここが固まれば残りは機械的に進む。

## メモ

Booleanと同様にMeshもSolidTraitのiteratorからconstructするようにする