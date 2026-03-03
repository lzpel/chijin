# feature "color" Phase 6：色付き STEP/XDE 入出力の実装記録

作成: 2026-03-04

## 1. 目的

`docs/20260304-color_feature実装計画.md` の Phase 6（STEP XDE 色付き読み書き）を実装する。
Phase 1〜5（ColorMap・ブール演算色リレー・clean/translate/deep_copy）はすでに完了済み。

今回追加する API：

```rust
// STEP ファイルを XDE で読み込み、面ごとの色を ColorMap に格納する
Shape::read_step_colored(reader: &mut impl Read) -> Result<Shape, Error>

// Shape の ColorMap を STEP の STYLED_ITEM として書き出す
Shape::write_step_colored(&self, writer: &mut impl Write) -> Result<(), Error>
```

---

## 2. 変更対象ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `build.rs` | CMake `BUILD_MODULE_ApplicationFramework = ON`、XDE 静的ライブラリの追加リンク、Windows システムライブラリの追加 |
| `cpp/wrapper.h` | `read_step_colored_from_slice` / `write_step_colored_to_vec` 関数宣言の追加 |
| `cpp/wrapper.cpp` | XDE インクルード・ヘルパー関数・実装の追加 |
| `src/color_ffi.rs` | cxx ブリッジへの FFI 宣言追加 |
| `src/shape.rs` | `read_step_colored` / `write_step_colored` メソッドの追加 |
| `tests/color.rs` | `test_step_color_roundtrip` テストの追加 |

---

## 3. 各ファイルの変更詳細

### 3.1 `build.rs`

#### (a) CMake フラグ

```diff
- .define("BUILD_MODULE_ApplicationFramework", "OFF")
+ .define("BUILD_MODULE_ApplicationFramework", "ON")
```

XDE に必要な `TKLCAF`・`TKXCAF`・`TKCAF`・`TKCDF` を OCCT ビルドに含めるために必要。
`bundled` フィーチャーで OCCT をソースビルドする場合のみ使われるが、
`prebuilt` でもライブラリが存在すればリンクに成功する。
OCCT のリビルドは**フィーチャー `color` の有無に関係なく**同一の出力を生む（この設定は無条件 ON）。

#### (b) XDE 静的ライブラリの追加

```rust
if cfg!(feature = "color") {
    for lib in &["TKLCAF", "TKXCAF", "TKCAF", "TKCDF"] {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
}
```

`TKDESTEP`（`STEPCAFControl_Reader/Writer` を含む）は元から `OCC_LIBS` に含まれているため追加不要。

#### (c) Windows システムライブラリの追加

```rust
if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
    println!("cargo:rustc-link-arg=-ladvapi32");   // 既存
    println!("cargo:rustc-link-arg=-lole32");      // 追加
    println!("cargo:rustc-link-arg=-lwindowscodecs"); // 追加
}
```

`TKCAF` 追加により `TKService` の `Image_AlienPixMap.cxx.obj` が到達可能になり、
Windows Imaging Component (WIC) の COM API が必要になったため。

---

### 3.2 `cpp/wrapper.h`

`#ifdef CHIJIN_COLOR` ブロックに2関数を追加：

```cpp
/// STEP バイト列を XDE (STEPCAFControl_Reader) で読み込む。
/// 成功時、面の色を out_colors に書き込む。
std::unique_ptr<TopoDS_Shape> read_step_colored_from_slice(
    rust::Slice<const uint8_t> data, ColorMap& out_colors);

/// XDE (STEPCAFControl_Writer) で Shape を STEP バイト列に書き出す。
/// 失敗時は空 Vec を返す。
rust::Vec<uint8_t> write_step_colored_to_vec(
    const TopoDS_Shape& shape, const ColorMap& colors);
```

引数に `RustReader`/`RustWriter` ではなく `rust::Slice<uint8_t>` / `rust::Vec<uint8_t>` を使う理由は
[§4.2](#42-rustreaderwriter-のクロスブリッジ共有-失敗) を参照。

---

### 3.3 `cpp/wrapper.cpp`

#### インクルード（`#ifdef CHIJIN_COLOR`）

```cpp
#include <TDocStd_Document.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <Quantity_Color.hxx>
#include <TDF_LabelSequence.hxx>
```

`XCAFApp_Application.hxx` は意図的に除外している（[§4.3](#43-xcafapp_application-が-visualization-を引き込む) 参照）。

#### ヘルパー `extract_colors_from_label`

XDE のラベルツリーを再帰的に走査し、`XCAFDoc_ColorSurf` → `XCAFDoc_ColorGen` の順で色を取得する静的関数。

```cpp
static void extract_colors_from_label(
    const TDF_Label& label,
    const Handle(XCAFDoc_ShapeTool)& shape_tool,
    const Handle(XCAFDoc_ColorTool)& color_tool,
    ColorMap& out_colors)
{
    TopoDS_Shape shape = shape_tool->GetShape(label);
    if (!shape.IsNull()) {
        Quantity_Color color;
        bool found = color_tool->GetColor(label, XCAFDoc_ColorSurf, color);
        if (!found) found = color_tool->GetColor(label, XCAFDoc_ColorGen, color);
        if (found) {
            Standard_Real r, g, b;
            color.Values(r, g, b, Quantity_TOC_sRGB);
            out_colors.set(shape,
                static_cast<uint8_t>(std::lround(r * 255.0)),
                static_cast<uint8_t>(std::lround(g * 255.0)),
                static_cast<uint8_t>(std::lround(b * 255.0)));
        }
    }
    TDF_LabelSequence subshapes;
    shape_tool->GetSubShapes(label, subshapes);
    for (int i = 1; i <= subshapes.Length(); ++i)
        extract_colors_from_label(subshapes.Value(i), shape_tool, color_tool, out_colors);
}
```

#### `read_step_colored_from_slice`

1. `rust::Slice<uint8_t>` を `std::string` に変換して `std::istringstream` に包む
2. `new TDocStd_Document("XmlXCAF")` で XDE ドキュメントを生成
3. `STEPCAFControl_Reader` でストリームから読み込み、`Transfer` でドキュメントへ展開
4. `XCAFDoc_ShapeTool::GetFreeShapes` で自由形状ラベルを列挙し、`GetShape` で形状を取得
5. `extract_colors_from_label` で色情報を `out_colors` へ抽出
6. 複数形状は `BRep_Builder::Add` で Compound にまとめて返す

#### `write_step_colored_to_vec`

1. `new TDocStd_Document("XmlXCAF")` でドキュメント生成
2. `XCAFDoc_ShapeTool::NewShape` でルートラベルを作り、`SetShape` で形状を割り当て
3. `XCAFDoc_ShapeTool::AddSubShape` で各 Face のサブラベルを取得
4. `XCAFDoc_ColorTool::SetColor` で各 Face の色を `Quantity_TOC_sRGB` で設定
5. `STEPCAFControl_Writer::Transfer(doc)` でドキュメントを転写
6. `std::ostringstream` 経由で書き出し、バイト列を `rust::Vec<uint8_t>` で返す

---

### 3.4 `src/color_ffi.rs`

`unsafe extern "C++"` ブロックに追加：

```rust
fn read_step_colored_from_slice(
    data: &[u8],
    out_colors: Pin<&mut ColorMap>,
) -> UniquePtr<TopoDS_Shape>;

fn write_step_colored_to_vec(
    shape: &TopoDS_Shape,
    colors: &ColorMap,
) -> Vec<u8>;
```

---

### 3.5 `src/shape.rs`

`#[cfg(feature = "color")] impl Shape` ブロックに追加：

```rust
pub fn read_step_colored(reader: &mut impl Read) -> Result<Shape, Error> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data).map_err(|_| Error::StepReadFailed)?;
    let mut out_colors = crate::color_ffi::colormap_new();
    let inner = crate::color_ffi::read_step_colored_from_slice(
        data.as_slice(),
        out_colors.pin_mut(),
    );
    if inner.is_null() {
        return Err(Error::StepReadFailed);
    }
    Ok(shape_new!(inner, out_colors))
}

pub fn write_step_colored(&self, writer: &mut impl Write) -> Result<(), Error> {
    let bytes = crate::color_ffi::write_step_colored_to_vec(&self.inner, &self.colors);
    if bytes.is_empty() {
        return Err(Error::StepWriteFailed);
    }
    writer.write_all(&bytes).map_err(|_| Error::StepWriteFailed)
}
```

ストリームを Rust 側でまるごとバッファリングしてから C++ に渡すため、
`RustReader`/`RustWriter` を一切使わない設計とした。

---

### 3.6 `tests/color.rs`

追加テスト `test_step_color_roundtrip`：

- 6面ボックスの各面に異なる RGB 色を設定
- `write_step_colored` → `Vec<u8>` に書き出し
- `read_step_colored` で読み戻し
- 全 Face を走査し、元の6色がすべて `HashSet` に含まれていることを確認

---

## 4. 試行錯誤の記録

### 4.1 `TKXDESTEP` が存在しない（初回リンクエラー）

**仮説**: `STEPCAFControl_Reader/Writer` は `TKXDESTEP` という名前のライブラリにあるだろう。

**試みた設定**:
```rust
for lib in &["TKCAF", "TKXDESTEP"] { ... }
```

**結果**: `cannot find -lTKXDESTEP`。

**調査**: `target/occt/win64/gcc/lib/` に `libTKXDESTEP.a` が存在しない。
`nm libTKDESTEP.a | grep STEPCAFControl` を実行すると `STEPCAFControl` シンボルが見つかった。
OCCT 7.9.3 では `STEPCAFControl` は `TKDESTEP` に統合されており、
`TKDESTEP` はすでに `OCC_LIBS` に含まれていたため追加不要だった。

**修正**: `TKXDESTEP` を除去。実際に必要なライブラリを `nm` で確認しながら特定。

---

### 4.2 `RustReader`/`RustWriter` のクロスブリッジ共有（失敗）

**仮説**: `color_ffi.rs` の cxx ブリッジで、`ffi.rs` で定義済みの `RustReader` を型エイリアスとして再利用できる。

**試みた記述**:
```rust
// color_ffi.rs 内の extern "Rust" ブロック
extern "Rust" {
    type RustReader = crate::stream::RustReader;  // ← 型エイリアス
}
```

**結果**:
```
error: type alias in extern 'Rust' block is not supported
```

**原因**: `cxx` は `extern "Rust"` ブロック内で型エイリアスを一切サポートしない。
別の cxx ブリッジで宣言された型を再利用する手段がない。

**修正**: ストリーミングをやめ、バッファリング方式に変更。
- Rust 側で `reader.read_to_end(&mut data)` してから `&[u8]` を C++ に渡す
- C++ からの戻り値は `rust::Vec<uint8_t>` で受け取り、Rust 側で `writer.write_all` する

---

### 4.3 `XCAFApp_Application` が Visualization を引き込む

**仮説**: XDE ドキュメントは `XCAFApp_Application::GetApplication()->NewDocument(...)` で作成するのが標準的。

**試みたコード**:
```cpp
Handle(XCAFApp_Application) app = XCAFApp_Application::GetApplication();
Handle(TDocStd_Document) doc;
app->NewDocument("XmlXCAF", doc);
```

**結果**:
```
undefined reference to `XCAFPrs_Driver::...`
undefined reference to `TPrsStd_Driver::...`
undefined reference to `AIS_InteractiveContext::...`
```

**原因**: `XCAFApp_Application` のスタティックイニシャライザが `XCAFPrs_Driver` を登録し、
それが `TKVCAF` → `TKV3d`（3D Visualization）全体を引き込む。
Visualization モジュールを `BUILD_MODULE_Visualization = OFF` でビルドしているため未解決。

**修正**: `XCAFApp_Application` を一切使わず、`TDocStd_Document` を直接生成する。

```cpp
Handle(TDocStd_Document) doc = new TDocStd_Document("XmlXCAF");
```

`TDocStd_Document` のコンストラクタは自前で `TDF_Data` を生成するため、
アプリケーション層なしでも XDE の読み書きには十分。

---

### 4.4 `TNaming_NamedShape` が未解決（`TKCAF` 追加）

**エラー**:
```
undefined reference to `TNaming_NamedShape::Restore(...)'
undefined reference to `TNaming_Builder::TNaming_Builder(TDF_Label const&)'
```

**原因**: `TKXCAF` の `XCAFDoc.cxx.obj` / `XCAFDoc_Datum.cxx.obj` が
`TNaming_NamedShape`・`TNaming_Builder` を参照している。
これらは `TKCAF` にある（`nm libTKCAF.a | c++filt | grep TNaming_NamedShape` で確認）。

**修正**: `build.rs` に `TKCAF` を追加。

---

### 4.5 `CDM_Document` が未解決（`TKCDF` 追加）

**エラー**:
```
undefined reference to `CDM_Document::CDM_Document()'
undefined reference to `CDM_Document::IsOpened() const'
```

**原因**: `TKLCAF` の `TDocStd_Document.cxx.obj` が `CDM_Document`（基底クラス）を参照。
`CDM_Document` は `TKCDF` にある。

**修正**: `build.rs` に `TKCDF` を追加。

---

### 4.6 `Image_AlienPixMap` が WIC を参照（Windows システムライブラリ追加）

**エラー**:
```
undefined reference to `__imp_CoInitializeEx'
undefined reference to `__imp_CoCreateInstance'
undefined reference to `GUID_WICPixelFormat8bppIndexed'
undefined reference to `CLSID_WICImagingFactory'
```

**原因**: `TKService`（`OCC_LIBS` で既リンク）の `Image_AlienPixMap.cxx.obj` が
Windows Imaging Component (WIC) の COM API を使用している。
`TKCAF` 追加により `Image_AlienPixMap` が到達可能になり、リンカが解決を要求した。

必要なシステムライブラリ:
- `ole32` — `CoInitializeEx`, `CoCreateInstance`, `CreateStreamOnHGlobal` 等
- `windowscodecs` — `GUID_WICPixelFormat*`, `CLSID_WICImagingFactory` 等の GUID データ

MinGW では `libole32.a` / `libwindowscodecs.a` ともに利用可能（確認済み）。

**修正**: `build.rs` の Windows 用リンク引数に追加。

```rust
if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
    println!("cargo:rustc-link-arg=-ladvapi32");   // 既存
    println!("cargo:rustc-link-arg=-lole32");      // 追加
    println!("cargo:rustc-link-arg=-lwindowscodecs"); // 追加
}
```

---

## 5. 最終的なリンク依存グラフ（feature = "color"）

```
STEPCAFControl_Reader/Writer
    └── TKDESTEP (OCC_LIBS に既存)

TDocStd_Document
    └── TKLCAF
            └── CDM_Document → TKCDF

XCAFDoc_ColorTool, XCAFDoc_ShapeTool, XCAFDoc_DocumentTool
    └── TKXCAF
            └── TNaming_NamedShape, TNaming_Builder → TKCAF

Image_AlienPixMap (TKService, OCC_LIBS に既存)
    └── Windows:
            ├── CoInitializeEx, CoCreateInstance → ole32
            └── GUID_WICPixelFormat*, CLSID_WICImagingFactory → windowscodecs
```

---

## 6. 結果

```
test test_set_and_get_face_color    ... ok
test test_translated_preserves_colors ... ok
test test_clean_preserves_colors    ... ok
test test_step_color_roundtrip      ... ok   ← Phase 6
test test_boolean_preserves_colors  ... ok
test test_stretch_preserves_colors  ... ok

test result: ok. 6 passed; 0 failed
```

既存の integration テスト 21 件・stretch_box テスト 2 件も引き続き全パス。
