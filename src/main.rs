mod ast;
mod config;
mod lexer;
mod lower;
mod luau;
mod parser;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clap::{Args, Parser, Subcommand};

use config::{Config, LoadedConfig, DEFAULT_EXCLUDES};
use lower::Lowerer;
use parser::Parser as LuwuParser;

#[derive(Parser)]
#[command(
    name = "luwu",
    version,
    about = "Transpiler from Luwu (.luwu) to plain Luau (.luau)",
    long_about = "Luwu -> Luau transpiler.\n\n\
                  The output contains no types at all: classes, traits, `none`, \
                  `try` and the other Luwu-only constructs are lowered into \
                  ordinary Luau tables and metatables.",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transpile Luwu sources into Luau files
    Build(CommonArgs),

    /// Parse and lower the sources without writing any file
    Check(CommonArgs),

    /// Rebuild whenever a source file changes
    Watch(WatchArgs),

    /// Create a `luwu.toml` configuration file and a project skeleton
    Init(InitArgs),

    /// Create a new project directory with a Luwu skeleton
    New(NewArgs),

    /// Remove the output directory
    Clean(CleanArgs),

    /// Show the effective configuration
    PrintConfig(PrintConfigArgs),
}

#[derive(Args, Clone)]
struct CommonArgs {
    /// Files or directories to compile (overrides `input.paths`)
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Output directory (overrides `output.dir`)
    #[arg(short, long, value_name = "DIR")]
    out: Option<PathBuf>,

    /// Path to the configuration file
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    config: Option<PathBuf>,

    /// Ignore `luwu.toml` completely
    #[arg(long)]
    no_config: bool,

    /// Extra directory or file name to skip (repeatable)
    #[arg(long = "exclude", value_name = "NAME")]
    exclude: Vec<String>,

    /// Print less
    #[arg(short, long)]
    quiet: bool,

    /// Print more
    #[arg(short, long)]
    verbose: bool,

    /// Show what would be done without touching the file system
    #[arg(long)]
    dry_run: bool,

    /// Do not copy non-Luwu files (models, json, assets) into the output
    #[arg(long)]
    no_assets: bool,
}

#[derive(Args, Clone)]
struct WatchArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Debounce delay in milliseconds (overrides `watch.debounce_ms`)
    #[arg(long, value_name = "MS")]
    debounce: Option<u64>,
}

#[derive(Args)]
struct InitArgs {
    /// Directory to initialise (defaults to the current one)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Overwrite files that already exist
    #[arg(long)]
    force: bool,

    /// Do not create the `src/` directory
    #[arg(long)]
    no_src: bool,

    /// Also create a Rojo project (default.project.json and script stubs)
    #[arg(long)]
    rojo: bool,

    /// Project name used in `default.project.json`
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
}

#[derive(Args)]
struct NewArgs {
    /// Name of the new project directory
    #[arg(value_name = "NAME")]
    name: String,

    /// Overwrite files that already exist
    #[arg(long)]
    force: bool,

    /// Also create a Rojo project (default.project.json and script stubs)
    #[arg(long)]
    rojo: bool,
}

#[derive(Args)]
struct CleanArgs {
    /// Output directory to remove (overrides `output.dir`)
    #[arg(short, long, value_name = "DIR")]
    out: Option<PathBuf>,

    /// Path to the configuration file
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    config: Option<PathBuf>,

    /// Ignore `luwu.toml` completely
    #[arg(long)]
    no_config: bool,

    /// Show what would be removed without touching the file system
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct PrintConfigArgs {
    /// Path to the configuration file
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    config: Option<PathBuf>,

    /// Ignore `luwu.toml` completely
    #[arg(long)]
    no_config: bool,
}

/// Everything the commands need after the configuration and the flags are
/// merged together.
struct Resolved {
    paths: Vec<PathBuf>,
    out: PathBuf,
    excludes: Vec<String>,
    clean: bool,
    copy_assets: bool,
    debounce_ms: u64,
    config_path: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Build(args) => run_build(&args, false),
        Command::Check(args) => run_build(&args, true),
        Command::Watch(args) => run_watch(&args),
        Command::Init(args) => run_init(&args),
        Command::New(args) => run_new(&args),
        Command::Clean(args) => run_clean(&args),
        Command::PrintConfig(args) => run_print_config(&args),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn load_config(
    config: Option<&Path>,
    no_config: bool,
) -> Result<LoadedConfig, config::ConfigError> {
    config::load(config, no_config, &std::env::current_dir().unwrap_or_default())
}

fn resolve(
    loaded: &LoadedConfig,
    paths: &[PathBuf],
    out: Option<&Path>,
    exclude: &[String],
    debounce: Option<u64>,
) -> Resolved {
    let cfg: &Config = &loaded.config;

    let mut sources: Vec<PathBuf> = paths.to_vec();
    let explicit = !sources.is_empty();
    if sources.is_empty() {
        if cfg.input.paths.is_empty() {
            sources = vec![default_input()];
        } else {
            sources = cfg.input.paths.clone();
        }
    }

    let out = out
        .map(PathBuf::from)
        .unwrap_or_else(|| cfg.output.dir.clone());

    let mut excludes: Vec<String> = DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect();
    // Paths given on the command line are an explicit request, so the
    // `input.exclude` list of the configuration file does not apply to them.
    if !explicit {
        excludes.extend(cfg.input.exclude.iter().cloned());
    }
    excludes.extend(exclude.iter().cloned());

    Resolved {
        paths: sources,
        out,
        excludes,
        clean: cfg.output.clean,
        copy_assets: cfg.output.copy_assets,
        debounce_ms: debounce.unwrap_or(cfg.watch.debounce_ms),
        config_path: loaded.path.clone(),
    }
}

/// `src/` when it holds Luwu sources, the current directory otherwise.
fn default_input() -> PathBuf {
    let src = Path::new("src");
    if src.is_dir() && contains_luwu(src) {
        PathBuf::from("src")
    } else {
        PathBuf::from(".")
    }
}

fn contains_luwu(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if contains_luwu(&path) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("luwu") {
            return true;
        }
    }
    false
}

fn run_build(args: &CommonArgs, check_only: bool) -> Result<(), String> {
    let loaded = load_config(args.config.as_deref(), args.no_config).map_err(|e| e.to_string())?;
    let resolved = resolve(
        &loaded,
        &args.paths,
        args.out.as_deref(),
        &args.exclude,
        None,
    );

    let collected = collect_all(&resolved.paths, &resolved.excludes, &resolved.out)?;
    let sources = collected.sources;

    if sources.is_empty() && collected.assets.is_empty() {
        let listed = resolved
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("no .luwu files found in `{listed}`"));
    }

    let copy_assets = resolved.copy_assets && !args.no_assets;

    if !check_only && resolved.clean && !args.dry_run {
        remove_output_dir(&resolved.out)?;
    }

    let mut compiled = 0usize;
    let mut failed = 0usize;

    for source in &sources {
        let out_path = map_output_path(&resolved.out, &source.rel);
        match compile_file(&source.file) {
            Ok(code) => {
                compiled += 1;
                if check_only {
                    if args.verbose {
                        println!("checked {}", source.file.display());
                    }
                } else if args.dry_run {
                    println!("would write {}", out_path.display());
                } else {
                    if let Some(parent) = out_path.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            return Err(format!("cannot create `{}`: {e}", parent.display()));
                        }
                    }
                    if let Err(e) = fs::write(&out_path, code) {
                        return Err(format!("cannot write `{}`: {e}", out_path.display()));
                    }
                    if !args.quiet {
                        println!("{} -> {}", source.file.display(), out_path.display());
                    }
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("error: {e}");
            }
        }
    }

    // Everything that is not a Luwu source is copied as is, so that models,
    // json files and assets stay next to the generated Luau.
    let mut copied = 0usize;
    if copy_assets && !check_only {
        for dir in &collected.dirs {
            let target = resolved.out.join(dir);
            if args.dry_run {
                continue;
            }
            if let Err(e) = fs::create_dir_all(&target) {
                return Err(format!("cannot create `{}`: {e}", target.display()));
            }
        }

        for asset in &collected.assets {
            let target = resolved.out.join(&asset.rel);
            if same_path(&asset.file, &target) {
                continue;
            }
            if args.dry_run {
                println!("would copy {}", asset.file.display());
                copied += 1;
                continue;
            }
            if let Some(parent) = target.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return Err(format!("cannot create `{}`: {e}", parent.display()));
                }
            }
            if let Err(e) = fs::copy(&asset.file, &target) {
                return Err(format!(
                    "cannot copy `{}` to `{}`: {e}",
                    asset.file.display(),
                    target.display()
                ));
            }
            copied += 1;
            if args.verbose {
                println!("copied {}", asset.file.display());
            }
        }
    }

    if failed > 0 {
        return Err(format!("{failed} of {} file(s) failed", sources.len()));
    }

    if !args.quiet {
        match (check_only, args.dry_run) {
            (true, _) => println!("checked {compiled} file(s)"),
            (false, true) => {
                println!("would build {compiled} file(s) into {}", resolved.out.display())
            }
            (false, false) => {
                println!(
                    "built {compiled} file(s) into {}",
                    resolved.out.display()
                );
                if copy_assets {
                    println!("copied {copied} file(s)");
                }
            }
        }
    }

    Ok(())
}

fn run_watch(args: &WatchArgs) -> Result<(), String> {
    let loaded = load_config(args.common.config.as_deref(), args.common.no_config)
        .map_err(|e| e.to_string())?;
    let resolved = resolve(
        &loaded,
        &args.common.paths,
        args.common.out.as_deref(),
        &args.common.exclude,
        args.debounce,
    );

    println!(
        "watching {} (Ctrl+C to stop)",
        resolved
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut last: Vec<(PathBuf, SystemTime)> = Vec::new();
    loop {
        let collected = collect_all(&resolved.paths, &resolved.excludes, &resolved.out)?;
        let mut now = Vec::new();
        for source in collected.sources.iter().chain(collected.assets.iter()) {
            let mtime = fs::metadata(&source.file)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            now.push((source.file.clone(), mtime));
        }

        if now != last {
            last = now;
            if let Err(e) = run_build(&args.common, false) {
                eprintln!("error: {e}");
            }
        }

        std::thread::sleep(Duration::from_millis(resolved.debounce_ms));
    }
}

/// Creates the skeleton of a project: the configuration, the source folder
/// and, optionally, a Rojo project that already points at `build/`.
fn scaffold(dir: &Path, rojo: bool, force: bool, name: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create `{}`: {e}", dir.display()))?;

    let mut created: Vec<PathBuf> = Vec::new();

    // The configuration file.
    let config_path = dir.join(config::CONFIG_FILE_NAME);
    if config_path.exists() && !force {
        println!("kept    {} (already exists)", config_path.display());
    } else {
        fs::write(&config_path, config::default_config_text())
            .map_err(|e| format!("cannot write `{}`: {e}", config_path.display()))?;
        created.push(config_path);
    }

    // The source folder.
    let src = dir.join("src");
    if !src.exists() {
        fs::create_dir_all(&src).map_err(|e| format!("cannot create `{}`: {e}", src.display()))?;
        created.push(src.clone());
    }

    // Starter sources.
    if rojo {
        let shared = src.join("shared");
        let server = src.join("server");
        let client = src.join("client");
        for folder in [&shared, &server, &client] {
            if !folder.exists() {
                fs::create_dir_all(folder)
                    .map_err(|e| format!("cannot create `{}`: {e}", folder.display()))?;
                created.push(folder.clone());
            }
        }

        let files: [(PathBuf, &str); 3] = [
            (shared.join("init.luwu"), SAMPLE_SHARED),
            (server.join("init.server.luwu"), SAMPLE_SERVER),
            (client.join("init.client.luwu"), SAMPLE_CLIENT),
        ];
        for (path, content) in files {
            write_new(&path, content, force, &mut created)?;
        }

        let project_name = name
            .map(|n| n.to_string())
            .or_else(|| {
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_string())
            })
            .unwrap_or_else(|| "MyGame".to_string());

        let project_path = dir.join("default.project.json");
        let project = format!(
            "{{\n  \"name\": {},\n  \"tree\": {{\n    \"$className\": \"DataModel\",\n\n    \
             \"ReplicatedStorage\": {{\n      \"Shared\": {{\n        \"$path\": \"build/shared\"\n      }}\n    }},\n\n    \
             \"ServerScriptService\": {{\n      \"Server\": {{\n        \"$path\": \"build/server\"\n      }}\n    }},\n\n    \
             \"StarterPlayer\": {{\n      \"StarterPlayerScripts\": {{\n        \"Client\": {{\n          \"$path\": \"build/client\"\n        }}\n      }}\n    }}\n  }}\n}}\n",
            json_string(&project_name)
        );
        write_new(&project_path, &project, force, &mut created)?;
    } else {
        let main = src.join("main.luwu");
        write_new(&main, SAMPLE_MAIN, force, &mut created)?;
    }

    // A .gitignore that keeps the generated Luau out of the repository.
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, "# Generated Luau\n/build\n")
            .map_err(|e| format!("cannot write `{}`: {e}", gitignore.display()))?;
        created.push(gitignore);
    }

    for path in &created {
        println!("created {}", path.display());
    }

    Ok(())
}

fn write_new(
    path: &Path,
    content: &str,
    force: bool,
    created: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if path.exists() && !force {
        println!("kept    {} (already exists)", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
    }
    fs::write(path, content).map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
    created.push(path.to_path_buf());
    Ok(())
}

fn json_string(value: &str) -> String {
    format!("{:?}", value)
}

const SAMPLE_MAIN: &str = "-- Entry point: `luwu build` turns this file into build/main.luau\nprint(\"hello from luwu\")\n";

const SAMPLE_SHARED: &str = "-- Shared module: becomes a ModuleScript (build/shared/init.luau)\n\
                            class Greeter(name: string)\n\
                            \x20   function hello(self): string\n\
                            \x20       return `hi {self.name}`\n\
                            \x20   end\n\
                            end\n\
                            \n\
                            return {\n    Greeter = Greeter,\n}\n";

const SAMPLE_SERVER: &str = "-- Server script: becomes a Script (build/server/init.server.luau)\n\
                             print(\"server started\")\n";

const SAMPLE_CLIENT: &str = "-- Client script: becomes a LocalScript (build/client/init.client.luau)\n\
                             print(\"client started\")\n";

fn run_init(args: &InitArgs) -> Result<(), String> {
    let dir = args
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let name = args.name.clone();
    scaffold(
        &dir,
        args.rojo,
        args.force,
        name.as_deref(),
    )?;

    println!();
    println!("next steps:");
    println!("  luwu build        # compile the Luwu sources");
    if args.rojo {
        println!("  rojo build -o out.rbxl");
    }
    Ok(())
}

fn run_new(args: &NewArgs) -> Result<(), String> {
    let dir = PathBuf::from(&args.name);
    if dir.exists() {
        let empty = fs::read_dir(&dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !empty && !args.force {
            return Err(format!(
                "`{}` already exists and is not empty (use --force to overwrite)",
                dir.display()
            ));
        }
    }

    let project_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&args.name)
        .to_string();

    scaffold(&dir, args.rojo, args.force, Some(&project_name))?;

    println!();
    println!("next steps:");
    println!("  cd {}", args.name);
    println!("  luwu build");
    if args.rojo {
        println!("  rojo build -o out.rbxl");
    }
    Ok(())
}

fn run_clean(args: &CleanArgs) -> Result<(), String> {
    let loaded = load_config(args.config.as_deref(), args.no_config).map_err(|e| e.to_string())?;
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| loaded.config.output.dir.clone());

    if !out.exists() {
        println!("nothing to clean: {} does not exist", out.display());
        return Ok(());
    }

    if args.dry_run {
        println!("would remove {}", out.display());
        return Ok(());
    }

    remove_output_dir(&out)?;
    println!("removed {}", out.display());
    Ok(())
}

fn run_print_config(args: &PrintConfigArgs) -> Result<(), String> {
    let loaded = load_config(args.config.as_deref(), args.no_config).map_err(|e| e.to_string())?;
    let resolved = resolve(
        &loaded,
        &[],
        None,
        &[],
        None,
    );

    println!(
        "config file: {}",
        resolved
            .config_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    println!("version: {}", loaded.config.version);
    println!(
        "input.paths: {}",
        resolved
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "input.exclude: {}",
        resolved.excludes.join(", ")
    );
    println!("output.dir: {}", resolved.out.display());
    println!("output.clean: {}", resolved.clean);
    println!("watch.debounce_ms: {}", resolved.debounce_ms);
    Ok(())
}

/// A single file together with its path relative to the input root.
struct Source {
    file: PathBuf,
    rel: PathBuf,
}

/// Everything that was found under the input paths.
struct Collected {
    /// `.luwu` files that have to be transpiled.
    sources: Vec<Source>,
    /// Everything else: models, json, images, sounds - copied as is.
    assets: Vec<Source>,
    /// Directories, so that empty folders survive the build as well.
    dirs: Vec<PathBuf>,
}

fn collect_all(
    paths: &[PathBuf],
    excludes: &[String],
    out_dir: &Path,
) -> Result<Collected, String> {
    let mut collected = Collected {
        sources: Vec::new(),
        assets: Vec::new(),
        dirs: Vec::new(),
    };

    for path in paths {
        if path.is_file() {
            if path.extension().and_then(|e| e.to_str()) == Some("luwu") {
                let name = path
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("out.luwu"));
                collected.sources.push(Source {
                    file: path.clone(),
                    rel: name,
                });
            } else {
                let name = path
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("asset"));
                collected.assets.push(Source {
                    file: path.clone(),
                    rel: name,
                });
            }
            continue;
        }
        if !path.exists() {
            return Err(format!("input `{}` does not exist", path.display()));
        }
        collect_rec(path, path, excludes, out_dir, &mut collected)?;
    }

    collected
        .sources
        .sort_by(|a, b| a.rel.cmp(&b.rel));
    collected.assets.sort_by(|a, b| a.rel.cmp(&b.rel));
    collected.dirs.sort();
    Ok(collected)
}

fn collect_rec(
    root: &Path,
    dir: &Path,
    excludes: &[String],
    out_dir: &Path,
    collected: &mut Collected,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if excludes.iter().any(|e| e == name) {
                continue;
            }
            if same_path(&path, out_dir) {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(root) {
                collected.dirs.push(rel.to_path_buf());
            }
            collect_rec(root, &path, excludes, out_dir, collected)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if path.extension().and_then(|e| e.to_str()) == Some("luwu") {
                collected.sources.push(Source { file: path, rel });
            } else {
                collected.assets.push(Source { file: path, rel });
            }
        }
    }
    Ok(())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn map_output_path(out_dir: &Path, rel: &Path) -> PathBuf {
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("out.luwu");
    let new_name = if let Some(stem) = name.strip_suffix(".luwu") {
        format!("{stem}.luau")
    } else {
        format!("{name}.luau")
    };
    let mut path = out_dir.to_path_buf();
    if let Some(parent) = rel.parent() {
        path.push(parent);
    }
    path.push(new_name);
    path
}

fn compile_file(file: &Path) -> Result<String, String> {
    let source = fs::read_to_string(file).map_err(|e| format!("cannot read `{}`: {e}", file.display()))?;
    let tokens = lexer::lex(&source).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut parser = LuwuParser::new(tokens);
    let program = parser
        .parse_program()
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let mut lowerer = Lowerer::new(&program);
    lowerer
        .lower(&program)
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// Removes the output directory, refusing to touch anything dangerous.
fn remove_output_dir(out_dir: &Path) -> Result<(), String> {
    if !is_safe_to_remove(out_dir) {
        return Err(format!(
            "refusing to remove `{}`: pick a dedicated output directory",
            out_dir.display()
        ));
    }
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)
            .map_err(|e| format!("cannot remove `{}`: {e}", out_dir.display()))?;
    }
    Ok(())
}

fn is_safe_to_remove(dir: &Path) -> bool {
    if dir.as_os_str().is_empty() {
        return false;
    }
    let canonical = match dir.canonicalize() {
        Ok(path) => path,
        Err(_) => return false,
    };
    if canonical.parent().is_none() {
        return false;
    }
    if let Ok(current) = std::env::current_dir() {
        if let Ok(current) = current.canonicalize() {
            if canonical == current {
                return false;
            }
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if let Ok(home) = Path::new(&home).canonicalize() {
            if canonical == home {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(source: &str) -> String {
        let tokens = lexer::lex(source).expect("lex");
        let mut parser = LuwuParser::new(tokens);
        let program = parser.parse_program().expect("parse");
        let mut lowerer = Lowerer::new(&program);
        lowerer.lower(&program).expect("lower")
    }

    #[test]
    fn lowers_a_basic_class() {
        let out = compile(
            r#"
class Dog(name: string, age: number)
    function bark(self)
        print(`{self.name} barked!`)
    end
end

const oreo = Dog("Oreo", 3)
oreo:bark()
"#,
        );
        assert!(out.contains("const Dog = {}"));
        assert!(out.contains("function Dog.new(name, age)"));
        assert!(out.contains("self.name = name"));
        assert!(out.contains("function Dog:bark()"));
        assert!(out.contains("__call = function(_, ...)"));
        assert!(!out.contains(": string"));
    }

    #[test]
    fn lowers_destructuring_and_bundles() {
        let out = compile(
            r#"
const point = .(3, 4)
const .(x, y) = point
const copy = .{ x, y }
print(point.1, x + y, copy.x)
"#,
        );
        assert!(out.contains("const point = { 3, 4 }"));
        assert!(out.contains("const x, y = point[1], point[2]"));
        assert!(out.contains("{ x = x, y = y }"));
        assert!(out.contains("point[1]"));
    }

    #[test]
    fn lowers_try_and_ok_err() {
        let out = compile(
            r#"
const function parse_number(text: string): number
    return tonumber(text) or error(`bad: {text}`, 2)
end

const ok = try parse_number("42")
if ok is Ok.(value) then
    print(value)
end
"#,
        );
        assert!(out.contains("const ok_ok, ok_value = pcall(parse_number, \"42\")"));
        assert!(out.contains("if ok_ok then"));
        assert!(out.contains("const value = ok_value"));
    }

    #[test]
    fn lowers_parameterized_trait() {
        let out = compile(
            r#"
trait Stackable(max_stack: number = 64)
    count = 1
    function add(self, amount: number): number
        self.count += math.min(self.max_stack - self.count, amount)
        return self.count
    end
end

class Arrow implements Stackable end
class Potion implements Stackable(8) end
"#,
        );
        assert!(out.contains("const function Stackable(max_stack)"));
        assert!(out.contains("const trait = { max_stack = max_stack, count = 1 }"));
        assert!(out.contains("StackedArrowStackable = Stackable()"));
        assert!(out.contains("StackedPotionStackable = Stackable(8)"));
        assert!(out.contains("Arrow.add = StackedArrowStackable.add"));
    }

    fn fixture_files() -> Vec<std::path::PathBuf> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut out = Vec::new();
        collect_fixtures(&root, &mut out);
        out.sort();
        out
    }

    fn collect_fixtures(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_fixtures(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("luwu") {
                out.push(path);
            }
        }
    }

    fn compile_file(path: &std::path::Path) -> String {
        let source = fs::read_to_string(path).expect("read fixture");
        let tokens = lexer::lex(&source).expect("lex");
        let mut parser = LuwuParser::new(tokens);
        let program = parser.parse_program().expect("parse");
        let mut lowerer = Lowerer::new(&program);
        lowerer.lower(&program).expect("lower")
    }

    #[test]
    fn every_fixture_compiles_into_plain_luau() {
        let files = fixture_files();
        assert!(files.len() >= 7, "expected the fixture set to be present");
        for file in &files {
            let out = compile_file(file);
            assert!(!out.trim().is_empty(), "{} produced nothing", file.display());
            for line in out.lines() {
                let trimmed = line.trim_start();
                for keyword in [
                    "trait ",
                    "class ",
                    "implements",
                    "expect ",
                    "public ",
                    "private ",
                    "final ",
                ] {
                    assert!(
                        !trimmed.starts_with(keyword),
                        "{} leaked `{keyword}`: {}",
                        file.display(),
                        trimmed
                    );
                }
            }
            for leak in [": string", ": number", ": boolean", "class<", "nil?"] {
                assert!(
                    !out.contains(leak),
                    "{} leaked the type syntax `{leak}`",
                    file.display()
                );
            }
        }
    }

    #[test]
    fn trait_equality_does_not_leak_between_classes() {
        let out = compile(
            r#"
trait Vector
    function __eq(self, other): boolean
        return self.x == other.x and self.y == other.y
    end
end

class Point(x: number, y: number) implements Vector end
class Other(x: number, y: number) implements Vector end
"#,
        );
        assert!(out.contains("function Point.__eq(self, other)"));
        assert!(out.contains("function Other.__eq(self, other)"));
        assert!(out.contains("getmetatable(self)"));
    }

    #[test]
    fn traits_with_create_keep_their_methods() {
        let out = compile(
            r#"
const registry: { [class<Plugin>]: true? } = {}

trait Plugin
    expect function name(): string

    function greet(self): string
        return `plugin:{self:name()}`
    end

    function __create(wanted: string): Plugin
        for cls in registry do
            if cls.name() == wanted then
                return cls()
            end
        end
        return error(`missing plugin {wanted}`, 2)
    end
end

class Alpha implements Plugin
    function name(): string
        return "alpha"
    end
end
"#,
        );
        // The trait stays a table so that its methods reach the classes.
        assert!(out.contains("const Plugin = {}"));
        assert!(out.contains("function Plugin:greet()"));
        assert!(out.contains("Alpha.greet = Plugin.greet"));
        // `__create` makes the trait table callable.
        assert!(out.contains("__call = function(_, wanted)"));
    }

    #[test]
    fn traits_are_copied_through_needs() {
        let out = compile(
            r#"
trait Base
    function who(self): string
        return "base"
    end
end

trait Middle needs Base
    function middle(self): string
        return "middle"
    end
end

class Leaf implements Middle end
"#,
        );
        assert!(out.contains("Leaf.middle = Middle.middle"));
        assert!(out.contains("Leaf.who = Base.who"));
    }

    #[test]
    fn trait_members_show_up_in_class_metadata() {
        let out = compile(
            r#"
trait Named
    const name = "x"

    function hello(self): string
        return self.name
    end
end

class Robot implements Named end

print(class.fields(Robot()), class.member(Robot, "hello").kind)
"#,
        );
        assert!(out.contains("Robot.fields = {"));
        assert!(out.contains("Robot.members = {"));
        assert!(out.contains("\"hello\""));
        assert!(out.contains("\"method\""));
    }

    #[test]
    fn init_scaffolds_a_project() {
        let root = std::env::temp_dir().join(format!("luwu-init-{}", std::process::id()));
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(&root).unwrap();

        run_init(&InitArgs {
            path: Some(root.clone()),
            force: false,
            no_src: false,
            rojo: true,
            name: Some("Demo".into()),
        })
        .expect("init succeeds");

        assert!(root.join("luwu.toml").exists(), "config is created");
        assert!(root.join("src/shared/init.luwu").exists(), "shared stub");
        assert!(root.join("src/server/init.server.luwu").exists(), "server stub");
        assert!(root.join("src/client/init.client.luwu").exists(), "client stub");
        assert!(root.join("default.project.json").exists(), "rojo project");

        let project = fs::read_to_string(root.join("default.project.json")).unwrap();
        assert!(project.contains("\"name\": \"Demo\""));
        assert!(project.contains("build/shared"));

        // The generated config is valid and points at the sources.
        let loaded = config::load(Some(&root.join("luwu.toml")), false, &root).unwrap();
        assert_eq!(loaded.config.input.paths, vec![PathBuf::from("src")]);
        assert!(loaded.config.output.copy_assets);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn new_creates_a_project_directory() {
        let root = std::env::temp_dir().join(format!("luwu-new-{}", std::process::id()));
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(&root).unwrap();

        run_new(&NewArgs {
            name: root.join("Game").to_string_lossy().to_string(),
            force: false,
            rojo: false,
        })
        .expect("new succeeds");

        let project = root.join("Game");
        assert!(project.join("luwu.toml").exists());
        assert!(project.join("src/main.luwu").exists());
        assert!(project.join(".gitignore").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn copies_assets_next_to_the_generated_luau() {
        let root = std::env::temp_dir().join(format!("luwu-assets-{}", std::process::id()));
        let src = root.join("src");
        let out = root.join("build");
        fs::create_dir_all(src.join("world")).unwrap();
        fs::create_dir_all(src.join("assets")).unwrap();
        fs::write(src.join("main.luwu"), "print(1)\n").unwrap();
        fs::write(src.join("world/Arena.model.json"), "{}\n").unwrap();
        fs::write(src.join("assets/logo.png"), "binary").unwrap();
        fs::create_dir_all(src.join("empty")).unwrap();

        let args = CommonArgs {
            paths: vec![src.clone()],
            out: Some(out.clone()),
            config: None,
            no_config: true,
            exclude: Vec::new(),
            quiet: true,
            verbose: false,
            dry_run: false,
            no_assets: false,
        };
        run_build(&args, false).expect("build succeeds");

        assert!(out.join("main.luau").exists(), "luau file is written");
        assert!(
            out.join("world/Arena.model.json").exists(),
            "models are copied"
        );
        assert!(
            out.join("assets/logo.png").exists(),
            "assets are copied"
        );
        assert!(out.join("empty").is_dir(), "empty folders are kept");

        // With --no-assets only the Luau files are produced.
        let out2 = root.join("build2");
        let args2 = CommonArgs {
            out: Some(out2.clone()),
            no_assets: true,
            ..args
        };
        run_build(&args2, false).expect("build succeeds");

        assert!(out2.join("main.luau").exists());
        assert!(!out2.join("world/Arena.model.json").exists());
        assert!(!out2.join("assets/logo.png").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn or_return_guards_against_none() {
        let out = compile(
            r#"
const function find(items: { number }, wanted: number): number?
    for _, item in items do
        if item == wanted then
            return item
        end
    end
    return none
end

const function describe(items: { number }, wanted: number): string
    const found = find(items, wanted) or return "missing"
    return `found {found}`
end
"#,
        );
        // `none` is not `nil`, so the guard has to compare with the sentinel.
        assert!(out.contains("if (found == none) then"));
        assert!(out.contains("const none = setmetatable"));
    }

    #[test]
    fn config_merges_flags_over_file() {
        let loaded = LoadedConfig {
            config: Config {
                version: 1,
                input: config::InputConfig {
                    paths: vec![PathBuf::from("src")],
                    exclude: vec!["Generated".into()],
                },
                output: config::OutputConfig {
                    dir: PathBuf::from("out"),
                    clean: true,
                    copy_assets: true,
                },
                watch: config::WatchConfig {
                    debounce_ms: 250,
                },
            },
            path: Some(PathBuf::from("luwu.toml")),
        };

        let resolved = resolve(
            &loaded,
            &[],
            Some(Path::new("dist")),
            &["Extra".to_string()],
            None,
        );
        assert_eq!(resolved.paths, vec![PathBuf::from("src")]);
        assert_eq!(resolved.out, PathBuf::from("dist"));
        assert!(resolved.clean);
        assert_eq!(resolved.debounce_ms, 250);
        assert!(resolved.excludes.iter().any(|e| e == "Generated"));
        assert!(resolved.excludes.iter().any(|e| e == "Extra"));
        assert!(resolved.excludes.iter().any(|e| e == "build"));
    }

    #[test]
    fn config_paths_are_overridden_by_arguments() {
        let loaded = LoadedConfig {
            config: Config {
                input: config::InputConfig {
                    paths: vec![PathBuf::from("src")],
                    exclude: vec!["Generated".into()],
                },
                ..Config::default()
            },
            path: None,
        };

        let resolved = resolve(
            &loaded,
            &[PathBuf::from("game")],
            None,
            &[],
            Some(1000),
        );
        assert_eq!(resolved.paths, vec![PathBuf::from("game")]);
        assert_eq!(resolved.out, PathBuf::from("build"));
        assert_eq!(resolved.debounce_ms, 1000);
        // An explicit path is an explicit request: the configured excludes do
        // not apply to it.
        assert!(!resolved.excludes.iter().any(|e| e == "Generated"));
        assert!(resolved.excludes.iter().any(|e| e == "build"));
    }

    #[test]
    fn default_config_file_is_valid() {
        let text = config::default_config_text();
        let parsed: Config = toml::from_str(&text).expect("default config parses");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.input.paths, vec![PathBuf::from("src")]);
        assert_eq!(parsed.output.dir, PathBuf::from("build"));
    }

    #[test]
    fn output_paths_keep_the_rojo_suffixes() {
        assert_eq!(
            map_output_path(Path::new("build"), Path::new("Foo.server.luwu")),
            PathBuf::from("build/Foo.server.luau")
        );
        assert_eq!(
            map_output_path(Path::new("build"), Path::new("Mod/init.luwu")),
            PathBuf::from("build/Mod/init.luau")
        );
        assert_eq!(
            map_output_path(Path::new("build"), Path::new("Bar.client.luwu")),
            PathBuf::from("build/Bar.client.luau")
        );
    }
}
