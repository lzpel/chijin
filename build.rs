mod build_delegation;

use std::env;
use std::path::{Path, PathBuf};

/// OCCT release used by cadrum. Update this tag when bumping OCCT versions;
/// `slug()` derives the lowercase/underscore-stripped form used in filenames.
const OCCT_VERSION: &str = "V8_0_0_rc5";

/// GitHub Release tag under `lzpel/cadrum` that hosts the prebuilt tarballs.
/// Bump this when rebuilding prebuilts for the same OCCT version.
const OCCT_PREBUILT_TAG: &str = "occt-v800rc5";

/// `V8_0_0_rc5` → `v800rc5`. Shared rule: lowercase and drop underscores.
fn slug(version: &str) -> String {
	version.to_ascii_lowercase().replace('_', "")
}

fn main() {
	println!("cargo:rerun-if-env-changed=OCCT_ROOT");
	println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
	println!("cargo:rerun-if-env-changed=CADRUM_PREBUILT_URL");
	println!("cargo:rerun-if-changed=src/traits.rs");
	println!("cargo:rerun-if-changed=build_delegation.rs");

	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	build_delegation::build_delegation(include_str!("src/traits.rs"), &out_dir);

	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
	let target = env::var("TARGET").unwrap();

	let (occt_include, occt_lib_dir) = resolve_occt(&manifest_dir, &out_dir, &target);

	link_occt_libraries(&occt_include, &occt_lib_dir);
}

/// Resolve (include_dir, lib_dir) for OCCT.
///
/// Model: `OCCT_ROOT` is the single cache location. If unset, it defaults to
/// `target/cadrum-occt-<slug>-<target>/`. The `source-build` feature only
/// changes how a cache miss is populated; it does nothing on a cache hit.
///
///   1. Compute `effective_root = OCCT_ROOT or default`
///   2. If `effective_root` already has both lib and include → use it
///   3. Otherwise, populate it:
///        - `source-build` feature ON  → build from upstream OCCT sources
///        - `source-build` feature OFF → download prebuilt tarball (panic on failure)
fn resolve_occt(manifest_dir: &Path, out_dir: &Path, target: &str) -> (PathBuf, PathBuf) {
	// Default cache dir lives alongside other cargo build artifacts. Honor
	// `CARGO_TARGET_DIR` if set so that isolated-per-container builds (docker)
	// don't all collide on the same OCCT install path.
	let target_dir: PathBuf = env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| manifest_dir.join("target"));
	let default_root = target_dir.join(format!("cadrum-occt-{}-{}", slug(OCCT_VERSION), target));
	let effective_root: PathBuf = env::var("OCCT_ROOT").map(PathBuf::from).unwrap_or_else(|_| default_root.clone());

	println!("cargo:rerun-if-changed={}", effective_root.display());

	let [include_dir, lib_dir] = find_occt_dirs(&effective_root);
	if lib_dir.exists() && include_dir.exists() {
		return (include_dir, lib_dir);
	}

	// Cache miss: populate effective_root.
	if cfg!(feature = "source-build") {
		eprintln!("cargo:warning=OCCT cache miss at {} — building from source (this may take 10-30 minutes)", effective_root.display());
		return build_occt_from_source(out_dir, &effective_root);
	}

	match try_prebuilt(out_dir, &effective_root, target) {
		Some(pair) => pair,
		None => panic!(
			"\nFailed to download prebuilt OCCT for target `{}`.\n\
			 A prebuilt tarball for this target may not be published yet.\n\
			 See README for the list of supported prebuilt targets, or enable\n\
			 the `source-build` feature to build OCCT from upstream sources:\n\
			 \n    cargo build --features source-build\n",
			target
		),
	}
}

/// Attempt to download and extract a prebuilt OCCT tarball for `target` into `dest`.
/// Returns None on any failure; the caller decides whether to panic or fall back.
fn try_prebuilt(out_dir: &Path, dest: &Path, target: &str) -> Option<(PathBuf, PathBuf)> {
	let slug_ver = slug(OCCT_VERSION);
	let top_name = format!("cadrum-occt-{}-{}", slug_ver, target);
	let tarball_name = format!("{}.tar.gz", top_name);
	let url = env::var("CADRUM_PREBUILT_URL").unwrap_or_else(|_| format!("https://github.com/lzpel/cadrum/releases/download/{}/{}", OCCT_PREBUILT_TAG, tarball_name));

	eprintln!("cargo:warning=Downloading prebuilt OCCT from {}", url);

	// Extract into a staging directory inside OUT_DIR, then move the tarball's
	// top-level `<top_name>/` into `dest`. Staging decouples the tarball's
	// layout from the (possibly user-chosen) `OCCT_ROOT` path.
	let staging = out_dir.join("occt-prebuilt-staging");
	let _ = std::fs::remove_dir_all(&staging);
	std::fs::create_dir_all(&staging).ok()?;

	if let Err(e) = download_and_extract_tar_gz(&url, &staging) {
		eprintln!("cargo:warning=prebuilt fetch failed: {}", e);
		return None;
	}

	let extracted = staging.join(&top_name);
	if !extracted.is_dir() {
		eprintln!("cargo:warning=prebuilt tarball missing expected top-level dir `{}`", top_name);
		return None;
	}

	if let Some(parent) = dest.parent() {
		std::fs::create_dir_all(parent).ok()?;
	}
	let _ = std::fs::remove_dir_all(dest);
	if let Err(e) = std::fs::rename(&extracted, dest) {
		eprintln!("cargo:warning=failed to move extracted OCCT into {}: {}", dest.display(), e);
		return None;
	}

	let [include_dir, lib_dir] = find_occt_dirs(dest);
	if !lib_dir.exists() {
		eprintln!("cargo:warning=prebuilt extraction did not produce expected lib dir at {}", lib_dir.display());
		return None;
	}
	Some((include_dir, lib_dir))
}

/// Download `url` (a `.tar.gz`), gunzip, and untar into `dest`.
/// `dest` must already exist.
fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<(), String> {
	let bytes = fetch_bytes(url)?;
	let gz = libflate::gzip::Decoder::new(&bytes[..]).map_err(|e| format!("gzip decode failed: {e}"))?;
	tar::Archive::new(gz).unpack(dest).map_err(|e| format!("tar unpack failed: {e}"))?;
	Ok(())
}

/// Fetch a URL into a byte vector. Supports `http(s)://` via minreq and
/// `file://` via the local filesystem (used by CI smoke tests).
fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
	if let Some(rest) = url.strip_prefix("file://") {
		// Handle both POSIX (`file:///tmp/x`) and Windows (`file:///C:/x`) forms.
		let path: PathBuf = if rest.len() >= 3 && rest.starts_with('/') && rest.as_bytes()[2] == b':' {
			PathBuf::from(&rest[1..])
		} else {
			PathBuf::from(rest)
		};
		std::fs::read(&path).map_err(|e| format!("read {}: {}", path.display(), e))
	} else {
		let resp = minreq::get(url).send().map_err(|e| e.to_string())?;
		Ok(resp.into_bytes())
	}
}

/// OCCT toolkits to link against (OCCT 7.8+ / 8.x naming). In 7.8+,
/// TKSTEP*/TKBinTools/TKShapeUpgrade were reorganized into TKDESTEP/TKBin/
/// TKShHealing. TKService is intentionally excluded — it pulls
/// Image_AlienPixMap → ole32/windowscodecs on Windows and image I/O is unused.
///
/// The `color`-gated XDE (STEP-with-color) ApplicationFramework toolkits
/// reference Graphic3d_* symbols that normally live in TKService; those
/// references are stubbed out by `patch_occt_sources`. Layout verified by nm:
///   TKLCAF — TDocStd_Document/Application
///   TKXCAF — XCAFApp_Application, XCAFDoc_ColorTool/ShapeTool/DocumentTool
///   TKCAF  — TNaming_NamedShape/Builder (needed by TKXCAF's XCAFDoc)
///   TKCDF  — CDM_Document/Application (needed by TKLCAF's TDocStd_Document)
const OCC_LIBS: &[&str] = &[
	"TKernel", "TKMath", "TKBRep", "TKTopAlgo", "TKPrim", "TKBO", "TKBool",
	"TKShHealing", "TKMesh", "TKGeomBase", "TKGeomAlgo", "TKG3d", "TKG2d",
	"TKBin", "TKXSBase", "TKDE", "TKDECascade", "TKOffset", "TKDESTEP",
	#[cfg(feature = "color")] "TKLCAF",
	#[cfg(feature = "color")] "TKXCAF",
	#[cfg(feature = "color")] "TKCAF",
	#[cfg(feature = "color")] "TKCDF",
];

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path) {
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in OCC_LIBS {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// Safety-net: suppress any residual duplicate-symbol errors when linking
	// against OCCT static libraries on MinGW.  The primary fix is the
	// OCC_CONVERT_SIGNALS define added below to the cxx_build step.
	// Guard to GNU only: -Wl,... is GCC/ld syntax and is invalid on MSVC link.exe.
	let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
	let is_mingw_like = target_env == "gnu" || target_env == "gnullvm";
	if is_mingw_like {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	// windows-gnu: absorb libgcc / libstdc++ / libwinpthread statically so
	// the final exe's only runtime dep is msvcrt.dll (OS-bundled on every
	// Windows since NT4.0). Safe because wrapper.cpp exposes only a C ABI
	// via cxx — no libstdc++ types cross the boundary, so downstream's
	// libstdc++ version cannot conflict with the one frozen inside our
	// objects.
	//
	// `-static` as a link-arg covers libgcc and libwinpthread cleanly: gcc
	// driver rewrites `-lgcc`/`-lwinpthread` to their static .a variants.
	// libstdc++ is NOT absorbed by this flag alone — rustc hardcodes
	// `-Wl,-Bdynamic` before the native-library block and link-cplusplus
	// emits plain `-lstdc++` there, so ld resolves it against
	// libstdc++.dll.a regardless of a trailing `-static`. Fully absorbing
	// libstdc++ additionally requires the build environment to set
	//   CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++
	//   RUSTFLAGS=-L <dir containing libstdc++.a>
	// The first flips link-cplusplus to static-mode emission; the second
	// satisfies rustc's compile-time check on link-cplusplus itself (which
	// runs long before this build.rs, so a cargo:rustc-link-search from
	// here would arrive too late). `docker/Dockerfile_x86_64-pc-windows-gnu`
	// does both for the prebuilt Docker build; downstream consumers on
	// windows-gnu need to replicate them (see README).
	//
	// Gated to windows+gnu because `-static` on linux-gnu would try to
	// statically link glibc, which is neither shipped as a .a nor desired.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && is_mingw_like {
		println!("cargo:rustc-link-arg=-static");
	}



	// advapi32 / user32: no longer needed — patch_occt_sources() stubs the OSD
	// files (OSD_WNT, OSD_File, OSD_Protection, OSD_signal) that reference them.

	// Build cxx bridge + C++ wrapper
	let mut build = cxx_build::bridge("src/occt/ffi.rs");
	build.file("cpp/wrapper.cpp").include(occt_include).std("c++17").define("_USE_MATH_DEFINES", None);

	// wrapper.cpp は UTF-8 (日本語コメント含む)。MSVC は既定でシステム既定コードページ
	// (日本語環境なら CP932) で読むため、マルチバイトの末尾バイトが `\` などと解釈されて
	// 行が結合され、パースがずれる (例: `const int n = ...;` が消えて `n` undeclared)。
	// `/utf-8` を付けてソース/実行文字集合を UTF-8 に固定する。
	if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
		build.flag("/utf-8");
	}

	// Define CADRUM_COLOR for C++ when the "color" feature is enabled.
	#[cfg(feature = "color")]
	build.define("CADRUM_COLOR", None);
	
	build.compile("cadrum_cpp");

	println!("cargo:rerun-if-changed=src/occt/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Download OCCT source, patch, and build with CMake into `install_prefix`.
fn build_occt_from_source(out_dir: &Path, install_prefix: &Path) -> (PathBuf, PathBuf) {
	let occt_version = OCCT_VERSION;
	let occt_url = format!("https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz", occt_version);

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

		eprintln!("Downloading OCCT {} from {} ...", occt_version, occt_url);
		download_and_extract_tar_gz(&occt_url, &download_dir).expect("Failed to download/extract OCCT source tarball");

		// Write sentinel to mark successful extraction
		std::fs::write(&extraction_sentinel, "done").unwrap();
		eprintln!("OCCT source extracted successfully.");
	}

	// Auto-detect the extracted OCCT directory name
	// (GitHub archives name it OCCT-{tag}, e.g. OCCT-V8_0_0_rc5)
	let source_dir = std::fs::read_dir(&download_dir).expect("Failed to read occt-source directory").flatten().find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir()).map(|e| e.path()).expect("OCCT source directory not found after extraction");

	// Patch OCCT sources to remove TKService (Visualization) dependencies.
	// XCAFDoc_VisMaterial.cxx and XCAFPrs_Texture.cxx reference Graphic3d_* symbols
	// that live in TKService, which we don't build (BUILD_MODULE_Visualization=OFF).
	// The non-visualization TDF_Attribute methods (GetID, Restore, Paste, …) are
	// kept intact; only FillMaterialAspect / FillAspect are emptied.
	patch_occt_sources(&source_dir);

	let occt_root = install_prefix;

	// Determine lib path (CMake on Windows/MinGW installs to win64/gcc/lib)
	let [_, lib_dir] = find_occt_dirs(&occt_root);

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
			// llvm-rc (cargo-xwin MSVC path) defaults to codepage 0 (ASCII only)
			// and rejects any non-ASCII byte in narrow string literals. OCCT's
			// .rc files are cp1252-encoded (the © character arrives as a single
			// 0xA9 byte, per the error's "codepoint (169)"), so tell llvm-rc to
			// interpret narrow strings as cp1252.
			//
			// We pass this via CMAKE_RC_FLAGS_INIT, NOT CMAKE_RC_FLAGS, because
			// cargo-xwin's override.cmake contains this line:
			//   string(REPLACE "/D" "-D" CMAKE_RC_FLAGS "${CMAKE_RC_FLAGS_INIT}")
			// which unconditionally overwrites whatever we set in CMAKE_RC_FLAGS.
			// CMAKE_RC_FLAGS_INIT survives the overwrite and flows through the
			// REPLACE into CMAKE_RC_FLAGS. Harmless on Unix targets (no RC step).
			.define("CMAKE_RC_FLAGS_INIT", "-C 1252")
			.build();

		eprintln!("OCCT built at: {}", built.display());
	}

	// Re-resolve dirs after build (in case they were just created)
	let [include_dir, lib_dir] = find_occt_dirs(&occt_root);

	(include_dir, lib_dir)
}

/// Returns `[include_dir, lib_dir]`. Each entry is the first existing
/// candidate, or the first candidate as fallback. Handles Linux
/// (`include`,`lib`), MinGW-gcc (`inc`,`win64/gcc/lib`), llvm-mingw
/// (`win64/clang/lib`), and MSVC (`win64/vc14/lib`) install layouts.
fn find_occt_dirs(occt_root: &Path) -> [PathBuf; 2] {
	let pick = |cands: &[PathBuf]| cands.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| cands[0].clone());
	[
		pick(&[occt_root.join("include").join("opencascade"), occt_root.join("inc"), occt_root.join("include")]),
		pick(&[occt_root.join("lib"), occt_root.join("win64").join("gcc").join("lib"), occt_root.join("win64").join("clang").join("lib"), occt_root.join("win64").join("vc14").join("lib")]),
	]
}

/// Patch OCCT source files to work around unwanted link dependencies and
/// platform-specific toolchain quirks:
///
/// 1. TKService (Visualization) — even with BUILD_MODULE_Visualization=OFF:
///    - XCAFDoc_VisMaterial.cxx: stub bodies that use Graphic3d_* types.
///    - XCAFPrs_Texture.cxx: empty entirely (inherits Graphic3d_Texture2D).
///
/// 2. advapi32 / user32 (Windows system libs) — TKernel's OSD package:
///    - OSD_WNT.cxx: empty entirely (static initialiser calls AllocateAndInitializeSid).
///    - OSD_File.cxx: stub bodies (OpenProcessToken, SetSecurityDescriptorDacl, etc.).
///    - OSD_Protection.cxx: stub bodies (EqualSid, LookupAccountNameW, etc.).
///    - OSD_signal.cxx: stub bodies (MessageBoxA / MessageBeep on MSVC).
///    - OSD_FileNode.cxx: stub bodies (SetFileSecurityW + OSD_WNT helpers).
///    - OSD_Process.cxx: stub bodies (OpenProcessToken, GetUserNameW, EqualSid).
///
/// 3. glibc-only headers (musl target):
///    - Standard_StackTrace.cxx: stub bodies (backtrace, backtrace_symbols)
///      and comment out `<execinfo.h>` which musl does not ship.
///
/// Note on the llvm-rc non-ASCII issue (cargo-xwin MSVC target): this is
/// NOT patched here. It is handled at the CMake layer by passing
/// `CMAKE_RC_FLAGS=-C 1252` so llvm-rc interprets narrow RC string literals
/// as cp1252, accepting the whole range of Latin-1 characters instead of
/// rejecting any byte above 0x7F.
fn patch_occt_sources(source_dir: &Path) {
	// OSD stubs are Windows-only. On Linux the same files compile to real
	// POSIX implementations via `#ifdef _WIN32`; stubbing them on Linux turns
	// non-void bodies into UB (`{}` with no return) and crashes at runtime
	// the moment OCCT enters `OSD_Process::SystemDate`, `OSD::SignalMode`, etc.
	let is_windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

	for entry in [source_dir.join("src"), source_dir.join("adm")]
		.into_iter()
		.flat_map(walkdir::WalkDir::new)
		.flatten()
	{
		if !entry.file_type().is_file() {
			continue;
		}
		let path = entry.path();
		let Some(name) = path.file_name().and_then(|s| s.to_str()) else { continue };
		match name {
			// TKService (Visualization) cut: stub bodies only, keep signatures.
			"XCAFDoc_VisMaterial.cxx" => stub_out_methods(path, true),
			// Initializer list references the base class — body stubs alone
			// cannot cut the dependency, so empty the whole file.
			"XCAFPrs_Texture.cxx" => stub_out_methods(path, false),

			// musl: <execinfo.h> is a glibc extension. Stub backtrace() bodies
			// then comment out the include so the preprocessor stops looking.
			"Standard_StackTrace.cxx" => {
				stub_out_methods(path, true);
				comment_out_include(path, "execinfo.h");
			}

			// Windows OSD: cut advapi32 / user32 symbol references.
			// OSD_WNT.cxx has a static init calling AllocateAndInitializeSid —
			// must be emptied wholesale, not body-stubbed.
			"OSD_WNT.cxx" if is_windows => stub_out_methods(path, false),
			"OSD_File.cxx"
			| "OSD_Protection.cxx"
			| "OSD_signal.cxx"
			| "OSD_FileNode.cxx"
			| "OSD_Process.cxx"
				if is_windows =>
			{
				stub_out_methods(path, true);
			}

			// Note on `OCC_CONVERT_SIGNALS`: OCCT's `adm/cmake/occt_defs_flags.cmake`
			// auto-defines this on every non-MSVC build, which enables OCCT's
			// `OCC_CATCH_SIGNALS` macro to emit `setjmp()` calls that convert C
			// signals into C++ exceptions. For mingw-w64 that path emits calls to
			// the 2-arg SEH `_setjmp`, whose libmingwex export name varies across
			// mingw-w64 versions — the resulting prebuilt .a fails to link on
			// downstream users who have a different mingw-w64 than we built with.
			"occt_defs_flags.cmake" if is_windows => {
				let needle = "add_definitions(-DOCC_CONVERT_SIGNALS)";
				let replacement = "# add_definitions(-DOCC_CONVERT_SIGNALS)  # patched out by cadrum build.rs";
				if let Ok(content) = std::fs::read_to_string(path) {
					if content.contains(needle) && !content.contains(replacement) {
						let patched = content.replace(needle, replacement);
						if let Err(e) = std::fs::write(path, patched) {
							eprintln!("warning: failed to patch {}: {}", path.display(), e);
						} else {
							eprintln!("patched out OCC_CONVERT_SIGNALS in {}", path.display());
						}
					}
				}
			}

			_ => {}
		}
	}
}

/// Comment out a `#include <name>` directive in `path`. Used to sever the
/// dependency on platform-specific headers (e.g. `execinfo.h` on musl) after
/// the referring method bodies have been stubbed out.
fn comment_out_include(path: &Path, header: &str) {
	if !path.exists() {
		return;
	}
	let content = std::fs::read_to_string(path).expect("Failed to read file for include patching");
	let needle = format!("#include <{}>", header);
	if !content.contains(&needle) {
		return;
	}
	let replacement = format!("// {} (patched out by cadrum build.rs)", needle);
	let patched = content.replace(&needle, &replacement);
	std::fs::write(path, patched).expect("Failed to write patched include file");
	eprintln!("Patched out <{}> in {}", header, path.file_name().unwrap().to_string_lossy());
}

/// Neutralize a C++ source file at `path`.
///
/// # Arguments
/// - `keep_signatures` — `true`: keep `#include`s and signatures, replace only the
///   top-level method bodies with empty stubs. Use when the signature types are still
///   needed by the compiler.
///   `false`: empty the entire file (header comment only). Use when the initializer
///   list references a base class and body stubs alone cannot cut the dependency.
///
/// Stub body rules:
/// - `void` return / constructor / destructor → `{}`
/// - anything else → `{ return {}; }` (value-initialize)
///
/// # Note
/// `keep_signatures: true` cannot be used on files that have top-level
/// `namespace {}` or `extern "C" {}` blocks. Intended for `.cxx` implementation files only.
fn stub_out_methods(path: &Path, keep_signatures: bool) {
	if !path.exists() {
		return;
	}

	// Header line records the stub op + unix timestamp so old/new stubs are
	// distinguishable when inspecting a cached source tree.
	let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_else(|_| "unknown".to_string());
	let description = if keep_signatures { "method bodies stubbed" } else { "file emptied" };
	let header = format!("// Stubbed by cadrum build.rs at unix={unix}: {description}.\n");

	let patched = if keep_signatures {
		let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
		// Replace all top-level brace blocks with empty stubs.
		header + &stub_all_top_level_bodies(&content)
	} else {
		// Empty the file (header comment only).
		header
	};

	std::fs::write(path, patched).expect("Failed to write stubbed file");
	eprintln!("Stubbed {}", path.file_name().unwrap().to_string_lossy());
}

/// Lexically normalise `content` so the brace-depth scanner sees a clean
/// view: comments, string/char literals, and preprocessor directives are
/// replaced with same-length whitespace. Newlines are preserved so line
/// numbers (and downstream offsets) stay aligned with the original file.
///
/// The returned string has the same byte length as the input, which means
/// byte offsets computed on the normalised view can be used to slice the
/// original content verbatim.
fn lex_normalize(content: &str) -> String {
	let bytes = content.as_bytes();
	let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
	let mut i = 0;
	let mut at_line_start = true;

	let push_blank = |out: &mut Vec<u8>, b: u8| {
		out.push(if b == b'\n' { b'\n' } else { b' ' });
	};

	while i < bytes.len() {
		let c = bytes[i];

		// Line comment `// ... \n`
		if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
			while i < bytes.len() && bytes[i] != b'\n' {
				out.push(b' ');
				i += 1;
			}
			continue;
		}
		// Block comment `/* ... */`
		if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
			out.push(b' ');
			out.push(b' ');
			i += 2;
			while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
				push_blank(&mut out, bytes[i]);
				i += 1;
			}
			if i + 1 < bytes.len() {
				out.push(b' ');
				out.push(b' ');
				i += 2;
			} else {
				while i < bytes.len() {
					push_blank(&mut out, bytes[i]);
					i += 1;
				}
			}
			continue;
		}
		// String literal `"..."`
		if c == b'"' {
			out.push(b' ');
			i += 1;
			while i < bytes.len() && bytes[i] != b'"' {
				if bytes[i] == b'\\' && i + 1 < bytes.len() {
					out.push(b' ');
					push_blank(&mut out, bytes[i + 1]);
					i += 2;
				} else {
					push_blank(&mut out, bytes[i]);
					i += 1;
				}
			}
			if i < bytes.len() {
				out.push(b' ');
				i += 1;
			}
			continue;
		}
		// Char literal `'...'`
		if c == b'\'' {
			out.push(b' ');
			i += 1;
			while i < bytes.len() && bytes[i] != b'\'' {
				if bytes[i] == b'\\' && i + 1 < bytes.len() {
					out.push(b' ');
					out.push(b' ');
					i += 2;
				} else {
					out.push(b' ');
					i += 1;
				}
			}
			if i < bytes.len() {
				out.push(b' ');
				i += 1;
			}
			continue;
		}
		// Preprocessor directive `# ... \n` (honoring `\`-line-continuation)
		if at_line_start && c == b'#' {
			while i < bytes.len() {
				if bytes[i] == b'\n' {
					// Check for `\`-continuation: preceding non-space char.
					let mut k = i;
					while k > 0 && (bytes[k - 1] == b' ' || bytes[k - 1] == b'\t') {
						k -= 1;
					}
					let continued = k > 0 && bytes[k - 1] == b'\\';
					out.push(b'\n');
					i += 1;
					if !continued {
						break;
					}
				} else {
					out.push(b' ');
					i += 1;
				}
			}
			at_line_start = true;
			continue;
		}

		if c == b'\n' {
			at_line_start = true;
		} else if !c.is_ascii_whitespace() {
			at_line_start = false;
		}
		out.push(c);
		i += 1;
	}

	debug_assert_eq!(out.len(), bytes.len(), "lex_normalize must preserve byte length");
	String::from_utf8(out).expect("lex_normalize produced invalid utf-8")
}

/// Choose the stub body for a function/method signature `sig`, which is the
/// text from the previous statement terminator up to (but not including) the
/// opening `{`. `sig` is expected to already be lexically normalised via
/// `lex_normalize`, so comments, string literals, and preprocessor lines
/// are whitespace.
///
/// Returns `"{}"` for void returns, constructors, and destructors; returns
/// `"{ return {}; }"` otherwise so MSVC does not emit C4716.
fn stub_body_for_sig(sig: &str) -> &'static str {
	// Normalise `A :: B` → `A::B` so walk-back treats qualified ids as one
	// token. This is a semantic concern that survives lex_normalize.
	let sig_norm: String = {
		let mut s = sig.to_string();
		loop {
			let next = s.replace(" ::", "::").replace(":: ", "::");
			if next == s {
				break s;
			}
			s = next;
		}
	};

	// Find the `(` that belongs to the target function's parameter list,
	// skipping macro invocations like `IMPLEMENT_STANDARD_RTTIEXT(...)`.
	// Heuristic: if the identifier immediately before a `(` is entirely
	// uppercase (macro convention), walk past its matching `)` and keep
	// searching.
	let paren_pos = {
		let bytes = sig_norm.as_bytes();
		let mut cursor = 0;
		loop {
			let Some(off) = sig_norm[cursor..].find('(') else { return "{}"; };
			let pos = cursor + off;
			let before = sig_norm[..pos].trim_end();
			let id_start = before
				.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
				.map(|p| p + 1)
				.unwrap_or(0);
			let ident = &before[id_start..];
			let is_macro = !ident.is_empty()
				&& ident.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
				&& ident.chars().any(|c| c.is_ascii_uppercase());
			if !is_macro {
				break pos;
			}
			let mut depth = 1;
			let mut j = pos + 1;
			while j < bytes.len() && depth > 0 {
				match bytes[j] {
					b'(' => depth += 1,
					b')' => depth -= 1,
					_ => {}
				}
				j += 1;
			}
			cursor = j;
		}
	};
	let head_full = sig_norm[..paren_pos].trim();
	let head = head_full.rsplit('\n').next().unwrap_or(head_full).trim();
	if head.is_empty() {
		return "{}";
	}

	// Walk back from the end collecting the trailing qualified-id (function
	// name, possibly `Class::method` or `~Class`).
	let hb = head.as_bytes();
	let mut start = hb.len();
	while start > 0 {
		let c = hb[start - 1];
		if c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'~' {
			start -= 1;
		} else {
			break;
		}
	}
	let name = &head[start..];
	let return_part = head[..start].trim();

	// Destructor: `~Foo` or `Foo::~Foo`.
	if name.contains('~') {
		return "{}";
	}
	// Constructor: last two `::`-segments are identical (`Foo::Foo`), or the
	// name has no return-type prefix at all.
	let segs: Vec<&str> = name.split("::").collect();
	if segs.len() >= 2 && segs[segs.len() - 1] == segs[segs.len() - 2] {
		return "{}";
	}
	if return_part.is_empty() {
		return "{}";
	}

	// Look for `void` as a whole word in the return-type portion, and make
	// sure it is not `void*` / `void&`.
	let rb = return_part.as_bytes();
	let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
	let mut idx = 0;
	while let Some(off) = return_part[idx..].find("void") {
		let pos = idx + off;
		let end = pos + 4;
		let before_ok = pos == 0 || !is_ident(rb[pos - 1]);
		let after_ok = end >= rb.len() || !is_ident(rb[end]);
		if before_ok && after_ok {
			let mut j = end;
			while j < rb.len() && rb[j].is_ascii_whitespace() {
				j += 1;
			}
			if j >= rb.len() || (rb[j] != b'*' && rb[j] != b'&') {
				return "{}";
			}
		}
		idx = end;
	}

	"{ return {}; }"
}

/// Replace every top-level (brace depth 0) function body in `content` with
/// `{}` or `{ return {}; }` and return the result.
///
/// Walks a lexically normalised view of `content` so that comments, string
/// literals, and preprocessor directives cannot confuse the brace/sig
/// scanner. Because `lex_normalize` preserves byte offsets, slices computed
/// on the normalised view map one-to-one onto the original content, which
/// is what we write out verbatim outside the stubbed bodies.
///
/// Non-function brace blocks (class/struct/namespace definitions, aggregate
/// initialisers) are detected by checking whether the end of the preceding
/// signature — after stripping trailing function qualifiers — is `)`.
fn stub_all_top_level_bodies(content: &str) -> String {
	let normalized = lex_normalize(content);
	let nb = normalized.as_bytes();
	let mut result = String::new();
	let mut depth = 0usize;
	let mut i = 0;
	let mut last_end = 0;

	while i < nb.len() {
		match nb[i] {
			b'{' if depth == 0 => {
				let brace_pos = i;
				let prefix_norm = &normalized[last_end..brace_pos];
				let sig = prefix_norm
					.rfind(|c| c == ';' || c == '}')
					.map(|p| &prefix_norm[p + 1..])
					.unwrap_or(prefix_norm);

				let trimmed = sig.trim_end();
				let last_line = trimmed.rsplit('\n').next().unwrap_or(trimmed).trim();
				let is_function = {
					let mut t = last_line;
					loop {
						let prev_len = t.len();
						for kw in ["const", "override", "final", "noexcept", "mutable", "volatile", "= 0", "=0"] {
							if t.ends_with(kw) {
								t = t[..t.len() - kw.len()].trim_end();
								break;
							}
						}
						if t.len() == prev_len {
							break;
						}
					}
					t.ends_with(')')
				};
				let is_var_init = trimmed.ends_with('=') || !is_function;

				// Walk to the matching closing brace on the normalised view.
				depth = 1;
				i += 1;
				while i < nb.len() && depth > 0 {
					match nb[i] {
						b'{' => depth += 1,
						b'}' => depth -= 1,
						_ => {}
					}
					i += 1;
				}

				if is_var_init {
					// Leave the block untouched — continue without writing.
					continue;
				}

				// Function body: write original prefix verbatim, then the
				// stub. `last_end` advances past the original closing brace.
				let stub_body = stub_body_for_sig(sig);
				result.push_str(&content[last_end..brace_pos]);
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

