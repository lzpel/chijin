# OCCT ソーススタブ化 parser のエッジケース (Windows MSVC native build)

## 背景

`build.rs::patch_occt_sources` は Windows MSVC ターゲットで OCCT の OS 抽象レイヤ (`OSD_*.cxx` ほか) の関数本体をテキスト置換で空スタブ化している (`advapi32` / `user32` リンクを避ける目的)。従来は本体を一律 `{}` に置き換えていたが、これは

- Linux の GCC では missing-return 警告で済む
- MSVC では **C4716 ("must return a value") がエラー扱い** で `/W0` でも抑止不可

という理由で MSVC native build を追加した時点で破綻した。

修正方針は「void 関数・コンストラクタ・デストラクタ → `{}` / それ以外 → `{ return {}; }`」という単純なルールで、戻り値型ごとの value-initialization に委ねる。ただしシグネチャを正確に parse しないと誤判定で別種のコンパイルエラー (`C2534` / `C2562` / `C4716`) が続出する。本 note は実際に破綻した OCCT のソース片を列挙し、`build.rs::stub_body_for_sig` がどの段階でそれを吸収するかを記録する。

## 破綻事例と対策

### 1. `A :: B` 形式 (OCCT 古典スタイルの `::` 前後空白)

```cpp
OSD_Protection ::OSD_Protection()
{}

OSD_Protection ::OSD_Protection(const OSD_SingleProtection System,
                                const OSD_SingleProtection User, ...)
{ ... }

void OSD_Protection ::Values(OSD_SingleProtection& System, ...)
{ ... }
```

- `::` の前後に空白が入る OCCT 独自スタイル。
- walk-back で関数名を拾うとき `name = "OSD_Protection"` 単一セグメントになり、`Foo::Foo` パターンでのコンストラクタ検出が外れて `{ return {}; }` を出力 → `error C2534: constructor cannot return a value` (+ `C2562: 'void' function returning a value`)。
- **対策**: parse の前段で `" ::"` → `"::"`、`":: "` → `"::"` を収束まで繰り返し、`A::B` に正規化する。

### 2. 直前の関数の trailing コメントに `(` が混入

```cpp
OSD_Protection ::OSD_Protection()
{} // end constructor ( 1 )

OSD_Protection ::OSD_Protection(const OSD_SingleProtection System, ...)
```

- 2 個目のコンストラクタを parse するとき、`sig` は直前の `}` 以降を含むので `// end constructor ( 1 )` が入る。
- `sig.find('(')` がコメント内の `(` をヒットし、head が `// end constructor ` となる。結果 `name = "constructor"`, `return_part = "// end"` で、constructor 判定に失敗して `{ return {}; }`。
- **対策**: parse 前に `//...\n` と `/* ... */` を除去する。

### 3. ブロックコメント内のコード片 (`}` が sig 境界に漏れる)

```cpp
/*void OSD_File::Link (const TCollection_AsciiString& theToFile)
{}*/

#if defined(__CYGWIN32__) || defined(__MINGW32__)
  ...
#endif

void OSD_File::SetLock(const OSD_LockType theLock)
{ ... }
```

- `stub_all_top_level_bodies` の sig 抽出は raw content 上で `rfind(';' | '}')` する。`{}*/` の中の `}` は **コメント内だが raw 走査ではただの `}`** なので、ここが sig の開始点として採用される。
- 結果 sig が `*/\n\n#if defined(__CYGWIN32__)...\n\nvoid OSD_File::SetLock(...)` で始まる。
- stub_body_for_sig 内の comment stripper は `/* ... */` のペアを探すが、この `sig` には対応する `/*` が無く `*/` が孤児として残る。その後ろの `#if defined(` の `(` が最初の `(` として採用され、head が `*/` 付きの壊れた文字列になる。
- **対策**: sig 先頭に孤児 `*/` を検出したら (その前に `/*` が無い場合)、そこまでを一括で切り落とす。これでコメント途中から sig が始まるケースを無害化する。

### 4. Preprocessor 行の `(`

```cpp
#if defined(__CYGWIN32__) || defined(__MINGW32__)
  #define __try
#endif
```

- 上記 3 のようにコメント境界から sig が入るケース、あるいは単に関数間に `#if` がある場合、`sig.find('(')` が `defined(` などの `(` を拾って head を壊す。
- **対策**: parse 前に sig を行単位で走査し、trim 後に `#` で始まる行を一括削除。

### 5. マクロ呼び出し `IMPLEMENT_STANDARD_RTTIEXT(...)` を関数シグネチャと誤認

```cpp
IMPLEMENT_STANDARD_RTTIEXT(XCAFDoc_VisMaterial, TDF_Attribute)

//=================================================================================================

const Standard_GUID& XCAFDoc_VisMaterial::GetID()
{ ... }
```

- ファイル先頭近くには `IMPLEMENT_STANDARD_RTTIEXT(...)` のような展開後型情報を登録する OCCT 標準マクロがある。セミコロンを伴わず、かつ `(` を含む。
- sig は `last_end=0` (ファイル先頭) から始まるので、`strip_comments` → `strip_preprocessor` を通した後も `IMPLEMENT_STANDARD_RTTIEXT(XCAFDoc_VisMaterial, TDF_Attribute)\n\nconst Standard_GUID& XCAFDoc_VisMaterial::GetID()` が残る。
- `find('(')` が最初の `(` = `IMPLEMENT_STANDARD_RTTIEXT(` を拾い、head が `IMPLEMENT_STANDARD_RTTIEXT` だけになって、`name = "IMPLEMENT_STANDARD_RTTIEXT"` / `return_part = ""` → 「return type 無し = ctor/operator 扱い」で `{}` が出力される。GetID は実際には `const Standard_GUID&` を返すので、非 void に対する `{}` となり `error C4716`。
- **対策**: `(` を前から順に探し、**直前の識別子が全大文字 (`[A-Z0-9_]+` かつ 1 文字以上大文字)** ならマクロ呼び出しとみなしてその対応する `)` までスキップ、次の `(` を探す。全大文字は OCCT/C++ のマクロ命名規則に合致するが、関数・クラス名とは衝突しない (例: `OSD_Protection` には小文字 `rotection` が含まれるので誤判定しない)。

### 6. `class Standard_DbgHelper { ... };` を関数本体と誤認 (Standard_StackTrace.cxx)

```cpp
#elif defined(_WIN32) && !defined(OCCT_UWP)

  #include <windows.h>
  #include <dbghelp.h>

class Standard_DbgHelper
{
  // ... static members and helpers ...
};

#endif
```

- `class Standard_DbgHelper { ... };` はクラス定義で関数ではないが、stub_all_top_level_bodies の旧 `is_var_init` 判定は `!sig.contains('(')` だった。直前行の `#elif defined(_WIN32)` に `(` があるため条件が偽になり、この class 本体が関数本体として stub 化されていた。
- 結果 `class Standard_DbgHelper { throw 0; };` や `class Standard_DbgHelper { return {}; };` のような構文エラー (C2059: `syntax error: 'throw'/'return'`) を生む。
- **対策**: 関数本体判定を「**sig の末尾行から `const` / `override` / `final` / `noexcept` / `mutable` / `volatile` / `= 0` を剥いだ残りが `)` で終わる**」方式に変更。関数定義は必ず `)` (params の閉じ、あるいは init-list 要素の `)`) で終わり、class / struct / namespace ヘッダは識別子か `>` で終わるため、両者を確実に区別できる。preprocessor 行の `(` や `)` は末尾行だけ見ることで影響を排除する。

## 最終的な stub_body_for_sig のパイプライン

1. sig がブロックコメント途中から始まる (先頭に孤児 `*/`) なら先頭を切り落とす
2. `//` と `/* */` をテキストから除去
3. `#` で始まる行 (preprocessor) を削除
4. `A :: B` → `A::B` の空白正規化
5. マクロ呼び出し (`[A-Z_][A-Z0-9_]*` + `(`) をスキップしつつ関数の `(` を探す
6. 得られた `(` の前を head として walk-back で関数名を抽出
7. name に `~` → デストラクタ / `Foo::Foo` セグメント一致 → コンストラクタ → `{}`
8. return_part に whole-word の `void` (後ろに `*` / `&` が付かない) → `{}`
9. それ以外 → `{ return {}; }`

関数本体判定 (`is_function`) は別途、`stub_all_top_level_bodies` 側で「sig 末尾行が `)` で終わる」で行う。両者は役割が異なるので分離している。

## 実行時への副次効果

`{ return {}; }` は各戻り値型を value-initialization するので、ほとんどの OCCT OSD 関数について実用上無害な値 (`false` / `0` / 空文字列 / デフォルト構築値) を返す。従来の `{}` + 非 void (UB) や `throw 0;` (foreign exception で Rust 側 abort) ではいずれも 01_primitives 実行時に crash していたが、`{ return {}; }` 方針では **実行時も通って `.step` / `.svg` が生成されるようになった** (想定外の副産物)。ただしこれは OCCT が戻り値を緩くチェックしているだけで保証された挙動ではないので、本命の解としては issue #80 (「スタブをやめて advapi32 / user32 を素直にリンクする」) の方針を引き続き検討する価値がある。

## 参考

- 関連 issue: #80
- 対象コミット: 本 note と同じ PR
- OCCT ソース参照 (V8_0_0_rc5):
  - `src/FoundationClasses/TKernel/OSD/OSD_Protection.cxx` — 事例 1, 2
  - `src/FoundationClasses/TKernel/OSD/OSD_File.cxx` — 事例 3, 4
  - `src/DataExchange/TKXCAF/XCAFDoc/XCAFDoc_VisMaterial.cxx` — 事例 5
  - `src/FoundationClasses/TKernel/Standard/Standard_StackTrace.cxx` — 事例 6
