# Shape 再設計 — `type Shape = Vec<Solid>`

## Context

現行の `struct Shape` は内部に `TopoDS_Shape` を持ち、compound/solid/face 等の区別がない。
`into_solids`/`from_solids` を毎回手動で呼ぶ必要がある。
ベンチマークで `from_solids + into_solids` のコストがゼロと確認済みなので、
これを内部化し `type Shape = Vec<Solid>` に再設計する。

---

## 新しい型の構成

### 残る型（変更なし）
```
TShapeId, Rgb, Face, Edge, Mesh, Error
FaceIterator, EdgeIterator, ApproximationSegmentIterator
stream::RustReader, stream::RustWriter
```

### 変わる型

#### `Solid` (src/solid.rs) — 拡張

現行は空に近い struct。Shape が持っていたフィールドと単体操作を移動。

```rust
pub struct Solid {
    pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
    #[cfg(feature = "color")]
    pub colormap: HashMap<TShapeId, Rgb>,
}

impl Solid {
    // --- コンストラクタ ---
    pub fn half_space(origin: DVec3, normal: DVec3) -> Solid;
    pub fn box_from_corners(c1: DVec3, c2: DVec3) -> Solid;
    pub fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Solid;

    // --- 単体操作 ---
    pub fn deep_copy(&self) -> Solid;
    pub fn translated(&self, t: DVec3) -> Solid;
    pub fn rotated(&self, origin: DVec3, dir: DVec3, angle: f64) -> Solid;
    pub fn scaled(&self, center: DVec3, factor: f64) -> Solid;
    pub fn clean(&self) -> Result<Solid, Error>;

    // --- クエリ ---
    pub fn volume(&self) -> f64;
    pub fn is_null(&self) -> bool;
    pub fn shell_count(&self) -> u32;
    pub fn contains(&self, point: DVec3) -> bool;
    pub fn faces(&self) -> FaceIterator;
    pub fn edges(&self) -> EdgeIterator;
    pub fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;

    // --- カラー ---
    #[cfg(feature = "color")]
    pub fn paint(&mut self, color: Rgb);
}
```

#### `Shape` (src/shape.rs) — type alias

```rust
pub type Shape = Vec<Solid>;
```

#### `ShapeTrait` (src/shape.rs) — trait

```rust
pub trait ShapeTrait {
    // --- コンストラクタ（Vec<Solid> を返す） ---
    fn half_space(origin: DVec3, normal: DVec3) -> Shape;
    fn box_from_corners(c1: DVec3, c2: DVec3) -> Shape;
    fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Shape;
    fn empty() -> Shape;  // = vec![]

    // --- I/O ---
    fn read_step(r: &mut impl Read) -> Result<Shape, Error>;
    fn read_brep_bin(r: &mut impl Read) -> Result<Shape, Error>;
    fn read_brep_text(r: &mut impl Read) -> Result<Shape, Error>;
    fn write_step(&self, w: &mut impl Write) -> Result<(), Error>;
    fn write_brep_bin(&self, w: &mut impl Write) -> Result<(), Error>;
    fn write_brep_text(&self, w: &mut impl Write) -> Result<(), Error>;
    #[cfg(feature = "color")]
    fn read_step_with_colors(r: &mut impl Read) -> Result<Shape, Error>;
    #[cfg(feature = "color")]
    fn write_step_with_colors(&self, w: &mut impl Write) -> Result<(), Error>;
    #[cfg(feature = "color")]
    fn read_brep_color(r: &mut impl Read) -> Result<Shape, Error>;
    #[cfg(feature = "color")]
    fn write_brep_color(&self, w: &mut impl Write) -> Result<(), Error>;

    // --- Boolean 演算 ---
    fn union(&self, other: &Shape) -> Result<BooleanShape, Error>;
    fn subtract(&self, other: &Shape) -> Result<BooleanShape, Error>;
    fn intersect(&self, other: &Shape) -> Result<BooleanShape, Error>;

    // --- 変換 (各 Solid に適用して Vec を返す) ---
    fn translated(&self, t: DVec3) -> Shape;
    fn rotated(&self, origin: DVec3, dir: DVec3, angle: f64) -> Shape;
    fn scaled(&self, center: DVec3, factor: f64) -> Shape;
    fn deep_copy(&self) -> Shape;
    fn clean(&self) -> Result<Shape, Error>;

    // --- 集約クエリ ---
    fn volume(&self) -> f64;          // sum
    fn contains(&self, p: DVec3) -> bool;  // any
    fn is_null(&self) -> bool;        // all null or empty
    fn shell_count(&self) -> u32;     // sum

    // --- 描画系 (内部で compound してから処理) ---
    fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;
    fn to_svg(&self, dir: DVec3, tol: f64) -> Result<String, Error>;

    // --- カラー ---
    #[cfg(feature = "color")]
    fn paint(&mut self, color: Rgb);
}
```

#### `BooleanShape` (src/shape.rs) — フィールド変更

```rust
pub struct BooleanShape {
    pub solids: Shape,     // 旧: pub shape: Shape (struct)
    from_a: Vec<u64>,
    from_b: Vec<u64>,
}

impl From<BooleanShape> for Shape {  // = impl From<BooleanShape> for Vec<Solid>
    fn from(r: BooleanShape) -> Shape { r.solids }
}
```

### 消える型・メソッド

| 削除対象 | 理由 |
|----------|------|
| `struct Shape` | `type Shape = Vec<Solid>` に置換 |
| `Shape::from_solids()` | 内部ヘルパー `to_compound()` に移行、非公開 |
| `Shape::into_solids()` | 内部ヘルパー `decompose()` に移行、非公開 |
| `impl From<Solid> for Shape` | 不要 (`vec![solid]` で十分) |
| `Shape::set_global_translation()` | `translated()` で代替可 |

---

## 内部ヘルパー（非公開）

```rust
// shape.rs 内 (pub(crate) or private)

/// Vec<Solid> → TopoDS_Compound
fn to_compound(solids: &[Solid]) -> UniquePtr<ffi::TopoDS_Shape>;

/// TopoDS_Shape (compound) → Vec<Solid>
fn decompose(
    compound: &ffi::TopoDS_Shape,
    #[cfg(feature = "color")] colormap: &HashMap<TShapeId, Rgb>,
) -> Vec<Solid>;
```

---

## ShapeTrait メソッドの内部実装パターン

### Boolean 演算の例
```rust
fn union(&self, other: &Shape) -> Result<BooleanShape, Error> {
    let c_self = to_compound(self);    // 内部 from_solids
    let c_other = to_compound(other);
    let r = ffi::boolean_fuse(&c_self, &c_other);
    // ... build BooleanShape with decompose() ...
}
```

### I/O の例
```rust
fn read_step(reader: &mut impl Read) -> Result<Shape, Error> {
    let compound = ffi::read_step_stream(...);
    Ok(decompose(&compound, ...))      // 内部 into_solids
}

fn write_step(&self, writer: &mut impl Write) -> Result<(), Error> {
    let compound = to_compound(self);  // 内部 from_solids
    ffi::write_step_stream(&compound, ...);
}
```

### 変換の例
```rust
fn translated(&self, t: DVec3) -> Shape {
    self.iter().map(|s| s.translated(t)).collect()
}
```

---

## ファイル変更マップ

| ファイル | 変更内容 |
|----------|----------|
| `src/solid.rs` | Solid 拡張 (colormap + 全単体メソッド追加) |
| `src/shape.rs` | `struct Shape` → `type Shape`, ShapeTrait 定義+impl, BooleanShape 更新, 旧 Shape メソッドを ShapeTrait に移行 |
| `src/lib.rs` | `pub use shape::ShapeTrait` 追加, `pub use solid::Solid` 維持 |
| `src/utils.rs` | Shape → ShapeTrait ベースに更新 |
| `src/face.rs` | `Face::extrude` → `Result<Solid, Error>` 維持 (変更なし) |
| `tests/*.rs` | `Shape::` → `Shape::` (ShapeTrait import 追加), `into_solids`/`from_solids` 呼び出し削除 |

---

## 利用側の変化

```rust
// 旧 API
let cyl = Shape::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);
let half = Shape::half_space(origin, normal);
let result: Shape = cyl.intersect(&half)?.into();
let solids = result.into_solids();
let compound = Shape::from_solids(solids);

// 新 API
use chijin::ShapeTrait;
let cyl = Shape::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);  // Vec<Solid>
let half = Shape::half_space(origin, normal);                   // Vec<Solid>
let result: Shape = cyl.intersect(&half)?.into();               // Vec<Solid> 直接
// into_solids/from_solids 不要 — Shape は既に Vec<Solid>
result.len()  // ソリッド数
```

---

## 検証方法

```bash
# 全テスト
cargo test

# ベンチマーク — B の overhead が引き続き 0ms であること
cargo test bench_into_solids -- --nocapture

# カラーテスト
cargo test --features color
```
