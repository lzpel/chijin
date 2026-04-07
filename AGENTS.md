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
- examples/book.rs
    - mdbook形式でドキュメントを生成します
- src/traits.rs
    - traits.rsはバックエンド共通のトレイト定義（pub(crate)、ユーザーに非公開）
    - トレイト名は`<Type>Struct`の命名規則に従う（SolidStruct→Solid, FaceStruct→Face等）
    - fnシグネチャは1行、#[cfg]は直前1行のみ認識、ライフタイム/where句は非対応
- build_delegation.rs
    - traits.rsをパースして$OUT_DIR/generated_delegation.rsを生成する
    - 生成コードはlib.rs末尾でinclude!される