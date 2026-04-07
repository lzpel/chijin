# BRep末尾カラートレイラー方式

## 概要

CHJC独自フォーマットを廃止し、BRep（テキスト/バイナリ）の末尾にカラー情報を追記する方式に移行する。

## トレイラー形式

```
[BRep本体 (text or binary)]
[entry_count: u32 LE]
[N x (face_index: u32, r: f32, g: f32, b: f32) LE]   = N x 16 bytes
[magic: "CDCL"]
```

- 読み込み: Rust側で全バッファリング → 末尾4バイトが"CDCL"か判定 → colormap構築 → BRep部分だけC++に渡す
- 書き出し: BRep書き出し後にトレイラー追記

## 後方互換性

- `color`なしビルドでトレイラー付きファイルを読む → OCCTリーダーが末尾を無視 → 正常動作（色を失うだけ）
- `color`ありビルドでトレイラーなしファイルを読む → 末尾がCDCLでない → 素のBRepとして読む → 正常動作

## 棚上げ事項: ストリーム位置によるトレイラー回避

全バッファリングを避け、OCCTがBRepを読み終えたストリーム位置から直接色データを読む方式を検討したが、以下の理由で棚上げ:

- **バイナリ形式**: `BinTools::Read`がseekable streamを要求するため、C++ラッパーが全データをEOFまで読んで`istringstream`に入れている。`istringstream::tellg()`でBRep消費バイト数は取得可能だが、C++ FFIに返り値を追加する必要がある。
- **テキスト形式**: `RustReadStreambuf`が8KBバッファで先読みするため、`BRepTools::Read`停止後のRustリーダー位置がBRepデータ末尾と一致しない。streambufの未消費バッファ量を把握してRust側に返す仕組みが必要。

将来的にパフォーマンスが問題になった場合、バイナリ形式については`tellg()`の値をFFI経由で返すことで全バッファリングを回避できる余地がある。
