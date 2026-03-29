# FFI 例外安全性の強化と stretch_box テスト修正 (2026-03-03)

`stretch_box` テストがランダムパラメータ探索の途中で
`STATUS_STACK_BUFFER_OVERRUN` (0xc0000409) によりプロセスごと落ちていた問題の
原因調査と対処をまとめる。

---

## 1. `panic::catch_unwind` は C++ 例外を捕捉できない

### 問題

```
terminate called after throwing an instance of 'StdFail_NotDone'
error: test failed (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)
```

テスト側で `panic::catch_unwind(AssertUnwindSafe(|| ...))` を使っていたが、
C++ 例外が FFI 境界を越えてプロセスを強制終了した。

### 原因

`panic::catch_unwind` は **Rust パニック** のみを補足する。
OCCT の `StdFail_NotDone` は `Standard_Failure` のサブクラスであり、
C++ 例外として送出される。C++ 例外が cxx の FFI 境界を越えると UB となり、
Windows は `STATUS_STACK_BUFFER_OVERRUN` でプロセスを即時終了する
(`20260302-TIPS_jp.md` §2 参照)。

### 対処の方針

C++ 例外は **C++ 側の wrapper 関数内で必ず捕捉し、Rust 側には届かせない**。
テスト側のコードは変更不要。

---

## 2. `GeomAPI_ProjectPointOnSurf::LowerDistanceParameters` が投げる `StdFail_NotDone`

### 問題箇所

`face_normal_at_center` (`cpp/wrapper.cpp`) の中で、
面の重心を曲面へ投影し、投影点の UV パラメータを取得していた。

```cpp
GeomAPI_ProjectPointOnSurf projector(center, surface);
double u, v;
projector.LowerDistanceParameters(u, v);  // ← NbPoints()==0 のとき例外
```

`projector.NbPoints() == 0`（投影点が見つからない）のときに
`LowerDistanceParameters` が `StdFail_NotDone` を送出する。
これがランダムパラメータ探索中に発生した直接の原因。

### 対処

投影点の有無を確認してから呼び出すよう変更し、さらに全体を try/catch で包む。

```cpp
void face_normal_at_center(const TopoDS_Face& face,
    double& nx, double& ny, double& nz)
{
    try {
        GProp_GProps props;
        BRepGProp::SurfaceProperties(face, props);
        gp_Pnt center = props.CentreOfMass();

        Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
        GeomAPI_ProjectPointOnSurf projector(center, surface);

        if (projector.NbPoints() == 0) {   // ← ガード追加
            nx = ny = nz = 0.0;
            return;
        }
        double u, v;
        projector.LowerDistanceParameters(u, v);

        BRepGProp_Face gprop_face(face);
        gp_Pnt point;
        gp_Vec normal;
        gprop_face.Normal(u, v, point, normal);
        if (normal.Magnitude() > 1e-10) normal.Normalize();
        nx = normal.X(); ny = normal.Y(); nz = normal.Z();
    } catch (const Standard_Failure&) {
        nx = ny = nz = 0.0;
    }
}
```

---

## 3. 全 FFI wrapper 関数への try/catch 追加

既存の I/O 関数 (`BinTools`, `BRepTools`) には既に try/catch が付いていたが、
形状演算・ジオメトリ系の関数には付いていなかった。今回すべてに追加した。

| 関数 | 例外時の戻り値 |
|---|---|
| `boolean_fuse` | `make_empty()` |
| `boolean_cut` | `make_empty()` |
| `boolean_common` | `make_empty()` |
| `clean_shape` | `make_empty()` |
| `face_extrude` | `make_empty()` |
| `face_center_of_mass` | `cx = cy = cz = 0.0` |
| `face_normal_at_center` | `nx = ny = nz = 0.0` |
| `edge_approximation_segments_ex` | `count = 0`, `coords` 空 |

各ブーリアン演算の例：

```cpp
std::unique_ptr<TopoDS_Shape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Fuse fuse(a, b);
        fuse.Build();
        if (!fuse.IsDone()) return make_empty();
        BRepBuilderAPI_Copy copier(fuse.Shape(), Standard_True, Standard_False);
        return std::make_unique<TopoDS_Shape>(copier.Shape());
    } catch (const Standard_Failure&) {
        return make_empty();
    }
}
```

---

## 4. `stretch_box` テストの目的と設計

`tests/stretch_box.rs` の `stretch_box_random_survey` テストは、ランダムな中心点で
引き延ばし処理を大量実行し、エラーが生じるパラメータを探索して CSV に書き出す。

- シード固定の LCG 疑似乱数で再現性を確保
- 1000 点 × 3 軸 = 3000 試行
- 各試行の `(cx, cy, cz, dx, dy, dz, success, error_msg)` を `out/stretch_box_random_survey.csv` へ記録
- テスト自体はエラーがあっても継続し、最後まで実行されることを保証する

上記の C++ 修正により、今まで例外でプロセスが落ちていた試行が
`Err` として正常に記録されるようになった（今回は 3000/3000 成功）。

---

## 5. stretch アルゴリズムと `face_normal_at_center` の役割

### 引き延ばしのプロセス

`stretch_vector(shape, origin, delta)` は次の 3 ステップで形状を引き延ばす。

```
元の形状
      ┌─────────────┐
      │             │
      │    shape    │
      │             │
      └─────────────┘

① 切断平面（origin を通り、delta 方向に法線を持つ）で二分割
      ┌──────┬──────┐
      │      │      │
      │neg側 │ pos側│
      │      │      │
      └──────┴──────┘
         ↑ cutting plane (origin, delta.normalize())

② pos 側を delta だけ平行移動し、neg 側との間に隙間を作る
      ┌──────┐      ┌──────┐
      │      │ gap  │      │
      │ neg  │←→   │ pos  │ (translated)
      │      │      │      │
      └──────┘      └──────┘

③ 切断面に生じた断面（cut faces）を delta 方向に押し出して隙間を埋める
      ┌──────┬──────┬──────┐
      │      │filler│      │
      │ neg  │extrude pos  │
      │      │      │      │
      └──────┴──────┴──────┘
```

コードの対応：

```rust
let part_neg = shape.intersect(&half);              // ① neg 側
let part_pos = shape.subtract(&half).translated(delta); // ① + ② pos 側を移動
let filler   = extrude_cut_faces(&part_neg, origin, delta); // ③ 断面を押し出し
part_neg.union(&filler).union(&part_pos)            // 三者を合体
```

### 断面フェイスをどう特定するか

`extrude_cut_faces` は `part_neg` の全フェイスを走査して断面を探す。
断面フェイスの条件は 2 つ：

```rust
// 条件 1: フェイスの法線が引き延ばし方向とほぼ平行（閾値 0.99）
normal.dot(plane_normal).abs() > NORMAL_THRESHOLD
// 条件 2: フェイスの重心が切断平面上（許容誤差 0.5 mm 以内）
(center - origin).dot(plane_normal).abs() < COORD_TOLERANCE
```

条件 1 の評価に `face_normal_at_center()` が必要になる。
切断によって生じた断面は、切断平面（delta 方向に法線を持つ）と
向き合った面になるため、その法線は delta 方向と平行になる。
これ以外のフェイス（元の形状の側面・底面など）は法線方向が異なり除外される。

### なぜ曲面への投影が必要か

OCCT のサーフェスは**パラメトリック表現** `S(u, v) → (x, y, z)` を持ち、
法線も UV 座標の関数 `N(u, v) → (nx, ny, nz)` として定義される。
`BRepGProp_Face::Normal(u, v, point, normal)` は UV 座標を引数に取るため、
直接 3D 点を渡して法線を得る API は存在しない。

一方、`BRepGProp::SurfaceProperties` が返す重心は**3D 直交座標**の点である。
重心は面積で重み付けした積分の結果であり、曲面の場合は面上に乗らないこともある
（凸面の重心がサーフェス外に浮く場合など）。

```
曲面の例:
           重心（3D 点）
              ×  ← サーフェス外に浮いている
         ．．×．．  ← サーフェス
       ．        ．
      ．          ．
```

そこで `GeomAPI_ProjectPointOnSurf` を使い、
重心の 3D 点をサーフェス上の最近傍点へ投影して UV 座標を得る。

```
① BRepGProp → 重心 center (x, y, z) を計算
② GeomAPI_ProjectPointOnSurf → center を曲面上に投影、(u, v) を取得
③ BRepGProp_Face::Normal(u, v, ...) → (u, v) における法線を評価
```

平面フェイスでは重心は必ずサーフェス上にあるため投影は自明だが、
ブーリアン演算後に生じる曲面フェイスや自由曲面に対しても
同一コードで正しく動作するよう、常に投影を経由している。

### 投影が失敗するケース（`NbPoints() == 0`）

ブーリアン演算の結果として生じる**極めて細い・退化したフェイス**では、
数値的に投影点が見つからない場合がある。
このとき `LowerDistanceParameters` が `StdFail_NotDone` を投げる。
修正後は `NbPoints() == 0` を事前チェックし、法線を `(0, 0, 0)` として返す。
法線ゼロのフェイスは `normal.dot(plane_normal).abs() > 0.99` を満たさないため、
断面フェイスとして誤選択される心配はない。
