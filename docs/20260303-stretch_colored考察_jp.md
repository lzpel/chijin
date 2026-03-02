# stretch_colored.rs の実装に向けた考察：STEP カラー保持と面追跡

作成: 2026-03-03

## 1. 目的

ユーザーは STEP ファイルに含まれる面ごとの色情報を、`stretch` のような幾何変換の前後で保持したい。
例として、**6面が異なる色の直方体** を読み込み → stretch → 書き出しし、同じ6色が正しい面に残ることを検証する `examples/stretch_colored.rs` を構想する。

## 2. 現状の問題点

### 2.1 STEP I/O がカラーを捨てている

現在の `read_step_stream` / `write_step_stream` は `STEPControl_Reader` / `STEPControl_Writer` を使用している。
これらは **非 XDE** の読み書き API であり、形状のジオメトリ（`TopoDS_Shape`）のみを返す。STEP ファイルに埋め込まれた色・名前・アセンブリ構造は無視される。

```
現在: STEP → STEPControl_Reader → TopoDS_Shape（色情報なし）
```

### 2.2 ブール演算後の面追跡手段がない

`stretch` は内部で `intersect` / `subtract` / `union` / `extrude` を繰り返す。
ブール演算の結果として生成される `TopoDS_Face` は**新しいトポロジカルエンティティ**であり、元の面と `IsSame()` で比較しても `false` になる。

つまり、「変換前の Face A が変換後のどの Face に対応するか？」を知る手段が現在のバインディングには存在しない。

## 3. 面追跡のアプローチ

### 3.1 方式 A: OCCT ブール演算ヒストリ（推奨）

`BRepAlgoAPI_BooleanOperation` は `Modified()` / `Generated()` メソッドで、入力 Face → 出力 Face のマッピングを提供する。
現バインディングでは既に `collect_generated_faces()` で `Modified()` を使って切断面を収集しているが、**元面→結果面の追跡にも同じ仕組みが使える**。

```
入力面 F_i → boolean_op.Modified(F_i) → 出力面 F_i' のリスト
```

#### メリット
- OCCT が提供する正式な仕組み
- `deep_copy` 前に呼ぶ必要があるが、`collect_generated_faces()` と同じパターン

#### デメリット
- `stretch` は複数のブール演算を連鎖するため、各ステップで追跡を繋ぐ必要がある
- `ShapeUpgrade_UnifySameDomain`（`clean()`）も面を統合するため、追跡チェーンが切れる可能性がある

#### 実装スケッチ

```
// 擬似コード
fn stretch_vector_tracked(shape, origin, delta, color_map) -> (Shape, ColorMap) {
    let half = Shape::half_space(origin, delta.normalize());

    // intersect
    let (part_neg, neg_history) = shape.intersect_with_history(&half);
    // 元面→切断後面のマッピングを neg_history から取得
    // color_map[元面] → color_map[新面] に転写

    // subtract + translate
    let (part_pos, pos_history) = shape.subtract_with_history(&half);
    let part_pos = part_pos.translated(delta);
    // pos_history でも同様に追跡

    // extrude で作った filler 面は新規 → 色の割り当てポリシーが必要
    // union
    let (combined, union_history) = ...;
    // union_history で最終マッピングを更新

    (combined, updated_color_map)
}
```

### 3.2 方式 B: 幾何的ヒューリスティック（簡易版）

面の法線と重心を使って、変換前後の面を「最も近い面」でマッチングする。

```rust
// 変換前: 各面の (法線, 重心) を記録
let before: Vec<(DVec3, DVec3)> = shape.faces()
    .map(|f| (f.normal_at_center(), f.center_of_mass()))
    .collect();

// 変換後: 同様に計算
let after: Vec<(DVec3, DVec3)> = result.faces()
    .map(|f| (f.normal_at_center(), f.center_of_mass()))
    .collect();

// マッチング: 法線が一致し、重心が期待される変位分だけずれている面をペアリング
```

#### メリット
- 現在の API だけで実装可能（追加の FFI 不要）
- `stretch` の変位量が既知なので、期待される重心位置を事前計算できる

#### デメリット
- `clean()` で面が統合されると面数が変わり、1対1対応しない場合がある
- 曲面や複雑な形状では法線・重心だけでは不十分
- ストレッチで**分割された面**（例: 切断面と交差する元の面）は2つに分かれるため、 1対1マッチングでは対応不可

#### 直方体 + ストレッチの場合の適用可能性

直方体の6面は **すべて平面で法線が軸方向** のため、法線だけで一意に同定できる。
ストレッチ後も平面性は保たれるが、切断面に平行な面が分割される場合がある（例: X 方向ストレッチで YZ 平面に平行な面）。

**直方体の場合、面が分割されない条件**: ストレッチの切断面が直方体の面と一致しないこと。
中心で切断する場合、対向する2面が切断面に平行だが、面自体は分割されない（切断面の範囲外にある）。
→ **直方体 + 中心ストレッチの場合、6面は維持され、法線ベースのマッチングで対応可能**。

### 3.3 方式 C: XDE ドキュメント内で色を管理（最も正攻法）

OCCT の XDE（Extended Data Exchange）フレームワークを使い、`XCAFDoc_ColorTool` で面ごとに色をアタッチする方式。

```
必要な追加 OCC クラス:
├── TDocStd_Document        ── XDE ドキュメント
├── XCAFDoc_ShapeTool       ── 形状管理
├── XCAFDoc_ColorTool       ── 色管理
├── STEPCAFControl_Reader   ── XDE 対応 STEP 読み込み（色保持）
└── STEPCAFControl_Writer   ── XDE 対応 STEP 書き出し（色保持）
```

#### メリット
- STEP ファイルの色を読み書きで完全に保持
- OCCT の公式な色管理手段
- 面の追跡も `XCAFDoc_ShapeTool` の内部で管理される

#### デメリット
- **大量の新しい FFI バインディングが必要**（`TDocStd_Document`, `TDF_Label`, `XCAFDoc_*` 等）
- `chijin` の設計思想（最小限の OCC バインディング）に反する可能性
- ブール演算時に XDE ドキュメント側の面-色マッピングを手動で更新する必要がある

## 4. 推奨アプローチ：方式 B（幾何的ヒューリスティック）を例で使用

`examples/stretch_colored.rs` の目的は**概念実証**（色が保持できることのデモ）であるため、最も実装コストの低い方式 B を採用し、将来の API 拡張では方式 A または C を検討する。

### 4.1 実装計画

1. **入力**: 6面異なる色の直方体 STEP ファイル（手動作成またはプログラムで色付き STEP を生成）
2. **色マップ構築**: 変換前に各面の法線方向で面を特定し `HashMap<FaceId, Color>` を構築
3. **ストレッチ実行**: 既存の `stretch` 関数で直方体を伸縮
4. **色マップ再適用**: 変換後の各面の法線方向で元の色を再割り当て
5. **出力**: 色付きメッシュ（glTF/OBJ）、またはコンソールに色マッチング結果を出力

### 4.2 直方体 6 面の法線マッピング

```
法線方向     色（例）      面の意味
(+1, 0, 0)  赤 (Red)     +X 面
(-1, 0, 0)  緑 (Green)   −X 面
(0, +1, 0)  青 (Blue)    +Y 面
(0, -1, 0)  黄 (Yellow)  −Y 面
(0, 0, +1)  白 (White)   +Z 面
(0, 0, -1)  紫 (Purple)  −Z 面
```

直方体を中心でストレッチした場合、各面の法線方向は変わらないため、この方式で色を完全に追跡できる。

### 4.3 限界と注意

- **面分割**: ストレッチで面が分割される場合（切断面が元の面を横切る場合）、分割後の複数の面が同じ法線を持つため対応可能だが、`clean()` 後に面が再統合されると追跡が複雑化する
- **曲面**: シリンダーなど曲面を持つ形状では、法線が連続的に変化するため法線ベースのマッチングは困難
- **回転を含む変換**: 法線方向が変わるため、変換行列を法線に適用する必要がある

## 5. 将来の API 拡張に向けた提案

### 5.1 短期（方式 A の部分実装）

`BooleanShape` に面の追跡情報を追加する:

```rust
pub struct BooleanShape {
    pub shape: Shape,
    pub new_faces: Shape,
    pub face_history: FaceHistory,  // 新規追加
}

pub struct FaceHistory {
    // input_face → output_faces のマッピング
    mappings: Vec<(Face, Vec<Face>)>,
}
```

### 5.2 中期（XDE 読み書き対応）

`STEPCAFControl_Reader` / `STEPCAFControl_Writer` を使った色付き STEP の読み書き:

```rust
// 色情報を含むドキュメント型
pub struct ColoredShape {
    pub shape: Shape,
    pub face_colors: HashMap<FaceId, Color>,
}

impl ColoredShape {
    pub fn read_step_colored(reader: impl Read) -> Result<ColoredShape, Error>;
    pub fn write_step_colored(&self, writer: impl Write) -> Result<(), Error>;
}
```

### 5.3 長期

面のユーザー定義属性（色に限らず、材質やテクスチャ座標など）を管理する汎用的なフレームワーク。

## 6. まとめ

| 方式 | 実装コスト | 精度 | 汎用性 | 推奨用途 |
|------|----------|------|--------|---------|
| A: ブール演算ヒストリ | 中 | 高 | 高 | API 拡張 |
| B: 幾何的ヒューリスティック | 低 | 中 | 低 | 例の実装 |
| C: XDE フレームワーク | 高 | 最高 | 最高 | 本格的な色管理 |

`examples/stretch_colored.rs` では方式 B により、直方体の6面の法線方向で色をマッチングする方式が最も適切。
ただし、本格的なユースケースには方式 A（ブール演算ヒストリ）や方式 C（XDE）を API レベルでサポートする必要がある。
