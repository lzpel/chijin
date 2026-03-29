# BRep+Color 独自バイナリ形式 設計メモ (2026-03-04)

`--features color` 時に使える `read_brep_color` / `write_brep_color` の設計。

---

## 1. 背景と方針

OCCT 標準の BRep フォーマット（`BRepTools` / `BRep_Builder`）は純粋な
ジオメトリ・トポロジーのシリアライズ形式であり、色属性を持つ仕様ではない。

色を BRep と一緒に保存する方法は大きく 2 つある：

| 案 | 概要 | コスト |
|---|---|---|
| XCAF ドキュメント形式 | `TDocStd_Document` を `BinXCAFDrivers` でシリアライズ | 追加 OCC ライブラリ（TKBinXCAF 等）が必要、C++ 実装が必要 |
| **独自バイナリ形式（採用）** | 既存の BRep バイナリ出力 + colormap セクションを 1 ファイルに詰める | 新規 C++ コード不要、Pure Rust で実装可能 |

---

## 2. face の同一性問題

colormap のキーは `TShapeId`（`TopoDS_TShape*` のアドレス値）だが、
BRep をいったんファイルに書いて読み直すと、同じジオメトリでも
新しい `TShape` オブジェクトが割り当てられるためアドレスが変わる。

**解決策：face index（巡回順序）を安定識別子として使う。**

BRep バイナリ形式は `TopExp_Explorer(shape, TopAbs_FACE)` の巡回順序を保持する。
これは `remap_colormap_by_order`（translate/deep_copy で使用）と同じ前提。

- **書き込み時**：face を巡回し、colormap に含まれる face の
  `(0-based index, r, g, b)` を記録する
- **読み込み時**：BRep を復元後、face を同じ順で巡回し、
  `(index → TShapeId)` テーブルを作成してから colormap を再構築する

---

## 3. バイナリフォーマット仕様

GLB などと同様に、メタデータ（Color）を先に、バルクデータ（BRep）を後に置く。

```
┌─────────────────────────────────────────────────────────┐
│ magic   : [u8; 4]  = b"CHJC"                            │
│ version : u8       = 1                                   │
├─────────────────────────────────────────────────────────┤
│ Color セクション                                          │
│   color_count : u32 (little-endian)                     │
│   エントリ × color_count:                               │
│     face_index : u32 (little-endian)  ← 0-based        │
│     r          : f32 (little-endian)                    │
│     g          : f32 (little-endian)                    │
│     b          : f32 (little-endian)                    │
├─────────────────────────────────────────────────────────┤
│ BRep セクション                                           │
│   brep_len  : u64 (little-endian)                       │
│   brep_data : [u8; brep_len]  ← 既存 BRep binary 形式   │
└─────────────────────────────────────────────────────────┘
```

多バイト整数・浮動小数点はすべてリトルエンディアン。
BRep セクションと Color セクションの間にパディングなし。

---

## 4. 書き込みアルゴリズム（`write_brep_color`）

```
1. BRep バイナリを Vec<u8> に書き出す（既存 write_brep_bin 利用）
2. face を TopExp_Explorer で巡回し TShapeId → index の逆引きマップを構築
3. colormap の各エントリを逆引きマップで face_index に変換
   （colormap にあるが shape に存在しない TShapeId は無視）
4. ヘッダー（magic + version）を書く
5. color_count (u32 LE) を書く
6. (face_index u32 LE, r f32 LE, g f32 LE, b f32 LE) × color_count を書く
7. brep_len (u64 LE) + brep_data を書く
```

---

## 5. 読み込みアルゴリズム（`read_brep_color`）

```
1. magic を読んで b"CHJC" であることを確認（違えば Err）
2. version を読んで 1 であることを確認（違えば Err）
3. color_count (u32 LE) を読む
4. (face_index u32 LE, r f32 LE, g f32 LE, b f32 LE) × color_count を読む
5. brep_len (u64 LE) を読む
6. brep_data を brep_len バイト読んで read_brep_bin_stream に渡し inner を得る
7. face を TopExp_Explorer で巡回し index → TShapeId テーブルを構築
8. 各エントリの face_index で TShapeId を引き、colormap を構築
   （face_index が範囲外の場合は無視）
9. Shape { inner, colormap } を返す
```

---

## 6. Rust 実装イメージ

```rust
// shape.rs
#[cfg(feature = "color")]
pub fn write_brep_color(&self, writer: &mut impl Write) -> Result<(), Error> {
    // ① BRep を一時バッファに書き出す
    let mut brep_buf = Vec::new();
    self.write_brep_bin(&mut brep_buf)?;

    // ② TShapeId → face_index の逆引きマップ
    let id_to_index: std::collections::HashMap<TShapeId, u32> =
        FaceIterator::new(ffi::explore_faces(&self.inner))
            .enumerate()
            .map(|(i, f)| (f.tshape_id(), i as u32))
            .collect();

    // ③ colormap を (face_index, r, g, b) エントリに変換
    let mut entries: Vec<(u32, f32, f32, f32)> = self.colormap
        .iter()
        .filter_map(|(id, rgb)| {
            id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))
        })
        .collect();
    entries.sort_by_key(|e| e.0); // 決定論的な出力順のためソート

    // ④ 書き出し
    writer.write_all(b"CHJC")?;          // magic
    writer.write_all(&[1u8])?;           // version
    writer.write_all(&(entries.len() as u32).to_le_bytes())?;
    for (idx, r, g, b) in &entries {
        writer.write_all(&idx.to_le_bytes())?;
        writer.write_all(&r.to_le_bytes())?;
        writer.write_all(&g.to_le_bytes())?;
        writer.write_all(&b.to_le_bytes())?;
    }
    writer.write_all(&(brep_buf.len() as u64).to_le_bytes())?;
    writer.write_all(&brep_buf)?;
    Ok(())
}

#[cfg(feature = "color")]
pub fn read_brep_color(reader: &mut impl Read) -> Result<Shape, Error> {
    // ① magic + version
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|_| Error::BrepReadFailed)?;
    if &magic != b"CHJC" { return Err(Error::BrepReadFailed); }
    let mut ver = [0u8; 1];
    reader.read_exact(&mut ver).map_err(|_| Error::BrepReadFailed)?;
    if ver[0] != 1 { return Err(Error::BrepReadFailed); }

    // ② Color エントリ読み込み
    let mut count_buf = [0u8; 4];
    reader.read_exact(&mut count_buf).map_err(|_| Error::BrepReadFailed)?;
    let color_count = u32::from_le_bytes(count_buf) as usize;
    let mut entries = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        let mut entry = [0u8; 16]; // u32 + f32 * 3
        reader.read_exact(&mut entry).map_err(|_| Error::BrepReadFailed)?;
        let idx = u32::from_le_bytes(entry[0..4].try_into().unwrap());
        let r   = f32::from_le_bytes(entry[4..8].try_into().unwrap());
        let g   = f32::from_le_bytes(entry[8..12].try_into().unwrap());
        let b   = f32::from_le_bytes(entry[12..16].try_into().unwrap());
        entries.push((idx, r, g, b));
    }

    // ③ BRep 読み込み
    let mut len_buf = [0u8; 8];
    reader.read_exact(&mut len_buf).map_err(|_| Error::BrepReadFailed)?;
    let brep_len = u64::from_le_bytes(len_buf) as usize;
    let mut brep_buf = vec![0u8; brep_len];
    reader.read_exact(&mut brep_buf).map_err(|_| Error::BrepReadFailed)?;
    let mut rust_reader = RustReader::from_ref(&mut brep_buf.as_slice());
    let inner = ffi::read_brep_bin_stream(&mut rust_reader);
    if inner.is_null() { return Err(Error::BrepReadFailed); }

    // ④ face index → TShapeId テーブル構築
    let index_to_id: Vec<TShapeId> =
        FaceIterator::new(ffi::explore_faces(&inner))
            .map(|f| f.tshape_id())
            .collect();

    // ⑤ colormap 構築
    let colormap = entries.into_iter()
        .filter_map(|(idx, r, g, b)| {
            index_to_id.get(idx as usize).map(|&id| (id, Rgb { r, g, b }))
        })
        .collect();

    Ok(Shape { inner, colormap })
}
```

---

## 7. エラーハンドリング

既存の `Error` 型で対応可能：

| ケース | 使用するエラー |
|-------|-------------|
| magic 不一致 | `Error::BrepReadFailed` |
| version 不一致 | `Error::BrepReadFailed` |
| BRep パース失敗 | `Error::BrepReadFailed` |
| I/O エラー（read_exact 失敗）| `Error::BrepReadFailed` |
| BRep 書き込み失敗 | `Error::BrepWriteFailed` |
| I/O エラー（write_all 失敗）| `Error::BrepWriteFailed` |

---

## 8. 新規 C++ コードは不要

- `write_brep_bin` / `read_brep_bin`（既存）を内部で使う
- colormap の変換・シリアライズは Pure Rust
- `FaceIterator`（既存）と `face_tshape_id`（`--features color` 時の既存 FFI）を使う

---

## 9. テスト方針

`tests/integration_color_brep.rs`（新規）：

| テスト名 | 検証内容 |
|---------|---------|
| `write_then_read_preserves_colors` | 書いて読み直すと colormap が同一 |
| `roundtrip_after_boolean` | Boolean 演算後のシェイプも正しく往復できる |
| `colorless_shape_roundtrip` | colormap が空のシェイプも正しく往復できる |
| `invalid_magic_returns_error` | magic 不正なデータで `BrepReadFailed` が返る |
