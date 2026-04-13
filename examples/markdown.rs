//! Generate mdbook markdown and README examples section from numbered examples.
//! 番号付き example から mdbook 用 markdown と README の Examples 節を生成する。
//!
//! Usage / 使い方:
//!   cargo run --example markdown -- out/markdown/SUMMARY.md ./README.md
//!
//! 1. Discover NN_*.rs in examples/ / examples/ 配下の NN_*.rs を収集
//! 2. Run each example, collect outputs / 各 example を実行し生成物を回収
//! 3. Write SUMMARY.md + per-example .md / SUMMARY.md と各 example 用 .md を出力
//! 4. Update README.md ## Examples section / README.md の ## Examples 節を更新

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A numbered example file with its source content.
/// 番号付き example ファイルとそのソースコード。
struct Entry {
	path: PathBuf,    // absolute path to the .rs file / .rs ファイルの絶対パス
	content: String,  // source code / ソースコード
}

impl Entry {
	/// File stem, e.g. "01_primitives" / ファイル名（拡張子なし）
	fn stem(&self) -> &str {
		self.path.file_stem().unwrap().to_str().unwrap()
	}

	/// Display title, e.g. "Primitives" / 表示タイトル
	fn title(&self) -> String {
		let raw = self.stem()[3..].replace('_', " ");
		let mut chars = raw.chars();
		match chars.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
		}
	}

	/// First `//!` doc comment line as description / 冒頭の `//!` 行から説明文を抽出
	fn description(&self) -> &str {
		self.content.lines()
			.find(|l| l.starts_with("//!"))
			.map(|l| l.trim_start_matches("//!").trim())
			.unwrap_or("")
	}
}

/// Collect numbered example files (NN_*.rs) sorted by name.
/// 番号付き example (NN_*.rs) をファイル名順に収集する。
fn collect_entries() -> Vec<Entry> {
	let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
	let mut entries: Vec<Entry> = fs::read_dir(&examples_dir)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().into_string().ok()?;
			if name.len() >= 4 && name.ends_with(".rs") && name.as_bytes()[0].is_ascii_digit() && name.as_bytes()[1].is_ascii_digit() && name.as_bytes()[2] == b'_' {
				let path = e.path();
				let content = fs::read_to_string(&path).ok()?;
				Some(Entry { path, content })
			} else {
				None
			}
		})
		.collect();
	entries.sort_by(|a, b| a.stem().cmp(b.stem()));
	entries
}

/// Run each example in a temp directory and collect all generated files.
/// 一時ディレクトリで各 example を実行し、生成されたファイルを回収する。
fn collect_outputs(entries: &[Entry]) -> HashMap<PathBuf, Vec<u8>> {
	let tmp = std::env::temp_dir().join("cadrum_examples");
	clean_dir(&tmp);

	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
	for entry in entries {
		let stem = entry.stem();
		eprintln!("running example: {stem}");
		let status = Command::new("cargo")
			.args(["run", "--manifest-path", manifest.to_str().unwrap(), "--example", stem])
			.current_dir(&tmp)
			.status()
			.unwrap_or_else(|e| panic!("failed to run example {stem}: {e}"));
		assert!(status.success(), "example {stem} failed with {status}");
	}

	// Read all files from the temp directory / 一時ディレクトリの全ファイルを読み込む
	let outputs: HashMap<PathBuf, Vec<u8>> = fs::read_dir(&tmp)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let path = PathBuf::from(e.file_name());
			let contents = fs::read(e.path()).ok()?;
			Some((path, contents))
		})
		.collect();

	let _ = fs::remove_dir_all(&tmp);
	outputs
}

/// Return sorted asset PathBufs from outputs that belong to the given stem.
/// outputs から指定 stem に属するアセットのパスをソート済みで返す。
fn assets_for<'a>(outputs: &'a HashMap<PathBuf, Vec<u8>>, stem: &str) -> Vec<&'a PathBuf> {
	let mut names: Vec<&PathBuf> = outputs.keys()
		.filter(|p| {
			let name = p.to_str().unwrap_or("");
			name.starts_with(stem) && p.extension().map_or(false, |ext| matches!(ext.to_str(), Some("svg" | "png" | "step" | "brep" | "stl")))
		})
		.collect();
	names.sort();
	names
}

/// Write output files, SUMMARY.md, and per-example markdown pages.
/// 生成物・SUMMARY.md・各 example の markdown ページを出力する。
fn write_summary(summary_path: &Path, entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) {
	let out_dir = summary_path.parent().expect("summary_path must have a parent directory");
	clean_dir(out_dir);

	// Write example output files (svg, step, brep, etc.) / example の生成物を書き出す
	for (path, contents) in outputs {
		fs::write(out_dir.join(path), contents).unwrap();
	}

	// Build SUMMARY.md and individual pages / SUMMARY.md と個別ページを生成する
	let mut summary = String::from("# Summary\n\n");
	for entry in entries {
		let (stem, title, desc) = (entry.stem(), entry.title(), entry.description());
		summary.push_str(&format!("- [{}]({}.md)\n", title, stem));

		// Format assets as markdown / 生成物を markdown 形式に変換
		let assets: String = assets_for(outputs, stem).iter()
			.map(|p| {
				let name = p.to_str().unwrap();
				match p.extension().and_then(|e| e.to_str()) {
					Some("svg" | "png") => format!("- {name}\n![img]({name})"),
					_ => format!("- [{name}]({name})"),
				}
			})
			.collect::<Vec<_>>()
			.join("\n\n");

		let desc_section = if desc.is_empty() { String::new() } else { format!("\n{}\n", desc) };
		let assets_section = if assets.is_empty() { String::new() } else { format!("\n{}", assets) };
		let md = format!("# {}\n{}\n```rust\n{}\n```{}", title, desc_section, entry.content, assets_section);
		fs::write(out_dir.join(format!("{}.md", stem)), md).unwrap();
	}

	fs::write(summary_path, &summary).unwrap();
	eprintln!("generated: {}", summary_path.display());
}

/// Render a single example as markdown (description + run command + code + image) for README.
/// README 用に 1つの example を説明 + 実行コマンド + ソース + 画像の markdown として生成する。
/// 画像は GitHub Pages (mdbook 出力) の URL を参照する。
fn render_example(entry: &Entry, outputs: &HashMap<PathBuf, Vec<u8>>) -> String {
	let (stem, desc) = (entry.stem(), entry.description());
	let mut s = String::new();
	if !desc.is_empty() {
		s.push_str(&format!("\n{}\n", desc));
	}
	s.push_str(&format!("\n```sh\ncargo run --example {}\n```\n", stem));
	s.push_str(&format!("\n```rust\n{}\n```\n", entry.content));
	if let Some(img) = first_image(outputs, stem) {
		s.push_str(&format!(
			"\n<p align=\"center\">\n  <img src=\"https://lzpel.github.io/cadrum/{}\" alt=\"{}\" width=\"360\"/>\n</p>\n",
			img, stem
		));
	}
	s
}

/// Find the first SVG/PNG asset for a given stem.
/// 指定 stem の最初の SVG/PNG アセットを返す。
fn first_image<'a>(outputs: &'a HashMap<PathBuf, Vec<u8>>, stem: &str) -> Option<&'a str> {
	assets_for(outputs, stem).into_iter()
		.find(|p| p.extension().map_or(false, |ext| matches!(ext.to_str(), Some("svg" | "png"))))
		.and_then(|p| p.to_str())
}

/// Plain title without the numeric prefix, e.g. "primitives" or "write read".
/// 数字プレフィックス除去 + lowercase + `_` → 空白。
fn plain_title(entry: &Entry) -> String {
	entry.stem()[3..].replace('_', " ")
}

/// GitHub-style slug for linking to `#### Title` anchors, e.g. "write-read".
fn slug(entry: &Entry) -> String {
	plain_title(entry).replace(' ', "-")
}

/// Render the `## Usage` section: thumbnail table + install instructions.
fn render_usage(entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) -> String {
	const COLS: usize = 4;
	let mut s = String::from("## Usage\n\n");

	if !entries.is_empty() {
		let rows = entries.len().div_ceil(COLS);
		for row in 0..rows {
			// Title row / タイトル行
			let mut title_cells = Vec::with_capacity(COLS);
			let mut image_cells = Vec::with_capacity(COLS);
			for col in 0..COLS {
				let idx = row * COLS + col;
				if let Some(entry) = entries.get(idx) {
					let (title, anchor) = (plain_title(entry), slug(entry));
					title_cells.push(format!("[{}](#{})", title, anchor));
					let img_cell = match first_image(outputs, entry.stem()) {
						Some(img) => format!(
							"[<img src=\"https://lzpel.github.io/cadrum/{}\" width=\"180\" alt=\"{}\"/>](#{})",
							img, title, anchor
						),
						None => String::new(),
					};
					image_cells.push(img_cell);
				} else {
					title_cells.push(String::new());
					image_cells.push(String::new());
				}
			}
			s.push_str(&format!("| {} |\n", title_cells.join(" | ")));
			if row == 0 {
				s.push_str("|:---:|:---:|:---:|:---:|\n");
			}
			s.push_str(&format!("| {} |\n", image_cells.join(" | ")));
		}
		s.push('\n');
	}

	s.push_str("More examples with source code are available at [lzpel.github.io/cadrum](https://lzpel.github.io/cadrum).\n\n");
	s.push_str("Add this to your `Cargo.toml`:\n\n");

	// Extract major.minor from CARGO_PKG_VERSION / バージョンから major.minor を抽出
	let version = env!("CARGO_PKG_VERSION");
	let mut parts = version.split('.');
	let major = parts.next().unwrap();
	let minor = parts.next().unwrap();
	s.push_str(&format!("```toml\n[dependencies]\ncadrum = \"^{}.{}\"\n```\n", major, minor));

	s
}

/// Render the `## Example` section: every entry listed with `#### Title`.
fn render_example_section(entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) -> String {
	let mut s = String::from("## Example\n");
	for entry in entries {
		s.push_str(&format!("\n#### {}\n", entry.title()));
		s.push_str(&render_example(entry, outputs));
	}
	s.push('\n');
	s
}

/// Update README.md by replacing `## Usage` and `## Example` sections.
/// README.md の `## Usage` と `## Example` 節を自動生成で置換する。
fn write_readme(readme_path: &Path, entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) {
	let readme = fs::read_to_string(readme_path).expect("failed to read README.md");

	let mut new_readme = String::with_capacity(readme.len());
	let mut last_end = 0;

	for (i, line) in readme.lines().enumerate() {
		let content = match line.trim() {
			"## Usage" => render_usage(entries, outputs),
			"## Example" => render_example_section(entries, outputs),
			_ => continue,
		};

		let line_start = readme.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
		let line_end = line_start + line.len() + 1;
		let section_end = readme[line_end..].find("\n## ")
			.map(|j| line_end + j + 1)
			.unwrap_or(readme.len());

		new_readme.push_str(&readme[last_end..line_start]);
		new_readme.push_str(&content);
		new_readme.push('\n');

		last_end = section_end;
	}
	new_readme.push_str(&readme[last_end..]);

	fs::write(readme_path, &new_readme).unwrap();
	eprintln!("updated: {}", readme_path.display());
}

/// Remove and recreate a directory.
/// ディレクトリを削除して再作成する。
fn clean_dir(dir: &Path) {
	if dir.exists() {
		fs::remove_dir_all(dir).expect("failed to clean directory");
	}
	fs::create_dir_all(dir).expect("failed to create directory");
}

fn main() {
	let entries = collect_entries();
	let outputs = collect_outputs(&entries);

	// Each arg is a file path: dispatch by filename / 各引数をファイル名で判別して処理
	for arg in std::env::args().skip(1) {
		let path = PathBuf::from(&arg);
		let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
		if name.starts_with("SUMMARY") {
			write_summary(&path, &entries, &outputs);
		} else if name.starts_with("README") {
			write_readme(&path, &entries, &outputs);
		} else {
			eprintln!("unknown target: {arg} (expected SUMMARY.md or README.md)");
		}
	}
}
