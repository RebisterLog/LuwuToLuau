use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Source-oriented sourcemap for luwu-lsp: includes `.luwu` paths.
    Source,
    /// Build-oriented sourcemap for Rojo: `.luau`/assets only, no `.luwu`.
    Build,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(rename = "className", alias = "ClassName", alias = "class_name")]
    pub class_name: String,
    #[serde(
        rename = "filePaths",
        alias = "FilePaths",
        alias = "file_paths",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub file_paths: Vec<String>,
    #[serde(alias = "Children", default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

pub fn generate_from_project(project_path: &Path, mode: Mode) -> Result<Node, String> {
    let text = fs::read_to_string(project_path)
        .map_err(|e| format!("cannot read `{}`: {e}", project_path.display()))?;
    let project: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid project json `{}`: {e}", project_path.display()))?;
    let tree = project
        .get("tree")
        .ok_or_else(|| format!("`{}`: missing `tree`", project_path.display()))?;
    let root_name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("game")
        .to_string();
    let project_dir = project_path.parent().unwrap_or_else(|| Path::new("."));
    let mut node = build_node(&root_name, tree, project_dir, mode)?;
    // Root of a Rojo project is typically DataModel.
    if node.class_name.is_empty() {
        node.class_name = "DataModel".to_string();
    }
    normalize_node(&mut node);
    Ok(node)
}

fn build_node(name: &str, value: &Value, project_dir: &Path, mode: Mode) -> Result<Node, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("project node `{name}` must be an object"))?;

    let mut class_name = obj
        .get("$className")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let path = obj.get("$path").and_then(|v| v.as_str());

    let mut file_paths: Vec<String> = Vec::new();
    let mut children: Vec<Node> = Vec::new();

    if let Some(p) = path {
        let fs_path = project_dir.join(p);
        let (cn, fps, ch) = node_from_path(name, &fs_path, project_dir, mode)?;
        if class_name.is_empty() {
            class_name = cn;
        }
        file_paths.extend(fps);
        children.extend(ch);
    }

    for (k, v) in obj {
        if k.starts_with('$') {
            continue;
        }
        children.push(build_node(k, v, project_dir, mode)?);
    }

    if class_name.is_empty() {
        class_name = "Folder".into();
    }

    Ok(Node {
        name: name.to_string(),
        class_name,
        file_paths,
        children,
    })
}

fn node_from_path(
    name: &str,
    fs_path: &Path,
    project_dir: &Path,
    mode: Mode,
) -> Result<(String, Vec<String>, Vec<Node>), String> {
    if fs_path.is_file() {
        let (_node_name, class_name) = classify_file(name, fs_path, mode)
            .unwrap_or_else(|| (file_stem_name(fs_path), "ModuleScript".to_string()));
        let rel = rel_path_string(fs_path, project_dir);
        return Ok((class_name, vec![rel], Vec::new()));
    }
    if !fs_path.is_dir() {
        // Missing paths are tolerated; Rojo can have placeholders.
        return Ok(("Folder".to_string(), Vec::new(), Vec::new()));
    }
    expand_directory(name, fs_path, project_dir, mode)
}

fn expand_directory(
    name: &str,
    dir: &Path,
    project_dir: &Path,
    mode: Mode,
) -> Result<(String, Vec<String>, Vec<Node>), String> {
    let init = find_init(dir, mode);
    let mut file_paths = Vec::new();
    let mut class_name = "Folder".to_string();

    if let Some(init_path) = &init {
        if let Some((_n, cn)) = classify_file(name, init_path, mode) {
            class_name = cn;
        } else {
            class_name = "ModuleScript".to_string();
        }
        file_paths.push(rel_path_string(init_path, project_dir));
        let init_meta = dir.join("init.meta.json");
        if init_meta.is_file() {
            file_paths.push(rel_path_string(&init_meta, project_dir));
        }
    }

    let mut children = Vec::new();
    let entries = read_dir_sorted(dir)?;
    for entry in entries {
        let path = dir.join(&entry);
        if Some(&path) == init.as_ref() {
            continue;
        }
        let fname = entry.to_string_lossy().to_string();
        if fname == "init.meta.json" {
            continue;
        }
        if path.is_dir() {
            let (cn, fps, ch) = expand_directory(&fname, &path, project_dir, mode)?;
            // Skip empty folders in build mode? keep folders for structure.
            children.push(Node {
                name: fname,
                class_name: cn,
                file_paths: fps,
                children: ch,
            });
            continue;
        }
        if is_ignored_file(&fname) {
            continue;
        }
        if let Some((node_name, cn)) = classify_file(&fname, &path, mode) {
            let meta = meta_for_script(&path);
            let mut fps = vec![rel_path_string(&path, project_dir)];
            if let Some(m) = meta {
                if m.is_file() {
                    fps.push(rel_path_string(&m, project_dir));
                }
            }
            children.push(Node {
                name: node_name,
                class_name: cn,
                file_paths: fps,
                children: Vec::new(),
            });
        } else if mode == Mode::Source {
            // Keep non-script assets as file-backed nodes in source mode so they
            // can be represented/merged; use simple name/class inference.
            if let Some(asset_node) = asset_node_from_file(&fname, &path, project_dir) {
                children.push(asset_node);
            }
        } else {
            // Build mode: include assets/models/json except .luwu (none) and meta handled elsewhere.
            if let Some(asset_node) = asset_node_from_file(&fname, &path, project_dir) {
                // Don't emit .luwu in build mode (shouldn't exist) and don't emit init.meta duplicates badly.
                if !fname.ends_with(".luwu") {
                    children.push(asset_node);
                }
            }
        }
    }

    Ok((class_name, file_paths, children))
}

fn find_init(dir: &Path, mode: Mode) -> Option<PathBuf> {
    let candidates: &[&str] = match mode {
        Mode::Source => &[
            "init.server.luwu",
            "init.client.luwu",
            "init.luwu",
            "init.server.luau",
            "init.client.luau",
            "init.luau",
            "init.server.lua",
            "init.client.lua",
            "init.lua",
        ],
        Mode::Build => &[
            "init.server.luau",
            "init.client.luau",
            "init.luau",
            "init.server.lua",
            "init.client.lua",
            "init.lua",
        ],
    };
    for c in candidates {
        let p = dir.join(c);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn classify_file(name_hint: &str, path: &Path, mode: Mode) -> Option<(String, String)> {
    let fname = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name_hint);
    // Order matters: more specific suffixes first.
    let patterns: Vec<(&str, &str, bool)> = match mode {
        Mode::Source => vec![
            (".server.luwu", "Script", true),
            (".client.luwu", "LocalScript", true),
            (".server.luau", "Script", true),
            (".client.luau", "LocalScript", true),
            (".server.lua", "Script", true),
            (".client.lua", "LocalScript", true),
            (".luwu", "ModuleScript", true),
            (".luau", "ModuleScript", true),
            (".lua", "ModuleScript", true),
        ],
        Mode::Build => vec![
            (".server.luau", "Script", true),
            (".client.luau", "LocalScript", true),
            (".server.lua", "Script", true),
            (".client.lua", "LocalScript", true),
            (".luau", "ModuleScript", true),
            (".lua", "ModuleScript", true),
        ],
    };

    for (suffix, class_name, _strip_suffix) in patterns {
        if fname.ends_with(suffix) {
            let mut node_name = fname.strip_suffix(suffix).unwrap_or(fname).to_string();
            // init files take the container/folder name; caller passes folder name.
            if node_name == "init" {
                node_name = name_hint.to_string();
            }
            return Some((node_name, class_name.to_string()));
        }
    }
    // In source mode we never treat .luwu specially here beyond above; in build mode ignore .luwu.
    if mode == Mode::Build && fname.ends_with(".luwu") {
        return None;
    }
    None
}

fn meta_for_script(script_path: &Path) -> Option<PathBuf> {
    let fname = script_path.file_name()?.to_str()?;
    // script meta is usually Name.meta.json alongside, but simplest: replace known script suffix with .meta.json
    // Keep conservative: try exact common forms.
    let p = script_path.parent()?;
    for suffix in [
        ".server.luwu",
        ".client.luwu",
        ".server.luau",
        ".client.luau",
        ".server.lua",
        ".client.lua",
        ".luwu",
        ".luau",
        ".lua",
    ] {
        if let Some(stem) = fname.strip_suffix(suffix) {
            let meta = p.join(format!("{stem}.meta.json"));
            if meta.exists() {
                return Some(meta);
            }
        }
    }
    None
}

fn asset_node_from_file(fname: &str, path: &Path, project_dir: &Path) -> Option<Node> {
    if fname == "init.meta.json" || fname.ends_with(".meta.json") {
        return None;
    }
    if fname.ends_with(".luwu") || fname.ends_with(".luau") || fname.ends_with(".lua") {
        return None;
    }
    let (name, class_name) = if let Some(stem) = fname.strip_suffix(".model.json") {
        (stem.to_string(), "Model".to_string())
    } else if let Some(stem) = fname.strip_suffix(".project.json") {
        (stem.to_string(), "Folder".to_string())
    } else if let Some(stem) = fname.strip_suffix(".txt") {
        (stem.to_string(), "StringValue".to_string())
    } else if let Some(stem) = fname.strip_suffix(".csv") {
        (stem.to_string(), "LocalizationTable".to_string())
    } else if let Some(stem) = fname.strip_suffix(".json") {
        (stem.to_string(), "ModuleScript".to_string())
    } else if fname.ends_with(".rbxm") || fname.ends_with(".rbxmx") {
        (file_stem_name(path), "Model".to_string())
    } else {
        // Generic asset: keep as a file-backed node if Rojo/LSP benefits; Folder is safest fallback.
        (file_stem_name(path), "Folder".to_string())
    };
    Some(Node {
        name,
        class_name,
        file_paths: vec![rel_path_string(path, project_dir)],
        children: Vec::new(),
    })
}

fn is_ignored_file(fname: &str) -> bool {
    fname == ".DS_Store" || fname == "Thumbs.db"
}

fn file_stem_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Node")
        .to_string()
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<std::ffi::OsString>, String> {
    let mut names = fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .collect::<Vec<_>>();
    names.sort_by_key(|n| n.to_string_lossy().to_lowercase());
    Ok(names)
}

fn rel_path_string(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

pub fn normalize_node(node: &mut Node) {
    node.file_paths.sort();
    node.file_paths.dedup();
    node.children.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.name.cmp(&b.name))
    });
    // Merge duplicate child names produced by different passes.
    let mut merged: Vec<Node> = Vec::new();
    for child in node.children.drain(..) {
        if let Some(last) = merged.last_mut() {
            if last.name == child.name {
                merge_nodes(last, child);
                continue;
            }
        }
        merged.push(child);
    }
    node.children = merged;
    for c in &mut node.children {
        normalize_node(c);
    }
}

fn merge_nodes(target: &mut Node, other: Node) {
    if (target.class_name == "Folder" || target.class_name.is_empty())
        && !other.class_name.is_empty()
    {
        target.class_name = other.class_name.clone();
    }
    target.file_paths.extend(other.file_paths);
    for oc in other.children {
        if let Some(tc) = target.children.iter_mut().find(|c| c.name == oc.name) {
            merge_nodes(tc, oc);
        } else {
            target.children.push(oc);
        }
    }
}

/// Merge `other` (e.g. manual assets sourcemap) into `target` (root/source sourcemap).
pub fn merge_into(target: &mut Node, other: Node) {
    if target.name.is_empty() {
        target.name = other.name.clone();
    }
    if (target.class_name.is_empty() || target.class_name == "Folder")
        && !other.class_name.is_empty()
    {
        target.class_name = other.class_name;
    }
    for fp in other.file_paths {
        if !target.file_paths.contains(&fp) {
            target.file_paths.push(fp);
        }
    }
    for oc in other.children {
        if let Some(tc) = target.children.iter_mut().find(|c| c.name == oc.name) {
            merge_into(tc, oc);
        } else {
            target.children.push(oc);
        }
    }
    normalize_node(target);
}

pub fn write_json(path: &Path, node: &Node) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(node)
        .map_err(|e| format!("cannot serialize sourcemap: {e}"))?;
    fs::write(path, json + "\n").map_err(|e| format!("cannot write `{}`: {e}", path.display()))
}

pub fn read_sourcemap(path: &Path) -> Result<Node, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid sourcemap `{}`: {e}", path.display()))
}

/// Resolve the project root directory for outputs. Prefer the directory that
/// contains the config, otherwise current directory.
pub fn workspace_root(config_path: Option<&Path>) -> PathBuf {
    config_path
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn root_project_path(workspace: &Path) -> PathBuf {
    workspace.join("default.project.json")
}

/// Generate the build/default.project.json inside `out_dir` by rewriting
/// `$path` values from the root source project: leading `src/` is stripped so
/// paths point into the compiled layout inside the build directory.
pub fn generate_build_project(
    source_project: &Path,
    out_dir: &Path,
    project_name_fallback: &str,
) -> Result<PathBuf, String> {
    let text = fs::read_to_string(source_project)
        .map_err(|e| format!("cannot read `{}`: {e}", source_project.display()))?;
    let mut project: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid project json `{}`: {e}", source_project.display()))?;
    if project.get("name").is_none() {
        project["name"] = Value::String(project_name_fallback.to_string());
    }
    if let Some(tree) = project.get_mut("tree") {
        rewrite_tree_paths(tree);
    }
    let out = out_dir.join("default.project.json");
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&project)
        .map_err(|e| format!("cannot serialize build project: {e}"))?;
    fs::write(&out, json + "\n").map_err(|e| format!("cannot write `{}`: {e}", out.display()))?;
    Ok(out)
}

fn rewrite_tree_paths(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(p) = obj
        .get("$path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        let rewritten = rewrite_path(&p);
        obj.insert("$path".to_string(), Value::String(rewritten));
    }
    let keys: Vec<String> = obj.keys().cloned().collect();
    for k in keys {
        if k.starts_with('$') {
            continue;
        }
        if let Some(v) = obj.get_mut(&k) {
            rewrite_tree_paths(v);
        }
    }
}

fn rewrite_path(p: &str) -> String {
    let normalized = p.replace('\\', "/");
    for prefix in ["src/", "./src/"] {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    if normalized == "src" || normalized == "./src" {
        return ".".to_string();
    }
    normalized
}

/// Generate root sourcemap.json (for luwu-lsp) from root default.project.json
/// and merge manual assets sourcemap from config if present.
pub fn generate_root_sourcemap(
    workspace: &Path,
    assets_rel: &Path,
    verbose: bool,
) -> Result<Option<PathBuf>, String> {
    let project = root_project_path(workspace);
    if !project.is_file() {
        return Ok(None);
    }
    let mut node = generate_from_project(&project, Mode::Source)?;
    let assets_path = if assets_rel.is_absolute() {
        assets_rel.to_path_buf()
    } else {
        workspace.join(assets_rel)
    };
    if assets_path.is_file() {
        match read_sourcemap(&assets_path) {
            Ok(assets) => merge_into(&mut node, assets),
            Err(e) => {
                if verbose {
                    eprintln!("warning: {e}");
                }
            }
        }
    }
    normalize_node(&mut node);
    let out = workspace.join("sourcemap.json");
    write_json(&out, &node)?;
    Ok(Some(out))
}

/// Generate build/default.project.json and build/sourcemap.json (no .luwu).
pub fn generate_build_artifacts(
    workspace: &Path,
    out_dir: &Path,
    verbose: bool,
) -> Result<(), String> {
    let source_project = root_project_path(workspace);
    if !source_project.is_file() {
        if verbose {
            eprintln!(
                "warning: no default.project.json in workspace, skipping build sourcemap artifacts"
            );
        }
        return Ok(());
    }
    let name = fs::read_to_string(&source_project)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "game".to_string());
    let build_project = generate_build_project(&source_project, out_dir, &name)?;
    let mut node = generate_from_project(&build_project, Mode::Build)?;
    normalize_node(&mut node);
    let out = out_dir.join("sourcemap.json");
    write_json(&out, &node)?;
    if verbose {
        eprintln!(
            "generated {} and {}",
            build_project.display(),
            out.display()
        );
    }
    Ok(())
}

/// Apply Studio filter rules to a DataModel tree received from the companion plugin.
pub fn filter_studio_tree(root: Node, filter: &crate::config::StudioFilterConfig) -> Option<Node> {
    filter_node(root, filter, None)
}

fn filter_node(
    node: Node,
    filter: &crate::config::StudioFilterConfig,
    parent_path: Option<&str>,
) -> Option<Node> {
    let Node {
        name,
        class_name,
        file_paths,
        children: in_children,
    } = node;

    let path = match parent_path {
        Some(p) if !p.is_empty() => format!("{p}.{name}"),
        _ => name.clone(),
    };
    let alt_path = path
        .strip_prefix("game.")
        .or_else(|| path.strip_prefix("DataModel."))
        .map(|s| s.to_string());

    if path_blacklisted(&path, alt_path.as_deref(), &filter.exclude) {
        return None;
    }

    let allowed_here = node_values_allowed(&name, &class_name, filter);
    let mut children = Vec::new();
    for child in in_children {
        if let Some(filtered) = filter_node(child, filter, Some(&path)) {
            children.push(filtered);
        }
    }

    let whitelisted_branch = path_whitelisted(&path, alt_path.as_deref(), &filter.include);
    let keep = if !filter.include.is_empty() {
        // Keep nodes that are inside/leading to whitelisted paths and satisfy
        // class/name rules, or keep ancestors that contain kept descendants.
        (allowed_here && whitelisted_branch.included)
            || (!children.is_empty() && whitelisted_branch.can_have_included_descendant)
    } else {
        allowed_here || !children.is_empty()
    };

    if !keep {
        return None;
    }
    if filter.prune_empty && children.is_empty() && !allowed_here {
        return None;
    }

    Some(Node {
        name,
        class_name,
        file_paths,
        children,
    })
}

#[derive(Clone, Copy, Debug)]
struct PathWhitelistState {
    included: bool,
    can_have_included_descendant: bool,
}

fn path_whitelisted(path: &str, alt: Option<&str>, include: &[String]) -> PathWhitelistState {
    if include.is_empty() {
        return PathWhitelistState {
            included: true,
            can_have_included_descendant: true,
        };
    }
    let mut included = false;
    let mut can_have_included_descendant = false;
    for raw in include {
        let inc = raw.trim().trim_matches('.');
        if inc.is_empty() {
            continue;
        }
        for candidate in [Some(path), alt] {
            let Some(candidate) = candidate else { continue };
            if candidate == inc || path_is_under(candidate, inc) {
                included = true;
            }
            if candidate == inc || path_is_under(candidate, inc) || path_is_under(inc, candidate) {
                can_have_included_descendant = true;
            }
        }
    }
    PathWhitelistState {
        included,
        can_have_included_descendant,
    }
}

fn path_blacklisted(path: &str, alt: Option<&str>, exclude: &[String]) -> bool {
    for raw in exclude {
        let ex = raw.trim().trim_matches('.');
        if ex.is_empty() {
            continue;
        }
        for candidate in [Some(path), alt] {
            let Some(candidate) = candidate else { continue };
            if candidate == ex || path_is_under(candidate, ex) {
                return true;
            }
        }
    }
    false
}

fn path_is_under(path: &str, prefix: &str) -> bool {
    path.len() > prefix.len()
        && path.starts_with(prefix)
        && path.as_bytes().get(prefix.len()) == Some(&b'.')
}

fn node_values_allowed(
    name: &str,
    class_name: &str,
    filter: &crate::config::StudioFilterConfig,
) -> bool {
    if !filter.include_class.is_empty() && !filter.include_class.iter().any(|c| c == class_name) {
        return false;
    }
    if filter.exclude_class.iter().any(|c| c == class_name) {
        return false;
    }
    if !filter.include_name.is_empty() && !filter.include_name.iter().any(|n| name_matches(name, n))
    {
        return false;
    }
    if filter.exclude_name.iter().any(|n| name_matches(name, n)) {
        return false;
    }
    true
}

fn name_matches(name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return name == pattern;
    }
    // Simple glob: '*' matches any sequence.
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return name == pattern;
    }
    let mut rest = name;
    if let Some(first) = parts.first() {
        if !first.is_empty() {
            if !rest.starts_with(first) {
                return false;
            }
            rest = &rest[first.len()..];
        }
    }
    for mid in parts.iter().skip(1).take(parts.len().saturating_sub(2)) {
        if mid.is_empty() {
            continue;
        }
        if let Some(pos) = rest.find(mid) {
            rest = &rest[pos + mid.len()..];
        } else {
            return false;
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() {
            return rest.ends_with(last);
        }
    }
    true
}

/// Process a full DataModel tree payload from the Studio companion plugin and
/// write the filtered sourcemap. Returns the output path on success.
pub fn apply_studio_tree(
    tree: Node,
    workspace: &Path,
    studio: &crate::config::StudioSourcemapConfig,
    assets_rel: &Path,
    verbose: bool,
) -> Result<Option<PathBuf>, String> {
    let mut filtered = filter_studio_tree(tree, &studio.filter);
    let out_path = if studio.output.is_absolute() {
        studio.output.clone()
    } else {
        workspace.join(&studio.output)
    };

    if let Some(ref mut node) = filtered {
        normalize_node(node);
        write_json(&out_path, node)?;
        if verbose {
            eprintln!("wrote filtered studio sourcemap {}", out_path.display());
        }
    } else {
        // Nothing matched: remove stale output so merges don't pick it up.
        if out_path.is_file() {
            let _ = fs::remove_file(&out_path);
        }
        if verbose {
            eprintln!(
                "studio sourcemap filter produced no nodes; cleared {}",
                out_path.display()
            );
        }
    }

    if studio.apply_to_root {
        // Merge from configured assets path (commonly same as studio.output).
        generate_root_sourcemap(workspace, assets_rel, verbose)?;
    }

    Ok(filtered.map(|_| out_path))
}

pub fn clear_studio_sourcemap(
    workspace: &Path,
    studio: &crate::config::StudioSourcemapConfig,
    assets_rel: &Path,
    verbose: bool,
) -> Result<(), String> {
    if !studio.clear_on_request {
        return Ok(());
    }
    let out_path = if studio.output.is_absolute() {
        studio.output.clone()
    } else {
        workspace.join(&studio.output)
    };
    if out_path.is_file() {
        fs::remove_file(&out_path)
            .map_err(|e| format!("cannot remove `{}`: {e}", out_path.display()))?;
        if verbose {
            eprintln!("cleared studio sourcemap {}", out_path.display());
        }
    }
    if studio.apply_to_root {
        generate_root_sourcemap(workspace, assets_rel, verbose)?;
    }
    Ok(())
}
