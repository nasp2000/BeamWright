//! Restrictions an MCP session runs under.
//!
//! `--workspace <dir>` confines every path the agent sends to one folder and
//! `--read-only` refuses tools that change the document. The policy lives in the
//! front-end (stdio or HTTP), so it also applies when the requests are forwarded
//! to a running editor.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Tools that only read the document and are allowed in read-only mode.
pub const READ_ONLY_TOOLS: [&str; 12] = [
    "get_status",
    "list_objects",
    "list_layers",
    "list_parameters",
    "list_nodes",
    "list_checkpoints",
    "find_duplicates",
    "find_intersections",
    "inspect",
    "measure",
    "estimate",
    "check_job",
];

/// Workspace and read-only rules for one session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    workspace: Option<PathBuf>,
    read_only: bool,
}

impl Policy {
    /// Build a policy. The workspace must already exist.
    pub fn new(workspace: Option<PathBuf>, read_only: bool) -> Result<Self, String> {
        let workspace = match workspace {
            None => None,
            Some(path) => {
                let canonical = path
                    .canonicalize()
                    .map_err(|error| format!("cannot use workspace {}: {error}", path.display()))?;
                if !canonical.is_dir() {
                    return Err(format!(
                        "the workspace is not a folder: {}",
                        canonical.display()
                    ));
                }
                Some(canonical)
            }
        };
        Ok(Self {
            workspace,
            read_only,
        })
    }

    /// A session without restrictions.
    pub fn unrestricted() -> Self {
        Self::default()
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    /// Refuse a tool the policy does not allow.
    pub fn check_tool(&self, name: &str) -> Result<(), String> {
        if self.read_only && !READ_ONLY_TOOLS.contains(&name) {
            return Err(format!("'{name}' is not available in read-only mode"));
        }
        Ok(())
    }

    /// Resolve a path against the workspace and refuse it when it escapes.
    ///
    /// The parent folder is canonicalised so a symbolic link cannot be used to
    /// reach outside; the file itself need not exist yet.
    pub fn resolve(&self, path: &Path) -> Result<PathBuf, String> {
        let Some(workspace) = &self.workspace else {
            return Ok(path.to_path_buf());
        };
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace.join(path)
        };
        let parent = candidate
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(workspace);
        let canonical_parent = parent
            .canonicalize()
            .map_err(|error| format!("cannot reach {}: {error}", parent.display()))?;
        let name = candidate
            .file_name()
            .ok_or_else(|| format!("no file name in {}", candidate.display()))?;
        let resolved = canonical_parent.join(name);
        if !resolved.starts_with(workspace) {
            return Err(format!(
                "{} is outside the workspace {}",
                resolved.display(),
                workspace.display()
            ));
        }
        Ok(resolved)
    }

    /// Check and rewrite one JSON-RPC request.
    ///
    /// A `path` argument inside the workspace is replaced by its absolute
    /// resolved form, so an editor reached through a proxy opens the same file
    /// the policy approved.
    pub fn enforce(&self, request: &mut Value) -> Result<(), String> {
        if request.get("method").and_then(Value::as_str) != Some("tools/call") {
            return Ok(());
        }
        let Some(params) = request.get_mut("params") else {
            return Ok(());
        };
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.check_tool(&name)?;
        if let Some(arguments) = params.get_mut("arguments") {
            self.enforce_arguments(&name, arguments)?;
        }
        Ok(())
    }

    fn enforce_arguments(&self, tool: &str, arguments: &mut Value) -> Result<(), String> {
        let Some(object) = arguments.as_object_mut() else {
            return Ok(());
        };
        if self.workspace.is_some() && tool == "save_project" && object.get("path").is_none() {
            return Err(
                "save_project needs a path inside the workspace in this session".to_string(),
            );
        }
        if let Some(path) = object.get("path").and_then(Value::as_str) {
            let resolved = self.resolve(Path::new(path))?;
            object.insert(
                "path".to_string(),
                Value::String(resolved.to_string_lossy().into_owned()),
            );
        }
        if let Some(steps) = object.get_mut("steps").and_then(Value::as_array_mut) {
            for step in steps {
                let tool = step
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if let Some(arguments) = step.get_mut("arguments") {
                    self.enforce_arguments(&tool, arguments)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn temp_folder(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("beamwright_policy_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn an_open_policy_allows_everything() {
        let policy = Policy::unrestricted();
        assert!(policy.check_tool("add_rectangle").is_ok());
        assert_eq!(
            policy.resolve(Path::new("/tmp/example.bwp")).unwrap(),
            PathBuf::from("/tmp/example.bwp")
        );
    }

    #[test]
    fn a_workspace_confines_relative_and_absolute_paths() {
        let folder = temp_folder("inside");
        let policy = Policy::new(Some(folder.clone()), false).unwrap();
        let relative = policy.resolve(Path::new("part.bwp")).unwrap();
        assert_eq!(relative, folder.join("part.bwp"));
        let inside = folder.join("sub").join("part.svg");
        std::fs::create_dir_all(folder.join("sub")).unwrap();
        assert_eq!(policy.resolve(&inside).unwrap(), inside);
        let outside = Path::new("/tmp/elsewhere.bwp");
        assert!(policy.resolve(outside).is_err());
        assert!(policy.resolve(Path::new("../escape.bwp")).is_err());
        let _ = std::fs::remove_dir_all(folder);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_cannot_escape_the_workspace() {
        let folder = temp_folder("symlink");
        let outside = temp_folder("outside");
        std::os::unix::fs::symlink(&outside, folder.join("link")).unwrap();
        let policy = Policy::new(Some(folder.clone()), false).unwrap();
        assert!(policy.resolve(Path::new("link/file.bwp")).is_err());
        let _ = std::fs::remove_dir_all(folder);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn the_workspace_must_exist_and_be_a_folder() {
        assert!(Policy::new(Some(PathBuf::from("/no/such/folder")), false).is_err());
        let folder = temp_folder("file");
        let file = folder.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(Policy::new(Some(file.clone()), false).is_err());
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn read_only_refuses_changes_but_allows_inspection() {
        let policy = Policy::new(None, true).unwrap();
        assert!(policy.check_tool("inspect").is_ok());
        assert!(policy.check_tool("measure").is_ok());
        assert!(policy.check_tool("check_job").is_ok());
        assert!(policy.check_tool("add_rectangle").is_err());
        assert!(policy.check_tool("apply_batch").is_err());
        assert!(policy.check_tool("export_svg").is_err());
        assert!(policy.check_tool("save_project").is_err());
    }

    #[test]
    fn a_workspace_requires_a_save_path() {
        let folder = temp_folder("save");
        let policy = Policy::new(Some(folder.clone()), false).unwrap();
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "save_project", "arguments": {}}
        });
        assert!(policy.enforce(&mut request).is_err());
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "save_project", "arguments": {"path": "out.bwp"}}
        });
        assert!(policy.enforce(&mut request).is_ok());
        assert_eq!(
            request["params"]["arguments"]["path"],
            folder.join("out.bwp").to_string_lossy().as_ref()
        );
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn enforce_checks_tools_and_rewrites_paths() {
        let folder = temp_folder("enforce");
        let policy = Policy::new(Some(folder.clone()), true).unwrap();
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "check_job",
                "arguments": {"path": "part.bwp"}
            }
        });
        assert!(policy.enforce(&mut request).is_ok());
        assert_eq!(
            request["params"]["arguments"]["path"],
            folder.join("part.bwp").to_string_lossy().as_ref()
        );

        let mut blocked = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "add_rectangle", "arguments": {}}
        });
        assert!(policy.enforce(&mut blocked).is_err());

        let mut outside = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "check_job", "arguments": {"path": "/tmp/nope.bwp"}}
        });
        assert!(policy.enforce(&mut outside).is_err());

        let mut initialize = json!({"jsonrpc": "2.0", "id": 4, "method": "initialize"});
        assert!(policy.enforce(&mut initialize).is_ok());
        let _ = std::fs::remove_dir_all(folder);
    }
}
