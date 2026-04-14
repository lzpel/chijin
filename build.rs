mod build_delegation;

use std::env;
use std::io::Read;
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

/// Fetch a URL into a byte vector. Supports `http(s)://` via ureq and
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
		let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
		let mut body = Vec::new();
		resp.into_body().into_reader().read_to_end(&mut body).map_err(|e| e.to_string())?;
		Ok(body)
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
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	// advapi32 / user32: no longer needed — patch_occt_sources() stubs the OSD
	// files (OSD_WNT, OSD_File, OSD_Protection, OSD_signal) that reference them.

	// Build cxx bridge + C++ wrapper
	let mut build = cxx_build::bridge("src/occt/ffi.rs");
	build.file("cpp/wrapper.cpp").include(occt_include).std("c++17").define("_USE_MATH_DEFINES", None);

	// Define CADRUM_COLOR for C++ when the "color" feature is enabled.
	#[cfg(feature = "color")]
	build.define("CADRUM_COLOR", None);

	// On MinGW (Windows GNU toolchain), GCC at -O0 emits inline C++ methods
	// (from Standard_ErrorHandler.hxx) as strong (non-COMDAT) symbols in wrapper.o.
	// TKernel.a unconditionally defines the same methods in Standard_ErrorHandler.cxx.obj.
	// This causes "multiple definition" link errors.
	//
	// Standard_ErrorHandler.hxx only generates the inline stubs when OCC_CONVERT_SIGNALS
	// is NOT defined (via `#if !defined(OCC_CONVERT_SIGNALS)`).  Defining it here
	// suppresses those stubs in wrapper.o so only TKernel.a's implementation is linked.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		build.define("OCC_CONVERT_SIGNALS", None);
	}

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
/// (`include`,`lib`), MinGW (`inc`,`win64/gcc/lib`), and MSVC
/// (`win64/vc14/lib`) install layouts.
fn find_occt_dirs(occt_root: &Path) -> [PathBuf; 2] {
	let pick = |cands: [PathBuf; 3]| cands.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| cands[0].clone());
	[
		pick([occt_root.join("include").join("opencascade"), occt_root.join("inc"), occt_root.join("include")]),
		pick([occt_root.join("lib"), occt_root.join("win64").join("gcc").join("lib"), occt_root.join("win64").join("vc14").join("lib")]),
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

	for entry in walkdir::WalkDir::new(source_dir.join("src")).into_iter().flatten() {
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

/// Replace every top-level (brace depth 0) `{ … }` block in `content` with
/// `{}` or `{ return {}; }` and return the result.
///
/// Brace-initialised variables (`static int x{0};`, `std::atomic<T> y{...};`)
/// are preserved verbatim: a `{` preceded by `=`, or whose prefix line has no
/// `(` (not a function signature), is treated as a variable initialiser and
/// skipped rather than stubbed.
fn stub_all_top_level_bodies(content: &str) -> String {
	let bytes = content.as_bytes();
	let mut result = String::new();
	let mut depth = 0usize;
	let mut i = 0;
	let mut last_end = 0;

	while i < bytes.len() {
		match bytes[i] {
			b'{' if depth == 0 => {
				let prefix = &content[last_end..i];

				// Detect brace-initialised variables: if the non-whitespace
				// character immediately before '{' is '=' or the prefix since
				// the last statement terminator contains no '(' (i.e. it is
				// not a function/method signature), treat as variable init
				// and skip the block without stubbing.
				let sig = prefix.rfind(|c| c == ';' || c == '}').map(|p| &prefix[p + 1..]).unwrap_or(prefix);
				let trimmed = sig.trim_end();
				let is_var_init = trimmed.ends_with('=') || !sig.contains('(');

				if is_var_init {
					// Walk forward to find the matching '}' and preserve verbatim.
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
					// Keep the original text (prefix + braced block).
					continue;
				}

				// Function/method body: stub it.
				// Always use "{}" — "{ return {}; }" would fail on constructors,
				// and the CMake build uses -w to suppress missing-return warnings.
				let stub_body = "{}";

				// Walk forward with brace counting to find the matching closing brace.
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
				// i now points one past the closing '}'.
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

