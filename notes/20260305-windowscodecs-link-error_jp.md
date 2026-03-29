# TKService リンクエラー調査・修正記録 (0.3.3, 2026-03-05)

## 発端: windowscodecs 未解決シンボルエラー

chijin を依存クレートとして追加した別プロジェクト（`color` feature **なし**）を
Windows/MinGW でビルドすると、リンク時に以下のような未解決シンボルエラーが発生した。

```
undefined reference to `GUID_WICPixelFormat8bppIndexed'
undefined reference to `CLSID_WICImagingFactory'
undefined reference to `GUID_WICPixelFormat24bppBGR'
...
```

いずれも `libchijin-*.rlib(Image_AlienPixMap.cxx.obj)` から来ていた。

### 原因

`build.rs` の `occ_libs` に `"TKService"` が**無条件**で含まれていた。
`TKService` は `Image_AlienPixMap.cxx` を含み、Windows では WIC (Windows Imaging Component) を使って
PNG/JPEG/BMP の読み書きを行う。WIC には `ole32` / `windowscodecs` が必要だが、
これらは `color` feature のときだけリンク指示されていた。

```rust
// 修正前: TKService が常にリンクされる
let occ_libs = &[
    "TKernel", ..., "TKService",  // ← 問題
];

if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
    println!("cargo:rustc-link-arg=-ladvapi32");
    if color {  // ← color なしでは windowscodecs がリンクされない
        println!("cargo:rustc-link-arg=-lole32");
        println!("cargo:rustc-link-arg=-lwindowscodecs");
    }
}
```

chijin 自身のテストは `color` feature 付きで実行していたため気づかなかった。

---

## 第1の修正案: TKService を color feature 専用に移動

`nm` で確認した結果、`TKernel`〜`TKDESTEP`（base libs）および `wrapper.cpp` は
`TKService` のシンボルを一切参照しないことが判明。

→ `TKService` を `occ_libs` から削除し、`color` feature 時のみ追加する方針を採用した。

```rust
let occ_libs = &[
    "TKernel", ..., "TKDESTEP",  // TKService を削除
];

if color {
    for lib in &["TKLCAF", "TKXCAF", "TKCAF", "TKCDF", "TKService"] {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=-lole32");
        println!("cargo:rustc-link-arg=-lwindowscodecs");
    }
}
```

この時点では `color` feature なしのエラーは解消される見込みだった。

---

## 第2の問題: color feature でも TKService が必要になる理由の調査

`TKXCAF` が `Graphic3d_*`（TKService）を参照するという仮定のもと、
`TKService` を color 時にリンクすれば解決と思われた。

しかし実験的に `TKService` / `ole32` / `windowscodecs` を**すべて削除**してビルドすると、
以下のエラーが発生した。

```
undefined reference to `Graphic3d_MaterialAspect::Graphic3d_MaterialAspect()'
undefined reference to `Graphic3d_PBRMaterial::MetallicFromSpecular(...)'
undefined reference to `vtable for Graphic3d_TextureSet'
...
```

出所は `XCAFDoc_VisMaterial.cxx.obj` と `XCAFPrs_Texture.cxx.obj` の2ファイルのみ。

### 依存チェーンの解明

```
XCAFDoc_DocumentTool
  └─ XCAFDoc_VisMaterialTool::Set/GetID
       └─ XCAFDoc_VisMaterial::GetID / ctor
            ├─ FillMaterialAspect()  → Graphic3d_MaterialAspect  (TKService)
            ├─ FillAspect()          → Graphic3d_Aspects, XCAFPrs_Texture  (TKService)
            ├─ ConvertToPbrMaterial()   → Graphic3d_PBRMaterial  (TKService)
            └─ ConvertToCommonMaterial() → Graphic3d_PBRMaterial  (TKService)

XCAFPrs_Texture
  └─ 継承: Graphic3d_Texture2D  (TKService)
```

### OCCT モジュール構成の確認

`adm/MODULES` を確認した結果：

```
Visualization      TKService TKV3d TKOpenGl ...
ApplicationFramework TKCDF TKLCAF TKCAF ...
DataExchange       ... TKXCAF TKXmlXCAF TKBinXCAF ...
```

- `TKXCAF` は **DataExchange** モジュール（`BUILD_MODULE_DataExchange=ON` で含まれる）
- `TKService` は **Visualization** モジュール（`BUILD_MODULE_Visualization=OFF` で除外される）
- `TKLCAF`/`TKCAF`/`TKCDF` は **ApplicationFramework** モジュール（`BUILD_MODULE_ApplicationFramework=OFF` で除外）

つまり bundled ビルドでは `TKXCAF` はビルドされるが `TKService` はビルドされない。
`XCAFDoc_VisMaterial.cxx` がコンパイルされる際に Graphic3d ヘッダを参照するため
リンクエラーになる。

---

## 最終修正: OCCT ソースのパッチ適用

`TKService` をリンクせずに `TKXCAF` を使えるようにするため、
cmake ビルド前に OCCT ソースファイルを直接書き換える方式を採用した。

### パッチ対象と方針

| ファイル | 対応 |
|---|---|
| `src/XCAFDoc/XCAFDoc_VisMaterial.cxx` | Graphic3d 系 `#include` を削除し、Graphic3d を使うメソッドのボディを空スタブに置換 |
| `src/XCAFPrs/XCAFPrs_Texture.cxx` | ファイル全体を空にする（`FillAspect` のスタブ化後は誰も呼ばない） |

スタブ化するメソッド（ヘッドレス STEP I/O では呼ばれない可視化専用メソッド）:
- `FillMaterialAspect()` — `Graphic3d_MaterialAspect` を埋める
- `FillAspect()` — `Graphic3d_Aspects` ハンドルを埋める、`XCAFPrs_Texture` を使う
- `ConvertToPbrMaterial()` — `Graphic3d_PBRMaterial::MetallicFromSpecular` 等を使う
- `ConvertToCommonMaterial()` — 同上

`GetID()`・`Restore()`・`Paste()`・`NewEmpty()` など `TDF_Attribute` の必須インターフェースは
**そのまま残す**。これらが定義されていないとリンクエラーになるため、空ファイルは不可。

### build.rs の実装

```rust
fn patch_occt_sources(source_dir: &Path) {
    patch_remove_includes_and_stub_methods(
        &source_dir.join("src/XCAFDoc/XCAFDoc_VisMaterial.cxx"),
        &["Graphic3d_Aspects.hxx", "Graphic3d_MaterialAspect.hxx", "XCAFPrs_Texture.hxx"],
        &[
            "XCAFDoc_VisMaterial::FillMaterialAspect",
            "XCAFDoc_VisMaterial::FillAspect",
            "XCAFDoc_VisMaterial::ConvertToPbrMaterial",
            "XCAFDoc_VisMaterial::ConvertToCommonMaterial",
        ],
    );

    // XCAFPrs_Texture.cxx は完全に空にする
    let texture_cxx = source_dir.join("src/XCAFPrs/XCAFPrs_Texture.cxx");
    if texture_cxx.exists() {
        std::fs::write(&texture_cxx, "// Stubbed: TKService not built\n").unwrap();
    }
}
```

`patch_remove_includes_and_stub_methods` は：
1. `#include` 行のうち指定パターンに一致するものを削除
2. 指定メソッドのボディ `{ ... }` をブレースカウントで検出し置換
   - `void` メソッド → `{}`
   - 戻り値あり → `{ return {}; }`（値初期化されたデフォルト値を返す）

パッチは `build_occt_from_source` 内でソース展開後・cmake ビルド前に毎回適用される。

### 最終的な build.rs の構成

```
occ_libs (常時): TKernel TKMath TKBRep TKTopAlgo TKPrim TKBO TKBool
                 TKShHealing TKMesh TKGeomBase TKGeomAlgo TKG3d TKG2d
                 TKBin TKXSBase TKDE TKDECascade TKDESTEP
                 ※ TKService は含まない

color feature 時のみ追加:
  OCCT libs: TKLCAF TKXCAF TKCAF TKCDF
             ※ TKService は含まない（ソースパッチで不要になった）
  Windows system libs: -lole32 -lwindowscodecs
             ※ color 時のみ（TKCAF 経由で Image_AlienPixMap が...
               → いや、ソースパッチにより TKService 自体不要なので
                  ole32/windowscodecs も不要）
```

> **注意**: ソースパッチにより `TKService` が完全に不要になったため、
> `ole32` / `windowscodecs` も color 時でもリンク不要となった。

---

## 結果

| 条件 | 結果 |
|---|---|
| `bundled` feature のみ（color なし） | ビルド成功、リンクエラーなし |
| `bundled,color` feature | ビルド成功、全 30 テスト通過 |

### テスト結果（`cargo test --features "bundled,color"`）

```
test result: ok. 21 passed  (基本テスト)
test result: ok.  3 passed  (color boolean テスト)
test result: ok.  5 passed  (STEP カラー roundtrip)
test result: ok.  1 passed  (XDE STEP)
```

---

## 補足: なぜ XCAFPrs_Texture.cxx は完全に空でよいか

`XCAFPrs_Texture` クラスは `Graphic3d_Texture2D`（TKService）を継承しており、
そのシンボルを定義するために TKService が必要になる。

`FillAspect()` をスタブ化（空ボディ）した後は、
`XCAFPrs_Texture` のインスタンスを生成するコードが `.obj` 内に存在しなくなる。
リンカは誰も参照しない `.obj` ファイルを静的ライブラリから引き込まないため、
`XCAFPrs_Texture.cxx.obj` 内の未解決シンボルは問題にならない。

## 補足: なぜ XCAFDoc_VisMaterial.cxx は完全に空にできないか

`TDF_Attribute` の仮想関数（`GetID`・`Restore`・`NewEmpty`・`Paste`）が未定義だと
リンカが `undefined reference` エラーを出す。これらは STEP 読み書き時に
TKXCAF が内部で呼び出す必須インターフェースであり、空実装にすることもできない。
（`GetID` が誤った GUID を返すとドキュメントのシリアライズが壊れる。）
