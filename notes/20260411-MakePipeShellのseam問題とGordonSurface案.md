# MakePipeShell の seam 問題と Gordon surface 案

## MakePipeShell の seam 問題

`tests/sweep_sections.rs` で8断面ステラレーター風ソリッドを sweep_sections で生成し、
ZX/YZ 平面で4象限に分割して点対称（q1≈q3, q2≈q4）を検証したところ ~1.2% の誤差:

```
q1=54.5546  q2=34.9157  q3=53.9812  q4=35.3595
q1-q3 rel_err=0.0106, q2-q4 rel_err=0.0126
```

数学的には `sin(2φ)` / `cos(2φ)` のアスペクト比変動は完全に点対称だが、
closed spine の seam vertex（パラメータ 0 = 2π の点）付近で法線の不連続が生じている。

## 試した対策と結果

| 対策 | 効果 |
|---|---|
| `SetForceApproxC1(true)` | なし（数値完全一致） |
| 最初の profile を末尾に再追加 (IsSame trick) | なし（数値完全一致） |
| 上記の組み合わせ | なし（数値完全一致） |
| `GeomFill_NSections` + `GeomFill_Sweep` | surface 生成は成功するが、spine に沿った配置が二重適用になり対称性は改善しない |
| `GeomFill_NSections::BSplineSurface()` 直接生成 | spine を使わない補間なので ThruSections と本質的に同じ問題 |

### 根本原因

MakePipeShell は spine のパラメトリック空間で連続的にモーフィング補間する。
closed spine の seam vertex でパラメタリゼーションに不連続が生じ、これが非対称性の原因。
SetForceApproxC1 は曲面近似の問題のみ対象で、セクション配置のパラメタリゼーション不連続には無関係。
IsSame trick は ThruSections の離散的 wire 列で機能する仕組みで、MakePipeShell には効かない。

## OCCT 8.0.0 の Gordon surface

OCCT 8.0.0 (現在 RC5) で `GeomFill_GordonBuilder` / `GeomFill_Gordon` が追加された。

- **transfinite 補間** (Boolean sum formula): `S = S_profiles + S_guides - S_tensor`
- **closed (periodic) curve network を C2 連続で補間**と明記されている
- spine のパラメタリゼーションに依存しないので seam 問題が根本的に解決される

### 入力

- **Profiles**: 横断面曲線群（= 現在の sections そのまま）
- **Guides**: 縦断面曲線群（各セクションの対応点を繋ぐ曲線）

### sweep_sections API との互換性

Guides は sections から自動生成できる:
1. 各セクション曲線を同一パラメータ値 `t_j` でサンプリング
2. `t_j` ごとに全セクションの対応点を spine 方向に繋いだ B-spline を生成

これにより `sweep_sections(sections, spine, orient)` の外部シグネチャを変えずに
内部実装を MakePipeShell → Gordon surface に差し替え可能。
**ProfileOrient は実質無視される**（セクションの3D位置がすべてを決める）が、
現在の使い方（呼び出し側が sections を3D配置してから渡す）であれば問題ない。

### バージョン要件

| | OCCT バージョン |
|---|---|
| cadrum 現在 | 7.9.3 |
| CadQuery (OCP) | 7.9.3.1 |
| Gordon surface | 8.0.0 (RC5) |

OCCT 8.0.0 が安定リリースされたら build.rs のバージョンを上げて導入する。
cadrum がリンクする TKGeomAlgo に Gordon のコードが含まれる見込みで、
バイナリサイズへの影響は限定的（static link でデッドコード除去が効く）。
