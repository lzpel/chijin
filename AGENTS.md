# 基本方針

1. OCCTの仕様とSTEPファイルの仕様でどちらに忠実な表現にするか迷うときはSTEPファイルの仕様を優先する
2. 関数や構造体を増やす方向の検討より減らす方向の検討を優先する
3. OCCTのモジュールを減らす方向の検討を優先する
4. ユーザーに誤解を招くか、僅かな手間を強いるかで迷ったら後者を取る。

# 設計判断

- 第4方針違反で却下: `impl IntoIterator<Item = &Edge>` を期待する API に対して `impl IntoIterator for &Edge` を足せば `func(&edge)` と単一要素で書けるようになるが、そうすると「この関数はコレクションを受け取る」というシグネチャの意図がユーザーから見えなくなる。
- 採用: 単一 `DVec3` 引数は `impl Into<DVec3>` で受ける (issue #99)。`translate([0,1,0])` / `translate(DVec3::Z)` の両方を許容しつつ、シグネチャに `Into<DVec3>` の名前が残るため「ベクトル1つを受ける」意図は型レベルで保たれる。コレクション (`impl IntoIterator<Item = &DVec3>`) や enum バリアント (`ProfileOrient::Up(DVec3)`) は型シグネチャの意図がより強いため対象外。

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