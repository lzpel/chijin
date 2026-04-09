# 基本方針

- OCCTの仕様とSTEPファイルの仕様でどちらに忠実な表現にするか迷うときはSTEPファイルの仕様を優先する
- 関数や構造体を増やす方向の検討より減らす方向の検討を優先する
- OCCTのモジュールを減らす方向の検討を優先する

# ディレクトリ構成

- cpp/wrapper.h/cpp
    - occtとrust間のバインディング
- notes/YYYYMMDD-日本語タイトル.md
    - 設計方針などを記録
- examples/00_*.rs
    - このリポジトリのサンプルコードです。実行するとカレントディレクトリに00_*.svg/stepが生成されます。この命名規則に従う出力ファイルはbook.rsによりドキュメント内からリンクされます。
- examples/markdown.rs
    - 番号付きexample (NN_*.rs) を実行し、mdbook用markdownとREADMEのExamples節を生成する
    - 使い方: `cargo run --example markdown -- out/markdown/SUMMARY.md ./README.md`
    - 第1引数: SUMMARY.mdパス → mdbook用markdown一式を出力
    - 第2引数: README.mdパス → ## Examples節を最新のソースコードと生成物で更新（画像は GitHub Pages 上の `https://lzpel.github.io/cadrum/<name>.svg` を参照）
- src/traits.rs
    - traits.rsはバックエンド共通のトレイト定義（pub(crate)、ユーザーに非公開）
    - トレイト名は`<Type>Struct`の命名規則に従う（SolidStruct→Solid, FaceStruct→Face等）
    - fnシグネチャは1行、#[cfg]は直前1行のみ認識、ライフタイム/where句は非対応
- build_delegation.rs
    - traits.rsをパースして$OUT_DIR/generated_delegation.rsを生成する
    - 生成コードはlib.rs末尾でinclude!される