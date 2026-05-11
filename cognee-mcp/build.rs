use serde_json::json;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SERVER_NAME: &str = "cognee-mcp";
const BEGIN_MARKER: &str = "# BEGIN COGNEE_MCP_AUTOGEN";
const END_MARKER: &str = "# END COGNEE_MCP_AUTOGEN";

fn main() {
    print_rerun_rules();
    install_for_release_build();
}

fn print_rerun_rules() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=COGNEE_MCP_SKIP_AUTO_INSTALL");
    println!("cargo:rerun-if-env-changed=COGNEE_MCP_INSTALL_BIN_DIR");
    println!("cargo:rerun-if-env-changed=COGNEE_SERVICE_URL");
    println!("cargo:rerun-if-env-changed=COGNEE_MCP_READ_MODEL_PATH");
}

fn install_for_release_build() {
    if should_skip_install() {
        return;
    }

    if env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }

    if let Err(error) = install_mcp_configuration() {
        println!("cargo:warning=cognee MCP auto-install skipped: {error}");
    }
}

fn should_skip_install() -> bool {
    env::var("COGNEE_MCP_SKIP_AUTO_INSTALL")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn install_mcp_configuration() -> io::Result<()> {
    let manifest = install_manifest()?;
    create_launcher(&manifest)?;
    write_project_mcp_file(&manifest)?;
    configure_codex(&manifest)?;
    configure_claude(&manifest)?;
    println!(
        "cargo:warning=installed {SERVER_NAME} MCP launcher at {}",
        manifest.launcher.display()
    );
    Ok(())
}

fn install_manifest() -> io::Result<InstallManifest> {
    let home = home_directory()?;
    let repo = PathBuf::from(required_env("CARGO_MANIFEST_DIR")?);
    let release = release_directory()?;
    let launcher = install_bin_directory(&home).join(launcher_name());
    let executable = release.join(executable_name());
    let read_model = read_model_path(&repo);
    let service_url = service_url();
    Ok(InstallManifest {
        home,
        repo,
        launcher,
        executable,
        read_model,
        service_url,
    })
}

fn home_directory() -> io::Result<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

fn required_env(name: &str) -> io::Result<String> {
    env::var(name).map_err(|_| io::Error::new(io::ErrorKind::NotFound, name))
}

fn release_directory() -> io::Result<PathBuf> {
    let profile = required_env("PROFILE")?;
    let output = PathBuf::from(required_env("OUT_DIR")?);
    output
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new(&profile)))
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "release directory"))
}

fn install_bin_directory(home: &Path) -> PathBuf {
    env::var_os("COGNEE_MCP_INSTALL_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local").join("bin"))
}

fn launcher_name() -> &'static str {
    if cfg!(windows) {
        "cognee-mcp-rs.cmd"
    } else {
        "cognee-mcp-rs"
    }
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "cognee-mcp-rs.exe"
    } else {
        "cognee-mcp-rs"
    }
}

fn read_model_path(repo: &Path) -> PathBuf {
    env::var_os("COGNEE_MCP_READ_MODEL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join(".cognee").join("read_model.sqlite"))
}

fn service_url() -> String {
    env::var("COGNEE_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())
}

fn create_launcher(manifest: &InstallManifest) -> io::Result<()> {
    create_parent_directory(&manifest.launcher)?;
    let launcher = launcher_body(manifest);
    write_if_changed(&manifest.launcher, launcher.as_bytes())?;
    make_executable(&manifest.launcher)
}

fn launcher_body(manifest: &InstallManifest) -> String {
    if cfg!(windows) {
        windows_launcher(manifest)
    } else {
        unix_launcher(manifest)
    }
}

fn windows_launcher(manifest: &InstallManifest) -> String {
    format!("@echo off\r\n\"{}\" %*\r\n", manifest.executable.display())
}

fn unix_launcher(manifest: &InstallManifest) -> String {
    format!(
        "#!/usr/bin/env sh\nexec \"{}\" \"$@\"\n",
        manifest.executable.display()
    )
}

fn write_project_mcp_file(manifest: &InstallManifest) -> io::Result<()> {
    let path = manifest.repo.join(".mcp.json");
    let packet = json!({ "mcpServers": { SERVER_NAME: server_config(manifest) } });
    let body = json_bytes(&packet)?;
    write_if_changed(&path, &body)
}

fn server_config(manifest: &InstallManifest) -> serde_json::Value {
    json!({
        "type": "stdio",
        "command": manifest.launcher.display().to_string(),
        "args": [],
        "env": {
            "COGNEE_SERVICE_URL": manifest.service_url.clone(),
            "COGNEE_MCP_READ_MODEL_PATH": manifest.read_model.display().to_string()
        }
    })
}

fn configure_codex(manifest: &InstallManifest) -> io::Result<()> {
    let path = codex_config_path(manifest);
    let current = fs::read_to_string(&path).unwrap_or_default();
    let cleaned = remove_codex_sections(&remove_marked_block(&current));
    let next = append_codex_block(cleaned, manifest);
    write_with_backup(&path, next.as_bytes())
}

fn codex_config_path(manifest: &InstallManifest) -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.home.join(".codex"))
        .join("config.toml")
}

fn remove_marked_block(input: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    for line in input.lines() {
        update_marked_block(line, &mut skipping, &mut output);
    }
    output.trim_end().to_string()
}

fn update_marked_block(line: &str, skipping: &mut bool, output: &mut String) {
    if line.trim() == BEGIN_MARKER {
        *skipping = true;
    } else if line.trim() == END_MARKER {
        *skipping = false;
    } else if !*skipping {
        output.push_str(line);
        output.push('\n');
    }
}

fn remove_codex_sections(input: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    for line in input.lines() {
        skip_codex_section_line(line, &mut skipping, &mut output);
    }
    output.trim_end().to_string()
}

fn skip_codex_section_line(line: &str, skipping: &mut bool, output: &mut String) {
    if is_codex_server_header(line) {
        *skipping = true;
        return;
    }
    if is_other_header(line) {
        *skipping = false;
    }
    if !*skipping {
        output.push_str(line);
        output.push('\n');
    }
}

fn is_codex_server_header(line: &str) -> bool {
    line.trim().starts_with("[mcp_servers.cognee-mcp")
}

fn is_other_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && !is_codex_server_header(trimmed)
}

fn append_codex_block(mut current: String, manifest: &InstallManifest) -> String {
    if !current.is_empty() {
        current.push_str("\n\n");
    }
    current.push_str(&codex_block(manifest));
    current
}

fn codex_block(manifest: &InstallManifest) -> String {
    format!(
        "{BEGIN_MARKER}\n[mcp_servers.{SERVER_NAME}]\ncommand = {command}\nargs = []\ncwd = {cwd}\nenabled = true\n\n[mcp_servers.{SERVER_NAME}.env]\nCOGNEE_SERVICE_URL = {service}\nCOGNEE_MCP_READ_MODEL_PATH = {read_model}\n{END_MARKER}\n",
        command = toml_string(manifest.launcher.display()),
        cwd = toml_string(manifest.repo.display()),
        service = toml_string(&manifest.service_url),
        read_model = toml_string(manifest.read_model.display()),
    )
}

fn toml_string<T: ToString>(value: T) -> String {
    let value = value.to_string().replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{value}\"")
}

fn configure_claude(manifest: &InstallManifest) -> io::Result<()> {
    if configure_claude_with_cli(manifest) {
        return Ok(());
    }
    configure_claude_json(manifest)
}

fn configure_claude_with_cli(manifest: &InstallManifest) -> bool {
    let payload = serde_json::to_string(&server_config(manifest)).unwrap_or_default();
    Command::new("claude")
        .args([
            "mcp",
            "add-json",
            "-s",
            "user",
            SERVER_NAME,
            payload.as_str(),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn configure_claude_json(manifest: &InstallManifest) -> io::Result<()> {
    let path = manifest.home.join(".claude.json");
    let current = read_json_object(&path);
    let next = update_claude_mcp_server(current, manifest);
    let body = json_bytes(&next)?;
    write_with_backup(&path, &body)
}

fn json_bytes(value: &serde_json::Value) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_json_object(path: &Path) -> serde_json::Value {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}))
}

fn update_claude_mcp_server(
    mut current: serde_json::Value,
    manifest: &InstallManifest,
) -> serde_json::Value {
    ensure_json_object(&mut current);
    ensure_json_object(&mut current["mcpServers"]);
    current["mcpServers"][SERVER_NAME] = server_config(manifest);
    current
}

fn ensure_json_object(value: &mut serde_json::Value) {
    if !value.is_object() {
        *value = json!({});
    }
}

fn write_with_backup(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if file_matches(path, bytes) {
        return Ok(());
    }
    create_backup(path)?;
    write_if_changed(path, bytes)
}

fn create_backup(path: &Path) -> io::Result<()> {
    if path.exists() {
        let backup = path.with_extension(format!("bak-{}", unix_timestamp()));
        fs::copy(path, backup)?;
    }
    Ok(())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if file_matches(path, bytes) {
        return Ok(());
    }
    create_parent_directory(path)?;
    fs::write(path, bytes)
}

fn file_matches(path: &Path, bytes: &[u8]) -> bool {
    fs::read(path)
        .map(|current| current == bytes)
        .unwrap_or(false)
}

fn create_parent_directory(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

struct InstallManifest {
    home: PathBuf,
    repo: PathBuf,
    launcher: PathBuf,
    executable: PathBuf,
    read_model: PathBuf,
    service_url: String,
}
