use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Name of the project configuration file.
pub const CONFIG_FILE_NAME: &str = "luwu.toml";

/// Configuration format version supported by this binary.
pub const SUPPORTED_CONFIG_VERSION: u32 = 1;

/// Repository URL for updates.
pub const REPOSITORY_URL: &str = "https://github.com/RebisterLog/LuwuToLuau";

/// Directories that are never scanned for `.luwu` sources.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "build",
    "build_test",
    "dist",
    "target",
    "node_modules",
    ".git",
    ".luwu-cache",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Configuration format version.
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub input: InputConfig,

    #[serde(default)]
    pub output: OutputConfig,

    #[serde(default)]
    pub watch: WatchConfig,

    #[serde(default)]
    pub build: BuildConfig,

    #[serde(default)]
    pub sourcemap: SourcemapConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    /// Files or directories that hold the Luwu sources.
    #[serde(default)]
    pub paths: Vec<PathBuf>,

    /// Extra directory or file names that must not be scanned.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    /// Directory that receives the generated `.luau` files.
    #[serde(default = "default_output_dir")]
    pub dir: PathBuf,

    /// Remove the output directory before a build.
    #[serde(default)]
    pub clean: bool,

    /// Copy everything that is not a `.luwu` file (models, json, assets)
    /// into the output directory, keeping the folder structure.
    #[serde(default = "default_true")]
    pub copy_assets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchConfig {
    /// Debounce delay in milliseconds.
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
}

/// Which diagnostics block a build/check run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum FailOn {
    /// A preset name such as "default", "all", "syntax", "contracts", "types".
    Preset(String),
    /// An explicit list of categories.
    List(Vec<String>),
}

impl Default for FailOn {
    fn default() -> Self {
        FailOn::Preset("default".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailOnFlags {
    pub syntax: bool,
    pub contracts: bool,
    pub types: bool,
}

impl FailOn {
    pub fn flags(&self) -> Result<FailOnFlags, String> {
        let names = match self {
            FailOn::Preset(name) => match name.as_str() {
                "default" | "all" => vec!["syntax", "contracts", "types"],
                "syntax" => vec!["syntax"],
                "contracts" => vec!["contracts"],
                "types" => vec!["types"],
                other => {
                    return Err(format!(
                        "unknown build.fail_on preset `{other}` (expected one of: default, all, syntax, contracts, types)"
                    ))
                }
            },
            FailOn::List(list) => list.iter().map(|s| s.as_str()).collect(),
        };

        let mut flags = FailOnFlags {
            syntax: false,
            contracts: false,
            types: false,
        };
        for name in names {
            match name {
                "all" | "default" => {
                    flags.syntax = true;
                    flags.contracts = true;
                    flags.types = true;
                }
                "syntax" => flags.syntax = true,
                "contracts" => flags.contracts = true,
                "types" => flags.types = true,
                other => {
                    return Err(format!(
                    "unknown build.fail_on category `{other}` (expected: syntax, contracts, types)"
                ))
                }
            }
        }
        if !flags.syntax && !flags.contracts && !flags.types {
            return Err("build.fail_on must enable at least one category".to_string());
        }
        Ok(flags)
    }

    pub fn describe(&self) -> String {
        match self {
            FailOn::Preset(s) => s.clone(),
            FailOn::List(list) => format!("[{}]", list.join(", ")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    /// Which diagnostics block a build/check run. Accepts a preset
    /// ("default", "all", "syntax", "contracts", "types") or a list like
    /// ["syntax", "contracts", "types"].
    #[serde(default)]
    pub fail_on: FailOn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcemapConfig {
    /// Path to a manually provided sourcemap (generated by a Roblox plugin)
    /// that contains converted assets. When present, it is merged into the
    /// root sourcemap generated for luwu-lsp.
    #[serde(default = "default_sourcemap_assets")]
    pub assets: PathBuf,

    /// Settings for receiving and filtering DataModel sourcemaps from the
    /// Roblox Studio companion plugin.
    #[serde(default)]
    pub studio: StudioSourcemapConfig,
}

fn default_sourcemap_assets() -> PathBuf {
    PathBuf::from("sourcemap.assets.json")
}

impl Default for SourcemapConfig {
    fn default() -> Self {
        Self {
            assets: default_sourcemap_assets(),
            studio: StudioSourcemapConfig::default(),
        }
    }
}

fn default_studio_port() -> u16 {
    3667
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioSourcemapConfig {
    /// Enable the Studio sourcemap listener/handler.
    #[serde(default)]
    pub enabled: bool,

    /// Port for the companion plugin HTTP endpoint.
    #[serde(default = "default_studio_port")]
    pub port: u16,

    /// Output file for the filtered Studio/DataModel sourcemap.
    #[serde(default = "default_sourcemap_assets")]
    pub output: PathBuf,

    /// After writing the filtered studio sourcemap, regenerate the root
    /// `sourcemap.json` (merging `[sourcemap].assets`) for luau-lsp.
    #[serde(default = "default_true")]
    pub apply_to_root: bool,

    /// Remove the output file (and refresh root sourcemap if enabled) when a
    /// `POST /clear` request is received.
    #[serde(default = "default_true")]
    pub clear_on_request: bool,

    /// Filtering rules applied to incoming DataModel trees.
    #[serde(default)]
    pub filter: StudioFilterConfig,
}

impl Default for StudioSourcemapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_studio_port(),
            output: default_sourcemap_assets(),
            apply_to_root: true,
            clear_on_request: true,
            filter: StudioFilterConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StudioFilterConfig {
    /// Whitelist of DataModel dot-paths/prefixes to include (e.g. `ReplicatedStorage.Assets`).
    /// If empty, the full tree is considered before blacklist/exclude rules.
    #[serde(default)]
    pub include: Vec<String>,

    /// Blacklist of DataModel dot-paths/prefixes to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Whitelist of ClassName values. If non-empty, only these classes are kept.
    #[serde(default)]
    pub include_class: Vec<String>,

    /// Blacklist of ClassName values.
    #[serde(default)]
    pub exclude_class: Vec<String>,

    /// Whitelist of instance names. If non-empty, only matching names are kept.
    #[serde(default)]
    pub include_name: Vec<String>,

    /// Blacklist of instance names.
    #[serde(default)]
    pub exclude_name: Vec<String>,

    /// Remove branches that contain no kept descendants.
    #[serde(default = "default_true")]
    pub prune_empty: bool,
}

fn default_version() -> u32 {
    1
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("build")
}

fn default_debounce() -> u64 {
    400
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            input: InputConfig::default(),
            output: OutputConfig::default(),
            watch: WatchConfig::default(),
            build: BuildConfig::default(),
            sourcemap: SourcemapConfig::default(),
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: default_output_dir(),
            clean: false,
            copy_assets: true,
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce(),
        }
    }
}

/// Where the configuration came from, if anywhere.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub path: Option<PathBuf>,
}

impl LoadedConfig {
    pub fn empty() -> Self {
        Self {
            config: Config::default(),
            path: None,
        }
    }
}

/// Error while loading or validating the configuration.
#[derive(Debug)]
pub struct ConfigError {
    pub path: Option<PathBuf>,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}: {}", path.display(), self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Finds `luwu.toml` in `start` or in any parent directory.
pub fn find_config(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// Loads the configuration from an explicit path or by searching upwards.
pub fn load(
    explicit: Option<&Path>,
    no_config: bool,
    start: &Path,
) -> Result<LoadedConfig, ConfigError> {
    if no_config {
        return Ok(LoadedConfig::empty());
    }

    let path = match explicit {
        Some(path) => {
            if !path.is_file() {
                return Err(ConfigError {
                    path: Some(path.to_path_buf()),
                    message: "configuration file does not exist".into(),
                });
            }
            Some(path.to_path_buf())
        }
        None => find_config(start),
    };

    let Some(path) = path else {
        return Ok(LoadedConfig::empty());
    };

    let text = std::fs::read_to_string(&path).map_err(|e| ConfigError {
        path: Some(path.clone()),
        message: format!("cannot read configuration: {e}"),
    })?;

    let config: Config = toml::from_str(&text).map_err(|e| ConfigError {
        path: Some(path.clone()),
        message: format!("invalid configuration: {e}"),
    })?;

    if config.version != SUPPORTED_CONFIG_VERSION {
        eprintln!(
            "warning: {}: configuration version {} differs from supported version {} (this luwu v{}). Some options may not work as expected. Download/update: {}/releases",
            path.display(),
            config.version,
            SUPPORTED_CONFIG_VERSION,
            env!("CARGO_PKG_VERSION"),
            REPOSITORY_URL
        );
        if config.version > SUPPORTED_CONFIG_VERSION {
            return Err(ConfigError {
                path: Some(path.clone()),
                message: format!(
                    "unsupported configuration version {} (expected {}); please update luwu: {}/releases",
                    config.version,
                    SUPPORTED_CONFIG_VERSION,
                    REPOSITORY_URL
                ),
            });
        }
    }

    config
        .build
        .fail_on
        .flags()
        .map_err(|message| ConfigError {
            path: Some(path.clone()),
            message,
        })?;

    Ok(LoadedConfig {
        config,
        path: Some(path),
    })
}

/// The default configuration file content, used by `luwu init`.
pub fn default_config_text() -> String {
    let config = Config {
        version: 1,
        input: InputConfig {
            paths: vec![PathBuf::from("src")],
            exclude: Vec::new(),
        },
        output: OutputConfig {
            dir: PathBuf::from("build"),
            clean: false,
            copy_assets: true,
        },
        watch: WatchConfig {
            debounce_ms: default_debounce(),
        },
        build: BuildConfig::default(),
        sourcemap: SourcemapConfig::default(),
    };
    let mut text = toml::to_string_pretty(&config).unwrap_or_default();
    text.push_str(
        "\n# Fields:\n\
         #   input.paths                - what to compile (files or folders)\n\
         #   input.exclude              - folder/file names that are never scanned\n\
         #   output.dir                 - where the generated .luau files go\n\
         #   output.clean               - wipe the output folder before a build\n\
         #   output.copy_assets         - copy everything that is not a .luwu file\n\
         #                                (models, json, assets) as is\n\
         #   watch.debounce_ms          - rebuild delay in watch mode, ms\n\
         #   build.fail_on              - diagnostics that block a build/check run:\n\
         #                                \"default\" | \"all\" | \"syntax\" | \"contracts\" | \"types\"\n\
         #                                or a list, e.g. [\"syntax\", \"contracts\", \"types\"]\n\
         #   sourcemap.assets           - Roblox/plugin sourcemap merged into root sourcemap for luwu-lsp\n\
         #   sourcemap.studio.enabled   - listen for Roblox Studio companion plugin updates\n\
         #   sourcemap.studio.port      - HTTP port for the companion plugin (default: 3667)\n\
         #   sourcemap.studio.output    - filtered Studio sourcemap output path\n\
         #   sourcemap.studio.apply_to_root - regenerate root sourcemap.json after updates\n\
         #   sourcemap.studio.filter.*  - include/exclude (dot paths), include_class/exclude_class,\n\
         #                                include_name/exclude_name (whitelist/blacklist), prune_empty\n",
    );
    text
}
