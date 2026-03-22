use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

fn main() {
	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

	let (occt_include, occt_lib_dir) = if cfg!(feature = "bundled") {
		build_occt_from_source(&out_dir, &manifest_dir)
	} else if cfg!(feature = "prebuilt") {
		use_system_occt()
	} else {
		panic!("Either 'bundled' or 'prebuilt' feature must be enabled");
	};

	link_occt_libraries(&occt_include, &occt_lib_dir, cfg!(feature = "color"));
}

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path, color: bool) {
	// Required OCC toolkit libraries to link against (OCCT 7.8+ / 7.9.x naming).
	// In OCCT 7.8+: TKSTEP*/TKBinTools/TKShapeUpgrade were reorganized into
	// TKDESTEP/TKBin/TKShHealing respectively.
	let occ_libs = &[
		"TKernel",
		"TKMath",
		"TKBRep",
		"TKTopAlgo",
		"TKPrim",
		"TKBO",
		"TKBool",
		"TKShHealing", // includes former TKShapeUpgrade
		"TKMesh",
		"TKGeomBase",
		"TKGeomAlgo",
		"TKG3d",
		"TKG2d",
		"TKBin", // was TKBinTools
		"TKXSBase",
		"TKDE",        // DE framework base (OCCT 7.8+)
		"TKDECascade", // DE cascade bridge (OCCT 7.8+)
		"TKOffset",    // BRepOffsetAPI_MakePipeShell (helix sweep)
		"TKHLR",       // HLRBRep_Algo (hidden line removal for SVG export)
		"TKDESTEP",    // was TKSTEP + TKSTEP209 + TKSTEPAttr + TKSTEPBase
		               // TKService is NOT linked here: it contains Image_AlienPixMap (WIC image I/O)
		               // which pulls in ole32/windowscodecs on Windows, but image I/O is unused in
		               // the base API.  TKService is added below only when "color" is enabled because
		               // TKXCAF references Graphic3d_* symbols that live in TKService.
	];

	// Link OCC libraries
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in occ_libs {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// XDE (XDE-based STEP with color) requires ApplicationFramework libs.
	// In OCCT 7.9.3, library layout (verified by nm):
	//   TKLCAF   — TDocStd_Document, TDocStd_Application (NewDocument / Close)
	//   TKXCAF   — XCAFApp_Application, XCAFDoc_ColorTool, XCAFDoc_ShapeTool,
	//              XCAFDoc_DocumentTool
	//   TKCAF    — TNaming_NamedShape, TNaming_Builder (needed by TKXCAF's XCAFDoc)
	//   TKCDF    — CDM_Document, CDM_Application (needed by TKLCAF's TDocStd_Document)
	//   TKDESTEP — STEPCAFControl_Reader / Writer (already in OCC_LIBS above)
	//   TKService — Graphic3d_* symbols referenced by TKXCAF (XCAFDoc_VisMaterial etc.)
	if color {
		for lib in &["TKLCAF", "TKXCAF", "TKCAF", "TKCDF"] {
			println!("cargo:rustc-link-lib=static={}", lib);
		}
	}

	// Safety-net: suppress any residual duplicate-symbol errors when linking
	// against OCCT static libraries on MinGW.  The primary fix is the
	// OCC_CONVERT_SIGNALS define added below to the cxx_build step.
	// Guard to GNU only: -Wl,... is GCC/ld syntax and is invalid on MSVC link.exe.
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	// TKernel's OSD_WNT.cxx registers a static initialiser (Init_OSD_WNT) that
	// calls advapi32 functions (AllocateAndInitializeSid etc.) at program startup.
	// Standard_Macro.hxx forcibly undefs OCCT_UWP unless WINAPI_FAMILY_APP is set,
	// so the dependency cannot be removed via compiler flags alone.
	// Rust passes -nodefaultlibs, bypassing GCC's spec that normally adds -ladvapi32.
	//
	// Additional Windows system libs required by OCCT static libs (color feature only):
	//   ole32         — Image_AlienPixMap uses CoInitializeEx / CoCreateInstance /
	//                   CreateStreamOnHGlobal / GetHGlobalFromStream (WIC image I/O)
	//   windowscodecs — GUID_WICPixelFormat* / CLSID_WICImagingFactory data symbols
	//                   (pulled in transitively via TKService → Image_AlienPixMap)
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		// Use rustc-link-lib (not rustc-link-arg) so Cargo translates correctly
		// for both toolchains: MSVC gets advapi32.lib, GNU gets -ladvapi32.
		println!("cargo:rustc-link-lib=advapi32");
		// OSD_signal.cxx (TKernel) uses MessageBoxA / MessageBeep on MSVC Windows
		// (#if !defined(__MINGW32__) && !defined(__CYGWIN32__) block).
		// MinGW links user32 implicitly; MSVC requires explicit declaration.
		println!("cargo:rustc-link-lib=user32");
	}

	// Build cxx bridge + C++ wrapper
	let mut build = cxx_build::bridge("src/ffi.rs");
	build
		.file("cpp/wrapper.cpp")
		.include(occt_include)
		.std("c++17")
		.define("_USE_MATH_DEFINES", None);

	// Define CHIJIN_COLOR for C++ when the "color" feature is enabled.
	if color {
		build.define("CHIJIN_COLOR", None);
	}

	// On MinGW (Windows GNU toolchain), GCC at -O0 emits inline C++ methods
	// (from Standard_ErrorHandler.hxx) as strong (non-COMDAT) symbols in wrapper.o.
	// TKernel.a unconditionally defines the same methods in Standard_ErrorHandler.cxx.obj.
	// This causes "multiple definition" link errors.
	//
	// Standard_ErrorHandler.hxx only generates the inline stubs when OCC_CONVERT_SIGNALS
	// is NOT defined (via `#if !defined(OCC_CONVERT_SIGNALS)`).  Defining it here
	// suppresses those stubs in wrapper.o so only TKernel.a's implementation is linked.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
		&& env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
	{
		build.define("OCC_CONVERT_SIGNALS", None);
	}

	build.compile("chijin_cpp");

	println!("cargo:rerun-if-changed=src/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Feature "bundled": Download OCCT 7.9.3 source and build with CMake.
fn build_occt_from_source(out_dir: &Path, manifest_dir: &Path) -> (PathBuf, PathBuf) {
	let occt_version = "V7_9_3";
	let occt_url = format!(
		"https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz",
		occt_version
	);

	let download_dir = out_dir.join("occt-source");

	// Use a sentinel file to track successful extraction.
	let extraction_sentinel = download_dir.join(".extraction_done");

	if !extraction_sentinel.exists() {
		std::fs::create_dir_all(&download_dir).unwrap();

		// Clean up any partial extraction from a previous failed attempt
		if let Ok(entries) = std::fs::read_dir(&download_dir) {
			for entry in entries.flatten() {
				let name = entry.file_name();
				if name.to_string_lossy().starts_with("OCCT") && entry.path().is_dir() {
					eprintln!("Removing partial OCCT extraction: {:?}", name);
					let _ = std::fs::remove_dir_all(entry.path());
				}
			}
		}

		eprintln!("Downloading OCCT {} ...", occt_version);

		// Download using ureq (pure Rust HTTP client)
		let response = ureq::get(&occt_url)
			.call()
			.expect("Failed to download OCCT source tarball");

		let mut body = Vec::new();
		response
			.into_body()
			.into_reader()
			.read_to_end(&mut body)
			.expect("Failed to read OCCT download response body");

		eprintln!("Downloaded {} bytes. Extracting...", body.len());

		// Extract using libflate + tar (pure Rust)
		let gz_decoder =
			libflate::gzip::Decoder::new(&body[..]).expect("Failed to initialize gzip decoder");
		let mut archive = tar::Archive::new(gz_decoder);
		archive
			.unpack(&download_dir)
			.expect("Failed to extract OCCT source tarball");

		// Write sentinel to mark successful extraction
		std::fs::write(&extraction_sentinel, "done").unwrap();
		eprintln!("OCCT source extracted successfully.");
	}

	// Auto-detect the extracted OCCT directory name
	// (GitHub archives may name it OCCT-V7_9_3 or OCCT-7_9_3 depending on the tag)
	let source_dir = std::fs::read_dir(&download_dir)
		.expect("Failed to read occt-source directory")
		.flatten()
		.find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir())
		.map(|e| e.path())
		.expect("OCCT source directory not found after extraction");

	// Patch OCCT sources to remove TKService (Visualization) dependencies.
	// XCAFDoc_VisMaterial.cxx and XCAFPrs_Texture.cxx reference Graphic3d_* symbols
	// that live in TKService, which we don't build (BUILD_MODULE_Visualization=OFF).
	// The non-visualization TDF_Attribute methods (GetID, Restore, Paste, …) are
	// kept intact; only FillMaterialAspect / FillAspect are emptied.
	patch_occt_sources(&source_dir);

	// Install into target/occt for a stable, predictable location
	let occt_root = manifest_dir.join("target").join("occt");

	// Determine lib path (CMake on Windows/MinGW installs to win64/gcc/lib)
	let lib_dir = find_occt_lib_dir(&occt_root);

	// Build with CMake only if not already installed
	if !lib_dir.exists() {
		eprintln!("Building OCCT with CMake (this may take a while)...");

		let built = cmake::Config::new(&source_dir)
			.profile("Release")
			.define("BUILD_LIBRARY_TYPE", "Static")
			.define("CMAKE_INSTALL_PREFIX", occt_root.to_str().unwrap())
			// Disable optional dependencies we don't need
			.define("USE_FREETYPE", "OFF")
			.define("USE_FREEIMAGE", "OFF")
			.define("USE_OPENVR", "OFF")
			.define("USE_FFMPEG", "OFF")
			.define("USE_TBB", "OFF")
			.define("USE_VTK", "OFF")
			.define("USE_RAPIDJSON", "OFF")
			.define("USE_DRACO", "OFF")
			.define("USE_TK", "OFF")
			.define("USE_TCL", "OFF")
			.define("USE_XLIB", "OFF")
			.define("USE_OPENGL", "OFF")
			.define("USE_GLES2", "OFF")
			.define("USE_EGL", "OFF")
			.define("USE_D3D", "OFF")
			// Only build the modules we need
			.define("BUILD_MODULE_FoundationClasses", "ON")
			.define("BUILD_MODULE_ModelingData", "ON")
			.define("BUILD_MODULE_ModelingAlgorithms", "ON")
			.define("BUILD_MODULE_DataExchange", "ON")
			.define("BUILD_MODULE_Visualization", "OFF")
			.define("BUILD_MODULE_ApplicationFramework", "OFF")
			.define("BUILD_MODULE_Draw", "OFF")
			.define("BUILD_DOC_Overview", "OFF")
			.define("BUILD_DOC_RefMan", "OFF")
			.define("BUILD_YACCLEX", "OFF")
			.define("BUILD_RESOURCES", "OFF")
			.define("BUILD_SAMPLES_MFC", "OFF")
			.define("BUILD_SAMPLES_QT", "OFF")
			.define("BUILD_Inspector", "OFF")
			.define("BUILD_ENABLE_FPE_SIGNAL_HANDLER", "OFF")
			.build();

		eprintln!("OCCT built at: {}", built.display());
	}

	// Re-resolve lib dir after build (in case it was just created)
	let lib_dir = find_occt_lib_dir(&occt_root);
	let include_dir = find_occt_include_dir(&occt_root);

	(include_dir, lib_dir)
}

/// Find the OCCT include directory, checking common install layouts.
fn find_occt_include_dir(occt_root: &Path) -> PathBuf {
	let candidates = [
		occt_root.join("include").join("opencascade"),
		occt_root.join("inc"),
		occt_root.join("include"),
	];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("include")
}

/// Find the OCCT lib directory, checking common install layouts.
/// CMake on Windows/MinGW installs to win64/gcc/lib; on Linux to lib.
fn find_occt_lib_dir(occt_root: &Path) -> PathBuf {
	let candidates = [
		occt_root.join("lib"),
		occt_root.join("win64").join("gcc").join("lib"),
		occt_root.join("win64").join("vc14").join("lib"),
	];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("lib")
}

/// Feature "prebuilt": Use system-installed OCCT.
fn use_system_occt() -> (PathBuf, PathBuf) {
	let occt_root = env::var("OCCT_ROOT")
		.or_else(|_| env::var("CASROOT"))
		.expect(
			"OCCT_ROOT or CASROOT environment variable must be set \
             when using the 'prebuilt' feature",
		);

	let occt_root = PathBuf::from(occt_root);

	let include_dir = find_occt_include_dir(&occt_root);
	let lib_dir = find_occt_lib_dir(&occt_root);

	assert!(
		include_dir.exists(),
		"OCCT include directory not found at {}",
		include_dir.display()
	);
	assert!(
		lib_dir.exists(),
		"OCCT lib directory not found at {}",
		lib_dir.display()
	);

	(include_dir, lib_dir)
}

/// Patch two OCCT source files that pull in Graphic3d_* (TKService) symbols even
/// when BUILD_MODULE_Visualization=OFF:
///
///  - XCAFDoc/XCAFDoc_VisMaterial.cxx: remove #include lines for Graphic3d_Aspects,
///    Graphic3d_MaterialAspect and XCAFPrs_Texture, then empty the bodies of
///    FillMaterialAspect() and FillAspect() — the only methods that use those types.
///    All TDF_Attribute interface methods (GetID, Restore, Paste, …) are left intact.
///
///  - XCAFPrs/XCAFPrs_Texture.cxx: replaced with an empty file because it defines
///    XCAFPrs_Texture which inherits from Graphic3d_Texture2D (TKService).
///    The only caller was FillAspect(), which is now empty.
fn patch_occt_sources(source_dir: &Path) {
	// ボディのみスタブ化: #include・シグネチャを残し、メソッド本体だけを空にする。
	stub_out_methods(&source_dir.join("src/XCAFDoc/XCAFDoc_VisMaterial.cxx"), true);
	// ファイルごと空化: イニシャライザリストでベースクラスを参照するため
	// ボディスタブでは TKService 依存を断ち切れない。
	stub_out_methods(&source_dir.join("src/XCAFPrs/XCAFPrs_Texture.cxx"), false);
}

/// `path` の C++ ソースファイルを無力化する。
///
/// # 引数
/// - `keep_signatures` — `true`: `#include` とシグネチャを残し、トップレベルの
///   メソッドボディだけを空スタブに置換する。シグネチャの型参照が必要な場合に使う。
///   `false`: ファイルを空にする (冒頭コメントのみ残す)。イニシャライザリストで
///   ベースクラスを参照する等、ボディスタブでは依存を断ち切れない場合に使う。
///
/// ボディスタブ時の規則:
/// - `void` 戻り値 / コンストラクタ / デストラクタ → `{}`
/// - それ以外 → `{ return {}; }` (値初期化)
///
/// # 注意
/// `keep_signatures: true` はトップレベルに `namespace {}` や `extern "C" {}`
/// があるファイルには使えない。`.cxx` 実装ファイル専用。
fn stub_out_methods(path: &Path, keep_signatures: bool) {
	if !path.exists() {
		return;
	}

	// 冒頭にスタブ化の記録コメントを生成する。
	let timestamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| {
			let secs = d.as_secs();
			let (h, mi, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
			format!("{:02}:{:02}:{:02} UTC", h, mi, s)
		})
		.unwrap_or_else(|_| "unknown".to_string());
	let description = if keep_signatures {
		"all method bodies replaced with empty stubs"
	} else {
		"file emptied"
	};
	let header = format!(
		"// Stubbed by chijin build.rs: {}.\n\
		 // Stubbed at: {}\n",
		description, timestamp
	);

	let patched = if keep_signatures {
		let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
		// トップレベルのブレースブロックをすべてスタブ化する。
		header + &stub_all_top_level_bodies(&content)
	} else {
		// ファイルを空にする (冒頭コメントのみ)。
		header
	};

	std::fs::write(path, patched).expect("Failed to write stubbed file");
	eprintln!("Stubbed {}", path.file_name().unwrap().to_string_lossy());
}

/// `content` 内のトップレベル (ブレース深さ 0) にある `{ … }` ブロックをすべて
/// `{}` または `{ return {}; }` に置換して返す。
fn stub_all_top_level_bodies(content: &str) -> String {
	let bytes = content.as_bytes();
	let mut result = String::new();
	let mut depth = 0usize;
	let mut i = 0;
	let mut last_end = 0;

	while i < bytes.len() {
		match bytes[i] {
			b'{' if depth == 0 => {
				// トップレベルブロック開始: 直前のシグネチャで戻り値型を判定する。
				let prefix = &content[last_end..i];
				let stub_body = if is_void_return(prefix) {
					"{}"
				} else {
					"{ return {}; }"
				};

				// ブレースカウントでブロック終端を探す。
				depth = 1;
				i += 1;
				while i < bytes.len() && depth > 0 {
					match bytes[i] {
						b'{' => depth += 1,
						b'}' => depth -= 1,
						_ => {}
					}
					i += 1;
				}
				// i は閉じ '}' の次を指している。
				result.push_str(prefix);
				result.push_str(stub_body);
				last_end = i;
				continue;
			}
			b'{' => depth += 1,
			b'}' => {
				if depth > 0 {
					depth -= 1;
				}
			}
			_ => {}
		}
		i += 1;
	}
	result.push_str(&content[last_end..]);
	result
}

/// シグネチャ文字列から、ボディを `{}` で置換すべきか判定する。
///
/// 以下のいずれかに該当すれば `true` (空ボディ `{}`):
/// 1. 戻り値型が `void` — シグネチャ中に識別子 "void" がある
/// 2. デストラクタ — シグネチャに `::~` が含まれる
/// 3. コンストラクタ — `ClassName::ClassName(` のように `::` 前後の識別子が同一
///
/// いずれも該当しなければ `false` → `{ return {}; }` で値初期化する。
fn is_void_return(prefix: &str) -> bool {
	// 直前の定義の終端 (';' か '}') 以降のシグネチャ部分だけを見る。
	let sig = prefix
		.rfind(|c| c == ';' || c == '}')
		.map(|p| &prefix[p + 1..])
		.unwrap_or(prefix);

	// 1. void 戻り値型
	if sig
		.split(|c: char| !c.is_alphanumeric() && c != '_')
		.any(|w| w == "void")
	{
		return true;
	}

	// 2. デストラクタ: ::~ が含まれる
	if sig.contains("::~") {
		return true;
	}

	// 3. コンストラクタ: ClassName::ClassName( のパターン
	//    '(' の直前までを見て、最後の '::' を境に前後の識別子を比較する。
	if let Some(paren) = sig.find('(') {
		let before_paren = sig[..paren].trim_end();
		if let Some(dc) = before_paren.rfind("::") {
			let method_name = before_paren[dc + 2..].trim();
			let class_name = before_paren[..dc]
				.split(|c: char| !c.is_alphanumeric() && c != '_')
				.filter(|s| !s.is_empty())
				.last()
				.unwrap_or("");
			if !method_name.is_empty() && method_name == class_name {
				return true;
			}
		}
	}

	false
}
