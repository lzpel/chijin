# windows-gnu prebuilt の静的リンク

前提としてRustのターゲットx86_64-pc-windows-gnuは以下でビルドされることを前提としている。
- posix スレッドモデル
- msvcrt/ucrt Cランタイム

何らかのC/C++ライブラリを同封する場合もこれらのスレッドモデルとCランタイムに合わせて静的ライブラリをビルドする必要がある。さもなくばスレッドモデルとCランタイムに相違があるバイナリをリンクしようとしてリンクエラーになる。

## 課題

`docker/Dockerfile_x86_64-pc-windows-gnu` が生成する OCCT prebuilt を `stable-x86_64-pc-windows-gnu` ユーザーが取り込んでできる exe を、配布時に追加 DLL を同梱せずとも動作する「自己完結バイナリ」にしたい。具体的には `libgcc_s_seh-1.dll` / `libstdc++-6.dll` / `libwinpthread-1.dll` の 3 つの mingw ランタイム DLL を全て静的に吸収し、最終 exe の runtime dep を `msvcrt.dll` (OS 同梱) と Win32 API DLL のみに絞る。

あわせて、`x86_64-w64-mingw32-gcc` 無印参照によってスレッドモデルが Debian の `update-alternatives` に依存している曖昧さも除去したい。

## 原因

単純に `build.rs` から `cargo:rustc-link-arg=-static` を emit するだけでは libstdc++ が吸収できない。調査の結果、次の二重の障害があることが判明した:

### 障害 1: rustc がネイティブライブラリ列挙前に `-Wl,-Bdynamic` をハードコード

rustc が windows-gnu 向けに生成するリンカコマンドは次の構造:

```
... (object files)
-Wl,-Bstatic
... (Rust rlib 群)
-Wl,-Bdynamic       <-- ここで強制的に Bdynamic に戻される
-lstdc++            <-- link-cplusplus が emit、ここで dynamic 解決
-lkernel32 -lgcc_eh -l:libpthread.a -lmsvcrt -lmingwex -lmingw32 -lgcc -lmsvcrt ...
-Wl,--allow-multiple-definition
-static             <-- build.rs から cargo:rustc-link-arg=-static
```

`-static` は gcc ドライバレベルのフラグで、コマンドライン全体を走査して `-lgcc` / `-lwinpthread` を静的変種へ書き換える。実際これで libgcc と libwinpthread は静的吸収される。しかし `-lstdc++` は既に `-Wl,-Bdynamic` で ld の状態が dynamic に切り替わった後に出現するため、`-static` が手を付ける前に `libstdc++.dll.a` (import library) で解決されて終わる。

`-static-libstdc++` も同じ理由で効かない。`-Wl,-Bstatic -lstdc++ -Wl,-Bdynamic` を末尾に追加しても、link-cplusplus 由来の先行 `-lstdc++` (dynamic) が既に libstdc++-6.dll import を作成済みなので DLL 依存は消えない。

gcc で `x86_64-w64-mingw32-g++-posix /tmp/t.cpp -static-libgcc -static-libstdc++` を直接実行すると期待通り静的リンクできるのに、rustc 経由では同じ結果にならないのはこのためである。

### 障害 2: link-cplusplus の emit を切り替えるには CXXSTDLIB 経由、検索パスは別途 RUSTFLAGS 経由

`cxx` が間接的に依存する `link-cplusplus` クレートは、`cc` crate 経由で `CXXSTDLIB` / `CXXSTDLIB_<target>` / `TARGET_CXXSTDLIB` の環境変数を読み、その値をそのまま `cargo:rustc-link-lib={value}` として emit する。value に rustc の link-lib 修飾子 (`static=stdc++` など) を渡すとそれが直接 `-l static=stdc++` として渡るので、`CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++` を設定すれば link-cplusplus の emit が static 扱いになる。

ただし、rustc は `#[link(name="stdc++", kind="static")]` を含むクレート (link-cplusplus 本体) をコンパイルする時点で `libstdc++.a` が `-L` 検索パス上に存在することをチェックする。link-cplusplus は cxx → cadrum のビルドチェーンで cadrum よりずっと先にビルドされるため、cadrum の `build.rs` から `cargo:rustc-link-search=native=...` を emit してもそのときには既にチェックが終わっており、error: `could not find native static library 'stdc++'` で link-cplusplus 自体のコンパイルが失敗する。

したがって libstdc++.a の検索パスは build.rs より早いタイミングで、全クレートに対して与えなければならない。実質 `RUSTFLAGS="-L <dir>"` を環境変数として cargo 起動前に与えるのが唯一の経路となる。

## 解決策

3 つの仕掛けを組み合わせる:

### 1. `build.rs`: `-static` を windows-gnu のみに emit

```rust
if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
{
    println!("cargo:rustc-link-arg=-static");
}
```

これで libgcc と libwinpthread が吸収される。linux-gnu で誤爆しないよう OS+ENV 両方で gate する (linux で `-static` は glibc を静的リンクしようとして失敗する)。

### 2. `Dockerfile`: `CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++`

```dockerfile
ENV CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++
```

link-cplusplus の cc crate 経由 emit を `cargo:rustc-link-lib=static=stdc++` に切り替える。これにより rustc が生成するリンカコマンドで `-lstdc++` が `-Wl,-Bstatic -lstdc++ -Wl,-Bdynamic` のブラケットで囲まれ、`libstdc++.a` に解決される。

### 3. `Dockerfile`: entrypoint wrapper で `RUSTFLAGS=-L <dir>` を baked-in

```dockerfile
RUN LIBSTDCXX_DIR="$(dirname "$(x86_64-w64-mingw32-g++-posix -print-file-name=libstdc++.a)")" && \
    printf '#!/bin/sh\nexport RUSTFLAGS="-L %s ${RUSTFLAGS:-}"\nexec /entrypoint.sh "$@"\n' \
        "$LIBSTDCXX_DIR" > /entrypoint-wrapper.sh && \
    chmod +x /entrypoint-wrapper.sh

ENTRYPOINT ["/entrypoint-wrapper.sh"]
```

イメージビルド時に `g++-posix -print-file-name=libstdc++.a` で sysroot 内のパスを動的に取得し、それを含む `RUSTFLAGS` を固定値としてラッパースクリプトに焼き込む。Debian が gcc を bump したらイメージ再ビルド時に自動で追従する。`cargo:rustc-link-search` を build.rs から emit しても link-cplusplus の rlib コンパイル時には間に合わないので、この経路が実質唯一の解。

### 4. `Dockerfile`: posix サフィックスを明示

```dockerfile
ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
ENV CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
```

Debian `update-alternatives` の既定は現状 posix だが、将来 win32 に変わると silently ABI が壊れる。defensive に明示しておく (`ar` はスレッドモデルに依存しないのでサフィックスなし)。

## 検証結果

`01_primitives` を prebuilt Docker イメージ内で `cargo build --release --example` し、`x86_64-w64-mingw32-objdump -p` で DLL import table を確認:

```
DLL Name: KERNEL32.dll
DLL Name: msvcrt.dll
DLL Name: ntdll.dll
DLL Name: USERENV.dll
DLL Name: WS2_32.dll
DLL Name: api-ms-win-core-synch-l1-2-0.dll
DLL Name: bcryptprimitives.dll
```

全て OS 同梱 DLL のみ。`libgcc_s_seh-1.dll` / `libstdc++-6.dll` / `libwinpthread-1.dll` はいずれも imports に現れない。

## downstream ユーザーへの波及

cadrum を依存に持つ外部クレートから windows-gnu ビルドする場合、**同じ環境変数 2 点を downstream 側の build 環境にも設定する必要がある**:

```bash
export CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++
export RUSTFLAGS="-L /usr/lib/gcc/x86_64-w64-mingw32/14-posix"  # mingw gcc バージョンに応じて
```

または `.cargo/config.toml` で等価な設定を書く。これは `build.rs` からは制御不能な領域 (別クレートのビルドスクリプト環境には踏み込めないため)。README に記載する形で UX を補う。

環境変数を設定しない downstream では、リンク自体は成功するが最終 exe が `libstdc++-6.dll` に動的依存する状態になる (cadrum が `build.rs` で `-static` を emit しているので libgcc / libwinpthread は引き続き静的吸収される)。配布時に libstdc++-6.dll を同梱する前提なら十分実用的。

## 却下した案

- **UCRT に寄せる (`x86_64-pc-windows-gnullvm` + llvm-mingw)**: 理想的だが Rust ターゲット triple 変更を伴い、既存の gnu ユーザーを切り捨てる。release ワークフロー / Dockerfile ファイル名 / prebuilt tarball 名の大規模変更が必要。今回の fix スコープでは過剰。
- **Debian trixie の `gcc-mingw-w64-ucrt64` で OCCT だけ UCRT 化**: Rust libstd が msvcrt 決め打ちなので二重 CRT になり、`__acrt_iob_func` 等の UCRT 固有シンボルが未解決になる。
- **`-static-libgcc -static-libstdc++ -Wl,-Bstatic -lwinpthread -Wl,-Bdynamic` のみ**: rustc のハードコードされた `-Wl,-Bdynamic` を上書きできないため libstdc++ が吸収されない。
- **`cargo:rustc-link-search=native=...` を build.rs から emit**: link-cplusplus の rlib コンパイル時には間に合わず、error: `could not find native static library 'stdc++'` で失敗する。
- **`.cargo/config.toml` で RUSTFLAGS 設定**: cadrum ワークスペース内のビルドにしか効かず、downstream ユーザーには届かない (そもそも downstream UX は環境変数で要求する前提に割り切った)。

## 追記: prebuilt tarball に `libstdc++.a` を同梱する理由 (PR #63)

上記 3 点 (`-static` / `CXXSTDLIB=static=stdc++` / `RUSTFLAGS=-L`) は **コンテナ内で作る最終 exe** に対しては完結しているが、**cadrum を依存に取る downstream ユーザーが tarball を使ってビルドする** 経路には届かない。downstream の gcc バージョンが prebuilt を作ったコンテナと異なると、`std::istream::seekg(std::fpos<int>)` 等で undefined reference が多発する事象が観測された (cadrum2 + mingw gcc 15.2.0)。

### なぜ CXXSTDLIB の static 設定では防げないか

`CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++` は link-cplusplus が rustc に渡す link 指示を「dynamic → static」に切り替えるだけで、**どの `libstdc++.a` を引くかは -L サーチパス任せ**。prebuilt tarball に入っているのは OCCT の `.a` (= `.o` の詰め合わせ) のみで、libstdc++ の実体は混ざっていない。libstdc++ のシンボルは未解決参照のまま残り、最終 exe の ld 段階で初めて解決される。

downstream が自分の環境で最終 link を実行するとき:

1. OCCT の `.a` 群 (コンテナ内 gcc 14 でコンパイル) を link 対象に含める
2. `.o` は **gcc 14 の libstdc++ ヘッダを見て出力された mangled シンボル参照**を持っている
3. ld は `-L` サーチパス上の **downstream の libstdc++** を参照
4. downstream が gcc 15.2.0 なら、その libstdc++ は gcc 14 と export 構成が異なる (`std::istream::seekg(std::fpos<int>)` 周りの export が変動)
5. 結果 undefined reference

downstream 側が `CXXSTDLIB=static=stdc++` を設定していようがいまいが、**引く先が downstream の libstdc++ である**事実は変わらない。静的/動的の違いでしかない。

### なぜ Linux では顕在化しないか

linux-gnu の prebuilt は libstdc++ を bundle せず downstream の `libstdc++.so.6` に任せており、`OCC_CONVERT_SIGNALS` も有効のままだが問題が出ない。これはたまたまではなく、**ELF/glibc/libstdc++ の ABI 安定設計と PE/mingw libstdc++ の非バージョニング設計の構造差**による。

| 観点 | Linux (ELF + glibc) | Windows mingw (PE + libmingwex) |
|---|---|---|
| libstdc++ の ABI | symbol versioning (`GLIBCXX_3.4.X`) で後方互換を強制保証、過去タグを削除しない | 非 versioning、PE import/export は名前ベースのみ、gcc 跨ぎで export が消える |
| `_setjmp` の供給元 | glibc (POSIX 公開 API、数十年 ABI 不変) | libmingwex (mingw-w64 独自メンテ、x86_64 SEH 対応で過去にシンボル名が揺れた) |
| gcc バージョン跨ぎ prebuilt | 構造的に安全 | 構造的に危険 |
| `OCC_CONVERT_SIGNALS` | 必要 (POSIX シグナルハンドラ内から C++ throw 不可) | 不要 (SEH で賄える、OCCT docs も明記) |

libstdc++.so.6 は新しい gcc が出ても過去の `GLIBCXX_3.4.*` タグを全て維持するのがメンテポリシーなので、gcc 14 で作った `.o` が gcc 15 環境でも解決する。一方 mingw の libstdc++-6.dll は単純な PE export table しか持たず、gcc 側で inline 展開が変わるなどして過去のシンボルが export から消える事象が定期的に起きる。「mingw でビルドした C++ バイナリは gcc バージョンを跨げない」というのは mingw コミュニティの既知事項。

### 修正: build 時の libstdc++.a を tarball に同梱

`build_occt_from_source` 末尾で、ビルドツールチェーンの `g++-posix -print-file-name=libstdc++.a` からパスを取得し、prebuilt の `lib_dir` に copy する。tarball に自動で含まれる。

downstream の link 時には `build.rs` から `cargo:rustc-link-arg=-l:libstdc++.a` を emit し、ld にファイル名直指定で解決させる。`-l:<file>` は `-L` サーチパス上の literal ファイル名にヒットする指定で、rustc がまず prebuilt の `lib_dir` を `-L` に積むため、downstream の libstdc++ より **bundle 版 (gcc 14 版)** が先に引かれる。結果、`.o` と同じ gcc バージョンの libstdc++ から mangled シンボルが埋まり、version skew が発生しない。

あわせて、`OCC_CONVERT_SIGNALS` 経路は Windows では本来不要なので、`patch_occt_sources` で windows-gnu ビルド時に `adm/cmake/occt_defs_flags.cmake` の `add_definitions(-DOCC_CONVERT_SIGNALS)` をコメントアウトし、`.a` から `_setjmp` 参照自体を除去する。これで libmingwex の `_setjmp` バージョン揺れに対する依存もなくなる。

### 副作用

tarball サイズ: 56 → 57 MB (libstdc++.a 分 +1MB)。`OCC_CONVERT_SIGNALS` を切ったことで OCCT 内部の「シグナル → C++ 例外」変換が windows-gnu で無効化されるが、これは MSVC default と同じ挙動で、通常の幾何演算 (`Standard_Failure` 系の throw) には影響なし。

## ソース

- [How can I statically link libstdc++-6 when cross compiling to x86_64-pc-windows-gnu — rust-lang forum](https://users.rust-lang.org/t/how-can-i-statically-link-libstdc-6-when-cross-compilint-to-x86-64-pc-windows-gnu-from-linux/106587)
- [rustc -C link-args=-static-libgcc does not work on Windows — rust-lang/rust#15420](https://github.com/rust-lang/rust/issues/15420)
- [Statically link libstdc++ on windows-gnu (rustc 内部 PR) — rust-lang/rust#65911](https://github.com/rust-lang/rust/pull/65911)
- [Consider dynamically link to libgcc_s when targeting windows-gnu — rust-lang/rust#89919](https://github.com/rust-lang/rust/issues/89919)
- [link-cplusplus crate docs](https://docs.rs/link-cplusplus/latest/link_cplusplus/)
- [Binding c++ with cxx error on windows-gnu — rust-lang/rust#137301](https://github.com/rust-lang/rust/issues/137301)
- 本リポジトリ PR: [lzpel/cadrum#60](https://github.com/lzpel/cadrum/pull/60)

## 追記 2026-04-15: wrapper.o vtable 問題の発覚と部分的ワークアラウンド削除 (issue #66)

上記「追記: prebuilt tarball に `libstdc++.a` を同梱する理由」で導入した bundle + `-l:libstdc++.a` の file-exact pull 経路は、観測されていた undefined reference の **真の原因を誤診していた** ことが判明し、issue #66 で修正・撤去した。

### 観測

ローカル mingw-w64 gcc 15.2.0 (winlibs 系) で prebuilt `cadrum-occt-v800rc5-x86_64-pc-windows-gnu` を取り込むと、依然として undefined reference が発生:

```
wrapper.o:wrapper.cpp:(.rdata$_ZTVN6cadrum18RustWriteStreambufE+0x38):
    undefined reference to `std::basic_streambuf<char, std::char_traits<char> >
        ::seekpos(std::fpos<_Mbstatet>, std::_Ios_Openmode)'
```

未解決シンボルの参照元は **`wrapper.o` 1 箇所のみ**。OCCT 本体 (`libTKernel.a` 等) からの未解決参照は 0 件。上記追記セクションの想定 (「OCCT 内部の inline 展開が gcc14/15 で揺れる」) とは違っていた。

### 真の原因

`cpp/wrapper.h` の `RustReadStreambuf` / `RustWriteStreambuf` は `std::streambuf` の virtual を一部 (`underflow` / `overflow` / `xsputn` / `sync`) しか override していない。未 override スロット (特に `seekpos`) の vtable エントリは、コンパイラが基底 `std::basic_streambuf<char>::seekpos` の関数ポインタを wrapper.o 内に埋めて解決する必要があり、**これが extern シンボル参照として wrapper.o から要求される**。

`seekpos` のマングリングは `std::fpos<mbstate_t>` を含み、`mbstate_t` は mingw で内部 typedef `_Mbstatet` に resolve される。gcc 15 と gcc 14 でこの typedef 名周りの扱いが異なり、prebuilt 同梱の gcc 14 版 `libstdc++.a` にもユーザー sysroot の gcc 15 `libstdc++.dll.a` にも一致する export が存在しない (`-l:libstdc++.a` 経路でも落ちる)。

### 修正 (案 C-minimal)

両クラスに **`seekpos` の no-op override を追加**するだけで解決:

```cpp
std::streambuf::pos_type RustReadStreambuf::seekpos(pos_type, std::ios_base::openmode) {
    return pos_type(off_type(-1));
}
// RustWriteStreambuf も同じ
```

override を付けることで vtable スロットが wrapper.o 内の自前関数ポインタで埋まり、extern `seekpos` シンボル参照が完全に消える。libstdc++ のバージョン差と ABI の問題から wrapper.o が切り離される。OCCT の STEP / BRep I/O は前方向のみで `seekpos` を呼ばないため no-op で機能影響なし。`pos_type(off_type(-1))` は streambuf 契約上の「seek 失敗」値。

### OCCT 本体については gcc14/15 間で ABI 問題なし

今回 OCCT `.a` 由来の undefined reference は 1 件も観測されなかった。これは上記追記が警戒していた「OCCT 内部の inline 展開が gcc 跨ぎで揺れる」現象が、少なくとも現在の OCCT 800rc5 + 使用 API 範囲では **顕在化していない** ことを示唆する。OCCT の libstdc++ 依存は `std::string` / `std::ios_base::Init` など ABI が安定な関数に留まっており、gcc 14 / 15 のマングリングが一致している。

### 撤去した仕掛け

issue #66 で以下を削除した:

1. `build.rs` — `link_occt_libraries` の `println!("cargo:rustc-link-arg=-l:libstdc++.a")` 行 (file-exact pull の強制)
2. `build.rs` — `build_occt_from_source` 末尾の libstdc++.a バンドルブロック全体 (1 の消費者がなくなり dead code 化)
3. 次回の Docker rebuild 時に、上記 2 が走らなくなることで prebuilt tarball 内の `win64/gcc/lib/libstdc++.a` も自動的に消える (現行 v800rc5 は残置のまま無害)

### 維持した仕掛け

`-static` (build.rs) と `CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++` + RUSTFLAGS `-L` bake (Dockerfile) は **そのまま維持**。これらは libgcc / libwinpthread / libstdc++ 一般の静的吸収を担う仕掛けで、wrapper.o の vtable 問題とは独立している。最終成果物の runtime 依存は引き続き msvcrt.dll + Win32 API DLL のみ。libstdc++ はユーザーローカル (あるいは Docker 環境内) の gcc 版 `libstdc++.a` に対して link-cplusplus 経由の通常静的リンク経路で解決される (wrapper.o が gcc バージョン固有シンボルを要求しなくなったため、gcc14 固定が不要になった)。

### 教訓

未解決シンボルのエラーログを読むとき、**参照元がどのオブジェクトファイルから来ているか** (`wrapper.o:wrapper.cpp:(.rdata$_ZTV...)`) を最初に確認すべき。wrapper.o 1 箇所に閉じているなら、原因は OCCT ではなく wrapper.cpp の C++ クラス定義にある可能性が高い。上記追記は当時この切り分けを行わず「OCCT 内部」と決め打ちしたため、過剰な bundle + file-exact pull という遠回りの修正に至った。
