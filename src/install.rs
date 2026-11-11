use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the transpiler installs itself by default.
pub fn default_install_dir() -> PathBuf {
    if cfg!(windows) {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("Luwu");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local").join("bin");
    }
    PathBuf::from(".")
}

/// The file name of the binary on this platform.
pub fn binary_name() -> &'static str {
    if cfg!(windows) { "luwu.exe" } else { "luwu" }
}

/// Copies `source` into `dir` and returns the installed path.
/// When `source` is already the installed binary, the copy is skipped.
pub fn install_binary(source: &Path, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("cannot create `{}`: {e}", dir.display()))?;

    let target = dir.join(binary_name());
    if same_path(source, &target) {
        return Ok(target);
    }

    std::fs::copy(source, &target).map_err(|e| {
        format!(
            "cannot copy `{}` to `{}`: {e}",
            source.display(),
            target.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&target) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target, perms);
        }
    }

    Ok(target)
}

/// Removes the installed binary and reports whether something was removed.
pub fn uninstall_binary(dir: &Path) -> Result<bool, String> {
    let target = dir.join(binary_name());
    if !target.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&target)
        .map_err(|e| format!("cannot remove `{}`: {e}", target.display()))?;
    Ok(true)
}

/// Adds `dir` to the user PATH. Returns `true` when the PATH was changed.
pub fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    if cfg!(windows) {
        let out = run_powershell(&windows_path_script(dir, true))?;
        Ok(out.lines().any(|l| l.trim() == "changed"))
    } else {
        Ok(unix_add_to_path(dir))
    }
}

/// Removes `dir` from the user PATH. Returns `true` when the PATH was changed.
pub fn remove_from_user_path(dir: &Path) -> Result<bool, String> {
    if cfg!(windows) {
        let out = run_powershell(&windows_path_script(dir, false))?;
        Ok(out.lines().any(|l| l.trim() == "changed"))
    } else {
        Ok(unix_remove_from_path(dir))
    }
}

/// Checks whether `dir` is already present in the user PATH.
pub fn is_in_user_path(dir: &Path) -> bool {
    if cfg!(windows) {
        let script = format!(
            "$p = [Environment]::GetEnvironmentVariable('Path','User'); \
             if (($p -split ';') -contains '{}') {{ 'yes' }} else {{ 'no' }}",
            escape_ps(&dir.display().to_string())
        );
        run_powershell(&script)
            .map(|out| out.lines().any(|l| l.trim() == "yes"))
            .unwrap_or(false)
    } else {
        std::env::var("PATH")
            .map(|p| std::env::split_paths(&p).any(|e| same_path(&e, dir)))
            .unwrap_or(false)
    }
}

/// A short hint shown after installation when the directory is not yet on PATH.
pub fn path_hint(dir: &Path) -> String {
    if cfg!(windows) {
        format!(
            "Open a new terminal, then run `luwu --version`. If it is still not found, add this folder to PATH:\n  {}",
            dir.display()
        )
    } else {
        format!(
            "Add this to your shell profile and restart the shell:\n  export PATH=\"{}:$PATH\"",
            dir.display()
        )
    }
}

fn windows_path_script(dir: &Path, add: bool) -> String {
    let dir = escape_ps(&dir.display().to_string());
    let action = if add {
        "if (($user -split ';') -notcontains $dir) { \
           $new = if ([string]::IsNullOrEmpty($user)) { $dir } else { $user.TrimEnd(';') + ';' + $dir }; \
           [Environment]::SetEnvironmentVariable('Path', $new, 'User'); 'changed' } else { 'kept' }"
    } else {
        "if (($user -split ';') -contains $dir) { \
           $kept = @($user -split ';' | Where-Object { $_ -and $_ -ne $dir }); \
           [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User'); 'changed' } else { 'kept' }"
    };

    format!(
        "$dir = '{dir}'; \
         $user = [Environment]::GetEnvironmentVariable('Path','User'); \
         {action}; \
         $src = @'\n\
using System;\n\
using System.Runtime.InteropServices;\n\
public class EnvBroadcast {{\n\
    [DllImport(\"user32.dll\", SetLastError=true, CharSet=CharSet.Auto)]\n\
    public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);\n\
}}\n\
'@; \
         Add-Type -TypeDefinition $src -ErrorAction SilentlyContinue; \
         $result = [UIntPtr]::Zero; \
         [EnvBroadcast]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 2000, [ref]$result) | Out-Null"
    )
}

fn escape_ps(value: &str) -> String {
    value.replace('\'', "''")
}

fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("cannot run powershell: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "powershell failed".to_string()
        } else {
            stderr
        });
    }
    Ok(stdout)
}

#[cfg(windows)]
fn unix_add_to_path(_dir: &Path) -> bool {
    false
}

#[cfg(windows)]
fn unix_remove_from_path(_dir: &Path) -> bool {
    false
}

#[cfg(not(windows))]
fn unix_add_to_path(_dir: &Path) -> bool {
    // On Unix the reliable place to add PATH is the user's shell profile,
    // which differs per shell; we report that it was not changed automatically
    // and the caller prints the hint.
    false
}

#[cfg(not(windows))]
fn unix_remove_from_path(_dir: &Path) -> bool {
    false
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
