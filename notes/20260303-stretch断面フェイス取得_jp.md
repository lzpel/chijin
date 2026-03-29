# stretch における切断面取得の再設計：Generated() 活用案 (2026-03-03)

## 問題の整理

### 現状のフロー

`stretch_vector` は次の順序で動く。

```
① shape.intersect(half_space)  → part_neg
② shape.subtract(half_space)   → part_pos（平行移動）
③ extrude_cut_faces(part_neg)  → filler（heuristic で断面を探索）
④ part_neg.union(filler).union(part_pos)
```

③ の `extrude_cut_faces` が問題の核心。
`part_neg` の全フェイスを走査し、2 つの閾値でフィルタしている。

```rust
const NORMAL_THRESHOLD: f64 = 0.99;   // 法線の向きが切断方向とほぼ平行
const COORD_TOLERANCE: f64 = 0.5;     // 面の重心が切断平面から 0.5 mm 以内

normal.dot(plane_normal).abs() > NORMAL_THRESHOLD
    && (center - origin).dot(plane_normal).abs() < COORD_TOLERANCE
```

これが暫定的な heuristic であり、精度・頑健性に問題がある。

### なぜ OCCT は断面フェイスを知っているのか

`BRepAlgoAPI_Common` は演算完了後に **履歴テーブル** を保持している。

```cpp
BRepAlgoAPI_Common common(shape, half_space);
common.Build();

// half_space の境界フェイスから Generated されたフェイスが断面そのもの
const TopTools_ListOfShape& cut_faces = common.Generated(tool_face);
```

OCCT の Boolean 演算は「どのフェイスがどの入力フェイスから生成されたか」を
内部的に完全に追跡している。切断平面（half_space の境界面）から `Generated` した
フェイスがそのまま断面フェイスである。閾値も走査も不要。

### なぜ現状はこれを捨てているのか

`boolean_common` の実装は `Build()` 直後に `BRepBuilderAPI_Copy` を呼ぶ。

```cpp
BRepAlgoAPI_Common common(a, b);
common.Build();
// ↓ この行で Generated テーブルへのアクセスが断ち切られる
BRepBuilderAPI_Copy copier(common.Shape(), Standard_True, Standard_False);
return std::make_unique<TopoDS_Shape>(copier.Shape());
```

`BRepBuilderAPI_Copy` は Bug 1 fix として導入した必須の処理だが、
`Generated()` は `BRepAlgoAPI_Common` オブジェクト自体が保持しており、
コピー後のシェイプからは参照できない。

---

## Option C の設計

`BRepAlgoAPI_Common` オブジェクトが生きている間に `Generated()` を呼び、
断面フェイスを個別に deep copy してから両方まとめて返す。

### 新しい C++ オペーク型 `IntersectResult`

```cpp
// wrapper.h に追加
namespace chijin {
    class IntersectResult;  // opaque — shape + cut_faces を保持
}
```

```cpp
// wrapper.cpp に追加
class IntersectResult {
public:
    TopoDS_Shape shape;            // intersect の結果（deep copy 済み）
    TopoDS_Shape cut_faces;        // 断面フェイスの Compound（deep copy 済み）
};
```

### 新しい C++ 関数

```cpp
std::unique_ptr<IntersectResult> intersect_with_cut_faces(
    const TopoDS_Shape& shape,
    const TopoDS_Shape& half_space)
{
    try {
        BRepAlgoAPI_Common common(shape, half_space);
        common.Build();
        if (!common.IsDone()) return nullptr;

        // ① Generated faces を copy 前に収集
        BRep_Builder builder;
        TopoDS_Compound raw_cut;
        builder.MakeCompound(raw_cut);
        for (TopExp_Explorer ex(half_space, TopAbs_FACE); ex.More(); ex.Next()) {
            for (auto it = common.Generated(ex.Current()).begin();
                      it != common.Generated(ex.Current()).end(); ++it) {
                builder.Add(raw_cut, *it);
            }
        }

        // ② メインシェイプを deep copy（Bug 1 fix）
        BRepBuilderAPI_Copy shape_copy(common.Shape(), Standard_True, Standard_False);

        // ③ 断面フェイスを個別に deep copy
        BRep_Builder builder2;
        TopoDS_Compound cut_copy;
        builder2.MakeCompound(cut_copy);
        for (TopExp_Explorer ex(raw_cut, TopAbs_FACE); ex.More(); ex.Next()) {
            BRepBuilderAPI_Copy fc(ex.Current(), Standard_True, Standard_False);
            builder2.Add(cut_copy, fc.Shape());
        }

        auto result = std::make_unique<IntersectResult>();
        result->shape    = shape_copy.Shape();
        result->cut_faces = cut_copy;
        return result;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// アクセサ（cxx bridge からは直接フィールドアクセスできないため）
std::unique_ptr<TopoDS_Shape> intersect_result_shape(const IntersectResult& r) {
    return std::make_unique<TopoDS_Shape>(r.shape);
}
std::unique_ptr<TopoDS_Shape> intersect_result_cut_faces(const IntersectResult& r) {
    return std::make_unique<TopoDS_Shape>(r.cut_faces);
}
```

### cxx bridge への追加（`src/ffi.rs`）

```rust
unsafe extern "C++" {
    type IntersectResult;

    fn intersect_with_cut_faces(
        shape: &TopoDS_Shape,
        half_space: &TopoDS_Shape,
    ) -> UniquePtr<IntersectResult>;

    fn intersect_result_shape(r: &IntersectResult) -> UniquePtr<TopoDS_Shape>;
    fn intersect_result_cut_faces(r: &IntersectResult) -> UniquePtr<TopoDS_Shape>;
}
```

`unsafe impl Send for IntersectResult {}` も追加する。

### `Shape::intersect_with_cut_faces`（`src/shape.rs`）

```rust
/// intersect の結果と、切断平面が生成した断面フェイスの Compound を同時に返す。
pub fn intersect_with_cut_faces(&self, other: &Shape) -> Result<(Shape, Shape), Error> {
    let r = ffi::intersect_with_cut_faces(&self.inner, &other.inner);
    if r.is_null() {
        return Err(Error::BooleanOperationFailed);
    }
    let shape_inner = ffi::intersect_result_shape(&r);
    let faces_inner = ffi::intersect_result_cut_faces(&r);
    if shape_inner.is_null() || faces_inner.is_null() {
        return Err(Error::BooleanOperationFailed);
    }
    Ok((Shape { inner: shape_inner }, Shape { inner: faces_inner }))
}
```

---

## stretch_vector の変更

### 変更前

```rust
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
    let half = Shape::half_space(origin, delta.normalize());
    let part_neg = shape.intersect(&half)?;
    let part_pos = shape.subtract(&half)?.translated(delta);
    let filler = extrude_cut_faces(&part_neg, origin, delta)?;  // heuristic
    part_neg.union(&filler)?.union(&part_pos)
}
```

### 変更後

```rust
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
    let half = Shape::half_space(origin, delta.normalize());
    let (part_neg, cut_faces) = shape.intersect_with_cut_faces(&half)?;
    let part_pos = shape.subtract(&half)?.translated(delta);
    let filler = extrude_faces(&cut_faces, delta)?;
    part_neg.union(&filler)?.union(&part_pos)
}

/// 断面フェイスの Compound を押し出してフィラーを作る。
fn extrude_faces(cut_faces: &Shape, delta: DVec3) -> Result<Shape, Error> {
    let mut filler: Option<Shape> = None;
    for face in cut_faces.faces() {
        let extruded = Shape::from(face.extrude(delta)?);
        filler = Some(match filler {
            None => extruded,
            Some(f) => f.union(&extruded)?,
        });
    }
    Ok(filler.unwrap_or_else(Shape::empty))
}
```

---

## 削除されるもの

| 削除対象 | 理由 |
|---|---|
| `NORMAL_THRESHOLD = 0.99` | Generated() が正確な面を返すので不要 |
| `COORD_TOLERANCE = 0.5` | 同上 |
| `extrude_cut_faces` 関数（全走査版） | `extrude_faces`（直接版）に置き換え |
| stretch 中の `face.normal_at_center()` 呼び出し | heuristic の一部だったため不要に |
| stretch 中の `face.center_of_mass()` 呼び出し | 同上 |

`face_normal_at_center` / `face_center_of_mass` 自体はライブラリ API として残す。

---

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `cpp/wrapper.h` | `IntersectResult` クラス宣言を追加 |
| `cpp/wrapper.cpp` | `IntersectResult` 実装、`intersect_with_cut_faces`、アクセサ 2 関数を追加 |
| `src/ffi.rs` | `IntersectResult` opaque 型、FFI 関数 3 つを追加 |
| `src/shape.rs` | `intersect_with_cut_faces` メソッドを追加 |
| `tests/stretch_box.rs` | `stretch_vector` を変更、`extrude_cut_faces` を `extrude_faces` に置き換え、定数 2 つを削除 |

既存の `intersect` / `subtract` / `boolean_common` / `boolean_cut` は変更しない。
新関数は stretch 専用ではなく `Shape` の汎用メソッドとして公開する。

---

## 発展案：全 Boolean 演算を `BooleanShape` に統一する

### 各演算で `Generated()` が返すもの

OCCT の Boolean 演算ごとに「生成されるフェイス」の意味が異なる。

| 演算 | `Generated(tool_face)` の意味 | stretch での用途 |
|---|---|---|
| `intersect` (Common) | ツール境界面が元形状を切断して生んだ断面フェイス | ◎ フィラー押し出しに直接使う |
| `subtract` (Cut) | 同上（ただし part_pos 側に現れる、向きが反対） | ○ 同じ断面の裏側。どちらでも使える |
| `union` (Fuse) | 内部境界部で修正されたフェイス。"新たに切断して生んだ面" ではない | — 使わない。**空 Compound を返す** |

`subtract` の `Generated` は `intersect` の `Generated` と幾何的に同一の断面だが、
法線の向きが逆になる（part_pos 側から見た面）。
`BRepPrimAPI_MakePrism` はどちらの向きでも正しくプリズムを生成するので
どちらを使っても動作上の差はない。`intersect` 側を使うのが自然。

`union` については「切断平面が面を生んだ」という概念が成立しない。
内部では修正フェイス（`Modified()`）が存在するが、それは元のフェイスに
新たなエッジが刻まれた版であり、stretch のフィラーとは無関係。
統一型では `new_faces` を空 Compound として返す。

### `BooleanShape` 型の設計

```rust
/// Boolean 演算の結果。
/// `new_faces` は演算ツールの境界面が生んだフェイスの Compound。
/// union の場合は空 Compound が返る。
pub struct BooleanShape {
    pub shape: Shape,
    pub new_faces: Shape,
}

/// Shape が必要な場所に `.into()` で変換できる。
impl From<BooleanShape> for Shape {
    fn from(r: BooleanShape) -> Shape { r.shape }
}
```

両フィールドが `pub` なので、`result.shape` で直接アクセスでき、
Rust の構造体分解も使える。`shape()` / `into_shape()` のような
メソッドは `From` trait と役割が被るため設けない。

```rust
// 直接アクセス
let mesh = result.shape.mesh_with_tolerance(0.1)?;

// 構造体分解（最も明示的）
let BooleanShape { shape: part_neg, new_faces: cut_faces } = shape.intersect(&half)?;

// Shape として使いたいだけのとき
let s: Shape = a.union(&b)?.into();
```

### C++ 側の統一実装

3 演算の C++ 実装を共通の実装パターンに統一する。

```cpp
// union / subtract / intersect でそれぞれの BRepAlgoAPI_XXX を使い、
// Generated を呼ぶかどうかだけ変える。

std::unique_ptr<BooleanShape> boolean_fuse_impl(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Fuse op(a, b);
        op.Build();
        if (!op.IsDone()) return nullptr;
        BRepBuilderAPI_Copy copier(op.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        r->new_faces = make_empty_compound(); // union に Generated は不要
        return r;
    } catch (const Standard_Failure&) { return nullptr; }
}

std::unique_ptr<BooleanShape> boolean_cut_impl(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    // intersect_with_cut_faces と同じパターンで Generated を収集してから deep copy
    ...
}

std::unique_ptr<BooleanShape> boolean_common_impl(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    // intersect_with_cut_faces と同じパターン
    ...
}
```

### Rust 呼び出し側の変化

```rust
// 旧
let part_neg = shape.intersect(&half)?;                         // Shape
let filler = extrude_cut_faces(&part_neg, origin, delta)?;     // heuristic

// 新（BooleanShape 統一後）
let BooleanShape { shape: part_neg, new_faces: cut_faces } = shape.intersect(&half)?;
let filler = extrude_faces(&cut_faces, delta)?;                // 確実

// union は new_faces を無視して使う場合が多い
let result: Shape = a.union(&b)?.into();
// または構造体分解して明示的に
let BooleanShape { shape: result, .. } = a.union(&b)?;
```

### union の `new_faces` が空であることの安全性

空 Compound を渡された `extrude_faces` は 0 回ループして `Shape::empty()` を返す。
それを union しても形状は変わらない。呼び出し側が union の `new_faces` を
誤って stretch フィラーに使ったとしても壊れない。

### メリットとデメリット

| | メリット | デメリット |
|---|---|---|
| `BooleanShape` 統一型 | API が一貫。呼び出し側は常に同じ型を受け取る | 既存の `union/subtract/intersect` が破壊的変更になる |
| `BooleanShape` 統一型 | stretch 以外のユースケースでも Generated が取れる | union の `new_faces` は意味的に空であり、混乱の余地がある |
| 個別拡張（現行 Option C 案） | 既存 API を壊さない | `intersect_with_cut_faces` だけ特別扱い。一貫性に欠ける |

### 推奨方針

すでに `union/subtract/intersect` は `0.2.0` で破壊的変更（`Result<Shape, Error>` 化）
が行われる予定なので、同じタイミングで `Result<BooleanShape, Error>` に変更するのが
コストが最小。`BooleanShape` に `From<BooleanShape> for Shape` を実装すれば
既存の `.into()` パターンで簡単に移行できる。
