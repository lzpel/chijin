# 基本方針

- OCCTの仕様とSTEPファイルの仕様でどちらに忠実な表現にするか迷うときはSTEPファイルの仕様を優先する
- 関数や構造体を増やす方向の検討より減らす方向の検討を優先する
- OCCTのモジュールを減らす方向の検討を優先する
- **ユーザーに誤解を招くか、僅かな手間を強いるかで迷ったら後者を取る**。例えば `impl IntoIterator<Item = &Edge>` を期待する API に対して `impl IntoIterator for &Edge` を足せば `func(&edge)` と単一要素で書けるようになるが、そうすると「この関数はコレクションを受け取る」というシグネチャの意図がユーザーから見えなくなる。**単一要素でも `&[edge]` や `[&edge]` と書かせて、「これは集合を受け取る関数だ」という事実を型レベルで認識してもらう方を優先する**。sugar でシグネチャの本来の意図を覆い隠すより、呼び出し側に3文字余分に書かせる方が長期的な理解に繋がる。

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
    - 第2引数: README.mdパス → ## Examples節を最新のソースコードと生成物で更新、SVGをfigure/examples/に配置
- src/traits.rs
    - traits.rsはバックエンド共通のトレイト定義（pub(crate)、ユーザーに非公開）
    - トレイト名は`<Type>Struct`の命名規則に従う（SolidStruct→Solid, FaceStruct→Face等）
    - fnシグネチャは1行、#[cfg]は直前1行のみ認識、ライフタイム/where句は非対応
- build_delegation.rs
    - traits.rsをパースして$OUT_DIR/generated_delegation.rsを生成する
    - 生成コードはlib.rs末尾でinclude!される