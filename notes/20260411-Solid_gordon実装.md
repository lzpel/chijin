# Solid::gordon 実装

## 背景

ステラレーター平衡面の CAD 化において、既存の 2 手法には以下の問題がある:

- **Solid::loft (ThruSections)**: 断面間の B-spline 補間が暗黙の spine を生成するため、回転対称性が壊れる (notes/20260410-loft閉じ方とparastell比較.md 参照)
- **Solid::sweep (MakePipeShell)**: closed spine の seam vertex (パラメータ 0 = 2π) 付近で法線の不連続が生じ、~1.2% の非対称が出る (notes/20260411-MakePipeShellのseam問題とGordonSurface案.md 参照)

OCCT 8.0.0 で追加された `GeomFill_Gordon` は Boolean sum formula `S = S_profiles + S_guides - S_tensor` による transfinite 補間で、spine パラメタリゼーションに依存しないため seam 問題を根本的に回避できる。

## 設計判断

### API シグネチャ

```rust
fn gordon<'a, 'b, P, G, PI, GI>(profiles: P, guides: G) -> Result<Self, Error>
where
    P: IntoIterator<Item = PI>,
    PI: IntoIterator<Item = &'a Self::Edge>,
    G: IntoIterator<Item = GI>,
    GI: IntoIterator<Item = &'b Self::Edge>,
    Self::Edge: 'a + 'b;
```

- loft / sweep と同じ edge collection パターンに統一
- profiles = 断面曲線群 (V方向)、guides = ガイド曲線群 (U方向)
- tolerance は `Precision::Confusion()` 固定 (loft と同じ方針: ユーザーに露出しない)

### surface → solid の変換

Gordon surface は `Geom_BSplineSurface` を生成するだけで、直接 solid にはならない。以下の手順で solid 化する:

1. `BRepBuilderAPI_MakeFace(surface)` で Gordon face を作成
2. 先頭/末尾の profile wire から `BRepBuilderAPI_MakeFace(wire, onlyPlane=true)` でキャップ face を作成
3. `BRepBuilderAPI_Sewing` で全 face を縫合 → shell
4. `BRepBuilderAPI_MakeSolid(shell)` → solid

閉じた curve network (IsUClosed && IsVClosed) の場合はキャップ不要で、surface 自体が closed shell になる。

### edge → curve 変換

GeomFill_Gordon は `Geom_Curve` を受け取るが、cadrum の API は `Edge` (TopoDS_Edge) ベース。C++ wrapper 内で:

- 単一 edge → `BRep_Tool::Curve()` で直接抽出
- 複数 edge (wire) → `GeomConvert_CompCurveToBSplineCurve` で単一 BSpline に結合

### スレッド安全性

GeomFill は ThruSections と同じ TKGeomAlgo に属するため、安全側で既存の `LOFT_LOCK` Mutex を共有。

## 能力と制限

### 使えるもの

- **非滑らか (C⁰) な profile**: polygon 断面 (星形等) の角は BSpline の knot multiplicity で保存される。ただし断面間はスムーズに補間されるため、「断面位置では尖っているが断面間で稜線が維持される」とは限らない
- **非滑らか (C⁰) な guide**: 同様に角が保存される
- **非直交の交差**: `computeIntersections()` は `GeomAPI_ExtremaCurveCurve` で汎用探索するため交差角度に制約なし
- **閉じた curve network**: `detectClosedness()` で自動判定、C² 連続な周期補間

### 使えないもの

- **rational curve**: `GeomFill_Gordon` は Non-rational curves only。円弧 (`Geom_Circle`) は rational BSpline なので直接は不可。`GeomConvert::CurveToBSplineCurve` での近似変換が必要 (現実装では `BRep_Tool::Curve` が返す curve をそのまま渡しているため、rational curve 入力時は Gordon が失敗する可能性がある)
- **メビウスの帯**: BSpline surface は向き付け可能面のみ表現可能。非向き付け可能面は単一パッチでは不可能。また `detectClosedness()` が幾何学的同一性を要求するため、180° 回転した断面は closed と判定されない

## 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `cpp/wrapper.h` | `make_gordon` 宣言 |
| `cpp/wrapper.cpp` | `make_gordon` 実装 (~130行) |
| `src/occt/ffi.rs` | FFI 宣言 |
| `src/occt/solid.rs` | `gordon()` Rust 実装 |
| `src/traits.rs` | `fn gordon(...)` trait シグネチャ |
| `src/common/error.rs` | `GordonFailed(String)` variant |

## 今後の課題

- **rational curve 対応**: C++ wrapper 内で `GeomConvert::CurveToBSplineCurve` を挟んで non-rational 化する
- **sweep_sections 置き換え**: notes/20260411 で示唆された通り、guides を sections から自動生成することで `sweep_sections` の内部実装を Gordon に差し替え可能
- **対称性テスト**: 4-fold 対称 curve network で象限体積比較を行い、loft/sweep 比で対称性が改善されることを検証
