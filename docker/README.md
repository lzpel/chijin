# docker/

各 Rust ターゲット向けに OCCT をプレビルドし、`build.rs` がダウンロードして利用できる tarball を生成するためのディレクトリ。

## 目的

cadrum は OCCT (C++) に依存しており、ソースからのビルドには cmake・C++ コンパイラ・長いコンパイル時間が必要になる。エンドユーザーが `cargo build` するたびにこれを繰り返すのを避けるため、**ターゲット triple ごとに OCCT をビルド済みの tarball として配布する**。`build.rs` はデフォルトでこの tarball を取得して展開し、リンクするだけで済むようにしている。

このディレクトリは、その **tarball を生成するための環境** を Docker イメージまたは GitHub Actions ランナーとして定義する場所。

## 構成

- `Dockerfile_<target-triple>` — Linux / windows-gnu 向けのビルド環境定義。`docker build` 時にソースを image に `COPY` し、`RUN make cadrum-occt-${CARGO_BUILD_TARGET}` で tarball まで一気に作る
- `Makefile` — prebuilt 作成の **共通入口**。`cadrum-occt-<target-triple>` パターンルール 1 つだけを提供する。呼び出し経路は 3 つ:
  1. 各 Dockerfile の `RUN make cadrum-occt-${CARGO_BUILD_TARGET}` (docker build 時)
  2. `.github/workflows/prebuilt.yaml` の `build-windows-msvc` ジョブが `windows-2022` ランナー上で `make -C docker cadrum-occt-x86_64-pc-windows-msvc` を直接叩く
  3. ローカル開発で `make -C docker cadrum-occt-<triple>`

## Dockerfile の骨格

環境変数は cc クレート / cargo 標準の名前のみを使い、cadrum 独自の変数は一切定義しない:

```dockerfile
FROM <base>

# ツールチェイン install
RUN ...

# control cc crate and cargo command
ENV CC=<target C compiler>
ENV CXX=<target C++ compiler>
ENV AR=<target ar>
ENV CARGO_BUILD_TARGET=<rust target triple>

# add cargo target
RUN rustup target add ${CARGO_BUILD_TARGET}

# copy source and build OCCT prebuilt
COPY . /src
WORKDIR /src/docker
RUN make cadrum-occt-${CARGO_BUILD_TARGET}

# on `docker run -v $PWD/out:/out ...`, extract the tarball
CMD ["sh", "-c", "cp /src/cadrum-occt-*.tar.gz /out/"]
```

windows-gnu のみ、`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` / `CXXSTDLIB_x86_64_pc_windows_gnu` / `RUSTFLAGS -L <libstdc++.a dir>` を追加する(これらはいずれも cargo / cc-rs 標準の env var)。

## ローカルでの使い方

### Docker 経由(Linux / windows-gnu)

```sh
# 例: x86_64-unknown-linux-gnu
docker build -f docker/Dockerfile_x86_64-unknown-linux-gnu -t cadrum-prebuild:linux-gnu .
mkdir -p out
docker run --rm -v "$PWD/out:/out" cadrum-prebuild:linux-gnu
ls out/cadrum-occt-*.tar.gz
```

### Docker 外(ホストが target と一致する場合)

`docker/Makefile` は repo root を絶対パスで解決するので、コンテナの外からでもそのまま叩ける:

```sh
make -C docker cadrum-occt-x86_64-unknown-linux-gnu
```

成果物は **repo root** に `cadrum-occt-<slug>-<triple>.tar.gz` として出る。

## GitHub Actions でのビルド経路

`.github/workflows/prebuilt.yaml` に 2 種類のジョブがある:

- **`build` (Linux 系 matrix)** — `x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` / `x86_64-pc-windows-gnu` を Ubuntu ランナー上で `docker build` + `docker run` する
- **`build-windows-msvc`** — `x86_64-pc-windows-msvc` を `windows-2022` ランナー上で **Docker を介さず**, native `cl.exe` + `make -C docker cadrum-occt-x86_64-pc-windows-msvc` で直接ビルドする

### なぜ msvc だけ非対称なのか

cargo-xwin + clang-cl による cross ビルドで msvc prebuild を作ると、MSVC STL テンプレート(`std::variant` など)の COMDAT 排出判断が native `cl.exe` と食い違い、ユーザ側のリンク時に `std::_Variant_storage_<...>::~_Variant_storage_()` が未解決になる(issue #73 参照)。これは clang-cl と cl.exe の実装差分に根ざした構造的な ABI 境界問題で、ワークアラウンドが積み重なるだけで根治しない。

一方、**windows-gnu** は mingw-w64 + libstdc++ を両側で共有するので同じ問題は起きない。Linux 側 cross ビルドがエコシステム的にも成熟している。

そのため、

| target | builder | runner |
|---|---|---|
| `x86_64-unknown-linux-gnu` | Docker | `ubuntu-latest` |
| `aarch64-unknown-linux-gnu` | Docker | `ubuntu-24.04-arm` |
| `x86_64-pc-windows-gnu` | Docker | `ubuntu-latest` |
| `x86_64-pc-windows-msvc` | **native cl.exe** | **`windows-2022`** |

という**正当な非対称**になっている。public リポなので Windows ランナーの課金は発生しない。

## 現在プレビルドを配布している triple

- `x86_64-unknown-linux-gnu` — `manylinux_2_28_x86_64` ベース (AlmaLinux 8, glibc 2.28)。Ubuntu 18.10+/Debian 10+/RHEL 8+/Fedora 29+/Arch/openSUSE Leap 15.1+
- `aarch64-unknown-linux-gnu` — `manylinux_2_28_aarch64` ベース。Raspberry Pi 4/5 (64-bit OS)、AWS Graviton、Oracle Ampere、Apple Silicon Linux VM
- `x86_64-pc-windows-gnu`
- `x86_64-pc-windows-msvc` — native Windows runner で VS 2022 Build Tools を使ってビルド

musl 系 Linux (Alpine 等)、macOS (x86_64 / arm64)、Windows on ARM は現状プレビルド非対応。`cargo build --features source-build` で手元ビルドしてください。

### aarch64-unknown-linux-gnu のローカルビルド注意

x86_64 ホストで `docker run` すると Docker Desktop の QEMU user-mode emulation 経由になり、**OCCT ビルドに 3〜5 時間**かかります。日常的な iteration は x86_64 target だけで回し、aarch64 は GitHub Actions の `ubuntu-24.04-arm` runner (public repo は無料) に任せるのが現実的です。

## 新しいターゲットを追加するには

**Linux / windows-gnu 系の場合**:

1. `Dockerfile_<new-target-triple>` を追加する(上記「Dockerfile の骨格」に従う)
2. `.github/workflows/prebuilt.yaml` の `build` ジョブの matrix に 1 行追加する

`docker/Makefile` の `cadrum-occt-%` はパターンルールなので触らなくて良い。

**windows-msvc 以外で native runner が必要な場合**(例: macOS):

1. `.github/workflows/prebuilt.yaml` に独立したジョブを追加する(`build-windows-msvc` が参考)
2. 該当ランナーで `make -C docker cadrum-occt-<triple>` を実行する
