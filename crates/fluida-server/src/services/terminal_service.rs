use crate::{error::ApiError, services::file_service::FileService};
use axum::http::StatusCode;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Component, Path},
    sync::Arc,
    time::Instant,
};
use tokio::sync::Mutex;

pub const COMMANDS: [&str; 20] = [
    "help", "pwd", "cd", "ls", "tree", "cat", "head", "tail", "mkdir", "rmdir", "touch", "write",
    "append", "cp", "mv", "rm", "find", "stat", "date", "clear",
];
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalJob {
    pub id: String,
    pub owner_id: String,
    pub command: String,
    pub cwd: String,
    pub state: Value,
    pub started_at: String,
    pub finished_at: Option<String>,
}
#[derive(Clone)]
pub struct TerminalService {
    files: FileService,
    jobs: Arc<Mutex<HashMap<String, TerminalJob>>>,
    timeout_ms: u128,
    output_limit: usize,
}
impl TerminalService {
    pub fn new(files: FileService) -> Self {
        Self {
            files,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            timeout_ms: 5000,
            output_limit: 65536,
        }
    }
    fn reject(message: impl Into<String>) -> ApiError {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "TERMINAL_COMMAND_REJECTED",
            message,
        )
    }
    fn tokens(command: &str) -> Result<Vec<String>, ApiError> {
        let mut out = vec![];
        let (mut value, mut quote) = (String::new(), None);
        let mut chars = command.chars();
        while let Some(c) = chars.next() {
            if let Some(q) = quote {
                if c == q {
                    quote = None
                } else if c == '\\' {
                    if let Some(n) = chars.next() {
                        value.push(n)
                    }
                } else {
                    value.push(c)
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c)
            } else if c.is_whitespace() {
                if !value.is_empty() {
                    out.push(std::mem::take(&mut value))
                }
            } else if c == '\\' {
                if let Some(n) = chars.next() {
                    value.push(n)
                }
            } else {
                value.push(c)
            }
        }
        if quote.is_some() {
            return Err(Self::reject("Unterminated quote"));
        }
        if !value.is_empty() {
            out.push(value)
        }
        Ok(out)
    }
    fn target(&self, cwd: &str, input: &str) -> Result<String, ApiError> {
        if Path::new(input).is_absolute() {
            return Err(Self::reject("Absolute paths are not allowed"));
        }
        let mut parts: Vec<String> = cwd
            .split('/')
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .collect();
        for c in Path::new(&input.replace('\\', "/")).components() {
            match c {
                Component::Normal(v) => parts.push(v.to_string_lossy().into()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if parts.pop().is_none() {
                        return Err(Self::reject("Path escapes OS storage"));
                    }
                }
                _ => return Err(Self::reject("Invalid path")),
            }
        }
        let result = parts.join("/");
        self.files
            .resolve(&result)
            .map_err(|_| Self::reject("Path escapes OS storage"))?;
        Ok(result)
    }
    fn count(name: &str, args: &[String], min: usize, max: usize) -> Result<(), ApiError> {
        if args.len() < min || args.len() > max {
            Err(Self::reject(format!("{name}: invalid argument count")))
        } else {
            Ok(())
        }
    }
    async fn tree(&self, root: &str) -> Result<String, ApiError> {
        let base = self.files.resolve(root)?;
        let mut lines = vec![if root.is_empty() {
            ".".into()
        } else {
            root.into()
        }];
        for item in walkdir::WalkDir::new(base)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let depth = item.depth();
            lines.push(format!(
                "{}{}",
                "  ".repeat(depth.saturating_sub(1)),
                item.file_name().to_string_lossy()
            ));
        }
        Ok(lines.join("\n"))
    }
    pub async fn list(&self, owner: &str) -> Vec<TerminalJob> {
        self.jobs
            .lock()
            .await
            .values()
            .filter(|j| j.owner_id == owner)
            .cloned()
            .collect()
    }
    pub async fn execute(&self, owner: &str, command: &str, cwd: &str) -> Result<Value, ApiError> {
        if owner.is_empty() || command.len() > 4096 {
            return Err(Self::reject("Invalid terminal request"));
        }
        let mut tokens = Self::tokens(command)?;
        if tokens.is_empty() {
            return Err(Self::reject("Command is required"));
        }
        let name = tokens.remove(0);
        if !COMMANDS.contains(&name.as_str()) {
            return Err(Self::reject(format!("Command not allowed: {name}")));
        }
        let cwd = self.target("", cwd)?;
        let id = uuid::Uuid::new_v4().to_string();
        let started = Instant::now();
        let job = TerminalJob {
            id: id.clone(),
            owner_id: owner.into(),
            command: command.into(),
            cwd: cwd.clone(),
            state: json!({"kind":"running"}),
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
        };
        self.jobs.lock().await.insert(id.clone(), job);
        let at = |v: &str| self.target(&cwd, v);
        let mut next = cwd.clone();
        let output: Result<String, ApiError> = async {
            Ok(match name.as_str() {
                "help" => {
                    Self::count(&name, &tokens, 0, 0)?;
                    format!(
                        "Restricted commands:\n{}\nAll paths are contained in FluidaOS storage.",
                        COMMANDS.join("  ")
                    )
                }
                "pwd" => {
                    Self::count(&name, &tokens, 0, 0)?;
                    format!("/{cwd}")
                }
                "cd" => {
                    Self::count(&name, &tokens, 0, 1)?;
                    let target = at(tokens.first().map(String::as_str).unwrap_or(""))?;
                    if !tokio::fs::metadata(self.files.resolve(&target)?)
                        .await?
                        .is_dir()
                    {
                        return Err(Self::reject("Not a directory"));
                    }
                    next = target;
                    String::new()
                }
                "ls" => {
                    Self::count(&name, &tokens, 0, 1)?;
                    self.files
                        .list(&at(tokens.first().map(String::as_str).unwrap_or(""))?)
                        .await?
                        .into_iter()
                        .map(|e| format!("{} {}", if e.is_directory { 'd' } else { '-' }, e.name))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                "tree" => {
                    Self::count(&name, &tokens, 0, 1)?;
                    self.tree(&at(tokens.first().map(String::as_str).unwrap_or(""))?)
                        .await?
                }
                "cat" => {
                    Self::count(&name, &tokens, 1, 1)?;
                    self.files.read(&at(&tokens[0])?).await?
                }
                "head" | "tail" => {
                    Self::count(&name, &tokens, 1, 2)?;
                    let count = tokens
                        .get(1)
                        .map(|v| v.parse::<usize>())
                        .transpose()
                        .map_err(|_| Self::reject("Invalid line count"))?
                        .unwrap_or(10);
                    if count > 10000 {
                        return Err(Self::reject("Line count must be between 0 and 10000"));
                    }
                    let rows = self
                        .files
                        .read(&at(&tokens[0])?)
                        .await?
                        .lines()
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    if name == "head" {
                        rows.into_iter().take(count).collect::<Vec<_>>().join("\n")
                    } else {
                        rows.into_iter()
                            .rev()
                            .take(count)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                }
                "mkdir" => {
                    Self::count(&name, &tokens, 1, 1)?;
                    self.files.mkdir(&at(&tokens[0])?).await?;
                    String::new()
                }
                "rmdir" => {
                    Self::count(&name, &tokens, 1, 2)?;
                    if tokens.get(1).is_some_and(|v| v != "--recursive") {
                        return Err(Self::reject("rmdir: usage PATH [--recursive]"));
                    }
                    self.files
                        .delete(&at(&tokens[0])?, tokens.get(1).is_some())
                        .await?;
                    String::new()
                }
                "touch" => {
                    Self::count(&name, &tokens, 1, 1)?;
                    self.files.touch(&at(&tokens[0])?).await?;
                    String::new()
                }
                "write" | "append" => {
                    Self::count(&name, &tokens, 2, usize::MAX)?;
                    let path = at(&tokens[0])?;
                    let text = tokens[1..].join(" ");
                    if name == "write" {
                        self.files.write(&path, text.as_bytes()).await?
                    } else {
                        self.files.append(&path, &text).await?
                    }
                    String::new()
                }
                "cp" => {
                    Self::count(&name, &tokens, 2, 2)?;
                    self.files.copy(&at(&tokens[0])?, &at(&tokens[1])?).await?;
                    String::new()
                }
                "mv" => {
                    Self::count(&name, &tokens, 2, 2)?;
                    self.files
                        .rename(&at(&tokens[0])?, &at(&tokens[1])?)
                        .await?;
                    String::new()
                }
                "rm" => {
                    Self::count(&name, &tokens, 1, 1)?;
                    let path = at(&tokens[0])?;
                    if self.files.metadata(&path).await?.is_directory {
                        return Err(Self::reject(
                            "rm only removes files; use rmdir for directories",
                        ));
                    }
                    self.files.delete(&path, false).await?;
                    String::new()
                }
                "find" => {
                    Self::count(&name, &tokens, 1, 2)?;
                    let (dir, q) = if tokens.len() == 2 {
                        (at(&tokens[0])?, tokens[1].as_str())
                    } else {
                        (cwd.clone(), tokens[0].as_str())
                    };
                    self.files
                        .search(&dir, q, 200)
                        .await?
                        .into_iter()
                        .map(|v| v.entry.path)
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                "stat" => {
                    Self::count(&name, &tokens, 1, 1)?;
                    let e = self.files.metadata(&at(&tokens[0])?).await?;
                    format!(
                        "Path: {}\nType: {}\nSize: {}\nModified: {}\nMIME: {}",
                        e.path,
                        if e.is_directory { "directory" } else { "file" },
                        e.size,
                        e.modified_at,
                        e.mime_type
                    )
                }
                "date" => {
                    Self::count(&name, &tokens, 0, 0)?;
                    Utc::now().to_rfc3339()
                }
                "clear" => {
                    Self::count(&name, &tokens, 0, 0)?;
                    String::new()
                }
                _ => unreachable!(),
            })
        }
        .await;
        let mut jobs = self.jobs.lock().await;
        let stored = jobs.get_mut(&id).unwrap();
        stored.finished_at = Some(Utc::now().to_rfc3339());
        match output {
            Ok(stdout) => {
                if stdout.len() > self.output_limit {
                    return Err(Self::reject("Command output limit exceeded"));
                }
                let state = if started.elapsed().as_millis() > self.timeout_ms {
                    json!({"kind":"timed-out"})
                } else {
                    json!({"kind":"completed","exitCode":0})
                };
                stored.state = state.clone();
                Ok(
                    json!({"jobId":id,"stdout":stdout,"stderr":"","exitCode":0,"duration":started.elapsed().as_millis(),"cwd":next,"state":state}),
                )
            }
            Err(e) => {
                stored.state = json!({"kind":"failed","exitCode":null,"reason":e.message});
                Err(e)
            }
        }
    }
    pub async fn terminate(&self, owner: &str, id: &str) -> Result<TerminalJob, ApiError> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.get_mut(id).ok_or_else(ApiError::not_found)?;
        if job.owner_id != owner {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Terminal job belongs to another session",
            ));
        }
        if job.state.get("kind").and_then(Value::as_str) != Some("running") {
            return Err(Self::reject("Terminal job is not running"));
        }
        job.state = json!({"kind":"cancelled"});
        job.finished_at = Some(Utc::now().to_rfc3339());
        Ok(job.clone())
    }
}
