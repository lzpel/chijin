# `Result<Shape, Error>` 移行計画 (2026-03-03)

## 経緯

### 問題の発端

`stretch_box` テストがランダムパラメータ探索の途中でプロセスごと落ちていた。
原因は OCCT の C++ 例外 `StdFail_NotDone` が FFI 境界を越えたことによる
`STATUS_STACK_BUFFER_OVERRUN`（詳細は `20260303-ffi-exception-safety_jp.md` §1）。

### 最初の対処とその問題

C++ ラッパー関数が例外を送出した際に `make_empty()`（空の `TopoDS_Compound`）を
返すよう修正した。これでクラッシュは止まったが、新たな問題が生じた。

`Shape::is_null()` は `TopoDS_Shape::IsNull()` を呼ぶ。
`make_empty()` が返す空 Compound は `IsNull() == false` であるため、
`stretch_ok` の失敗検出コードをすり抜け、**本来 `success=0` と記録されるべき試行が
`success=1` と記録される**。

```
C++ 例外発生
  └─ boolean_fuse が make_empty() を返す（IsNull == false）
       └─ stretch_ok で s.is_null() == false
            └─ Ok(s) として記録 → CSV に success=1 ← 誤り
```

### 代替案の検討

C++ が `nullptr` を返し、`Shape::is_null()` で UniquePtr レベルの null も
検出する方針も検討した。しかしこの方針には欠陥がある。

null な `UniquePtr<TopoDS_Shape>` を保持する `Shape` が中間状態として存在すると、
それを別の操作（`union`, `subtract` など）の引数に渡した時点で
null 参照を C++ に渡すことになり UB（クラッシュ）が発生する。

```rust
let s = shape.union(&other);    // s.inner が null UniquePtr
let t = s.union(&yet_another);  // &s.inner を C++ に渡す → null 参照 → UB
```

これを防ぐには全操作箇所で null チェックを書く必要があり、
結局どちらの方針も「失敗を型の外に隠す」本質的な問題は解決しない。

### 根本解決：`Result<Shape, Error>` への移行

失敗しうる操作の戻り値型を `Result<Shape, Error>` に変更する。
これにより失敗が型として伝播し、呼び出し側は `?` 演算子で連鎖でき、
CSV 記録まで確実にエラーが届く。

---

## 変更方針

### 1. `src/error.rs` — エラー種別を追加

OCCT の形状演算失敗を表すバリアントを追加する。

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // 既存
    #[error("STEP read failed")]    StepReadFailed,
    #[error("BRep read failed")]    BrepReadFailed,
    #[error("STEP write failed")]   StepWriteFailed,
    #[error("BRep write failed")]   BrepWriteFailed,
    #[error("Triangulation failed")] TriangulationFailed,

    // 追加
    /// Boolean operation (fuse/cut/common) failed in OCCT.
    #[error("Boolean operation failed")]
    BooleanOperationFailed,

    /// Shape cleaning (UnifySameDomain) failed in OCCT.
    #[error("Shape clean failed")]
    CleanFailed,

    /// Face extrusion (MakePrism) failed in OCCT.
    #[error("Extrude failed")]
    ExtrudeFailed,
}
```

### 2. `cpp/wrapper.cpp` — 例外時に `nullptr` を返す

`make_empty()` の代わりに `nullptr` を返すことで、
Rust 側が `UniquePtr::is_null()` で失敗を検出できるようにする。

対象関数と、例外時の戻り値：

| 関数 | 変更前 | 変更後 |
|---|---|---|
| `boolean_fuse` | `make_empty()` | `nullptr` |
| `boolean_cut` | `make_empty()` | `nullptr` |
| `boolean_common` | `make_empty()` | `nullptr` |
| `clean_shape` | `make_empty()` | `nullptr` |
| `face_extrude` | `make_empty()` | `nullptr` |

void 関数（`face_center_of_mass`, `face_normal_at_center`）はゼロベクトル返しを維持する。
これらが返す値は stretch の断面検出フィルタ（`dot > 0.99`）を通過しないため、
安全な縮退として扱える。

### 3. `src/shape.rs` — 戻り値型の変更

#### 変更する関数（失敗しうる形状演算）

```rust
// 変更前
pub fn union(&self, other: &Shape) -> Shape
pub fn subtract(&self, other: &Shape) -> Shape
pub fn intersect(&self, other: &Shape) -> Shape
pub fn clean(&self) -> Shape

// 変更後
pub fn union(&self, other: &Shape) -> Result<Shape, Error>
pub fn subtract(&self, other: &Shape) -> Result<Shape, Error>
pub fn intersect(&self, other: &Shape) -> Result<Shape, Error>
pub fn clean(&self) -> Result<Shape, Error>
```

null UniquePtr が返ってきた場合に対応するエラーを返す：

```rust
pub fn union(&self, other: &Shape) -> Result<Shape, Error> {
    let inner = ffi::boolean_fuse(&self.inner, &other.inner);
    if inner.is_null() {
        return Err(Error::BooleanOperationFailed);
    }
    Ok(Shape { inner })
}
```

#### 変更しない関数（失敗しないコンストラクタ・変換）

`half_space`, `box_from_corners`, `cylinder`, `empty`, `deep_copy`, `translated`
はいずれも例外を送出しない操作なので変更しない。

### 4. `src/face.rs` — `extrude` の戻り値型の変更

```rust
// 変更前
pub fn extrude(&self, dir: DVec3) -> Solid

// 変更後
pub fn extrude(&self, dir: DVec3) -> Result<Solid, Error>
```

`face_extrude` が nullptr を返した場合に `Err(Error::ExtrudeFailed)` を返す。

### 5. `tests/stretch_box.rs` — 呼び出し側の更新

`stretch_vector` と `extrude_cut_faces` が `Result` を返すよう変更し、
`?` で連鎖する。

```rust
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
    let half = Shape::half_space(origin, delta.normalize());
    let part_neg = shape.intersect(&half)?;
    let part_pos = shape.subtract(&half)?.translated(delta);
    let filler = extrude_cut_faces(&part_neg, origin, delta)?;
    part_neg.union(&filler)?.union(&part_pos)
}
```

`stretch_ok` は `panic::catch_unwind` が不要になる。
C++ 例外は wrapper 内で捕捉されて `nullptr` → `Err` に変換されるため、
Rust パニックは発生しない。

```rust
// 変更後の stretch_ok
pub fn stretch_ok(...) -> Result<Shape, Error> {
    stretch(shape, cx, cy, cz, dx, dy, dz)
}
```

CSV 記録側も `Error` を `Display` で文字列化できる：

```rust
Err(e) => {
    writeln!(file, "...,0,{}", e).unwrap();
}
```

### 6. `tests/integration.rs` — 既存テストの更新

`union`, `subtract`, `intersect`, `clean` の呼び出し箇所に `?` または
`.unwrap()` を追加する。テスト内で失敗が予期されない場合は `.unwrap()` で問題ない。

---

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `src/error.rs` | `BooleanOperationFailed`, `CleanFailed`, `ExtrudeFailed` 追加 |
| `cpp/wrapper.cpp` | 5 関数の例外 fallback を `make_empty()` → `nullptr` に変更 |
| `src/shape.rs` | `union/subtract/intersect/clean` の戻り値を `Result<Shape, Error>` に変更 |
| `src/face.rs` | `extrude` の戻り値を `Result<Solid, Error>` に変更 |
| `tests/stretch_box.rs` | `stretch_vector`, `extrude_cut_faces`, `stretch` を `Result` 対応に変更、`stretch_ok` を簡略化 |
| `tests/integration.rs` | 変更した API 呼び出し箇所に `?` / `.unwrap()` を追加 |

---

## 破壊的変更について

`union`, `subtract`, `intersect`, `clean`, `Face::extrude` はすべて公開 API である。
戻り値型の変更は **破壊的変更（semver breaking change）** にあたる。
実装完了後はバージョンを `0.1.x` → `0.2.0` に上げる。
