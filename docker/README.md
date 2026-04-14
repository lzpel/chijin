# docker/

各 Rust ターゲット向けに OCCT をプレビルドし、`build.rs` がダウンロードして利用できる tarball を生成するためのディレクトリ。

## 目的

cadrum は OCCT (C++) に依存しており、ソースからのビルドには cmake・C++ コンパイラ・長いコンパイル時間が必要になる。エンドユーザーが `cargo build` するたびにこれを繰り返すのを避けるため、**ターゲット triple ごとに OCCT をビルド済みの tarball として配布する**。`build.rs` はデフォルトでこの tarball を取得して展開し、リンクするだけで済むようにしている。

このディレクトリは、その **tarball を生成するためのクロスビルド環境** を Docker イメージとして定義する場所。

## 構成

- `Dockerfile_<target-triple>` — ターゲットごとのクロスビルド環境を定義する Dockerfile。ファイル名末尾の triple (例: `x86_64-unknown-linux-musl`) がそのまま `TARGET` 環境変数として使われる
- `entrypoint.sh` — すべてのイメージで共有する薄いラッパ。`cargo build --features source-build,color` で OCCT をソースからビルドし、`build.rs` が `target/cadrum-occt-<slug>-<TARGET>/` に書き出したディレクトリを glob で拾って tarball 化する
- `Makefile` — `Dockerfile_*` を自動検出して build / run を駆動するドライバ

## 使い方

```sh
make -C docker run           # すべてのターゲットを build + run し、out/ に tarball を出力
make -C docker run-<target>  # 単一ターゲットのみ
make -C docker build         # イメージのビルドだけ
make -C docker list          # 検出されたターゲット一覧
make -C docker clean         # out/ を削除
```

成果物は `../out/` に 2 種類出力される:

- `cadrum-occt-<occt-slug>-<target>.tar.gz` — プレビルド tarball。トップディレクトリ名は `cadrum-occt-<occt-slug>-<target>/` で、`build.rs` がそのままこの名前で `target/` 配下にキャッシュとして展開することを前提にしている
- `<target>.log` — そのターゲットの完全なビルドログ

命名ロジック (`<occt-slug>` = `OCCT_VERSION` の小文字化＋アンダースコア除去) は `build.rs` が唯一のソース。`entrypoint.sh` は slug を計算せず、`target/cadrum-occt-*-<TARGET>` を glob で拾うだけ。

ここで作られた tarball を GitHub Release (`OCCT_PREBUILT_TAG` = `occt-v800rc5`) にアップロードすると、エンドユーザーの `cargo build` が自動的にこの tarball を落としてきて使う。

## 現在プレビルドを配布している triple

- `x86_64-unknown-linux-musl`
- `x86_64-pc-windows-gnu`
- `x86_64-pc-windows-msvc`

ユーザーがこれ以外の triple を使う場合は `cargo build --features source-build` を付けることで、OCCT を upstream からソースビルドする経路を有効化できる。

## 新しいターゲットを追加するには

1. `Dockerfile_<new-target-triple>` を追加する
2. 先頭で `ENV TARGET=<new-target-triple>` を設定する
3. そのターゲット用のクロスコンパイラと必要な環境変数 (`CC_<triple>` / `CXX_<triple>` / `CARGO_TARGET_<TRIPLE>_LINKER` など) を設定する
4. 末尾で `entrypoint.sh` を `ENTRYPOINT` に指定する

`Makefile` は `Dockerfile_*` を自動検出するので、追加側の変更は不要。
