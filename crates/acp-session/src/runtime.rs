//! One daemon-owned, bounded direct child. No persistence or automatic launch.
use crate::{AuthStatus, Capabilities, Permission, SESSION_API, State, View};
use acp_ipc::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{mpsc, oneshot},
};
const FRAME: usize = 65536;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn clean(s: &str, n: usize) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(n)
        .collect()
}
fn bounded(s: &str, n: usize) -> bool {
    !s.is_empty()
        && s.len() <= n
        && !s.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Program {
    id: String,
    executable: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    version: u32,
    programs: Vec<Program>,
}
#[derive(Default)]
struct Inner {
    view: View,
    tx: Option<mpsc::Sender<Control>>,
    next: u64,
    context: Option<Arc<acp_ipc::session_lease::SessionLease>>,
    context_enabled: bool,
}
#[derive(Clone, Default)]
pub struct Supervisor {
    inner: Arc<Mutex<Inner>>,
    programs: Arc<BTreeMap<String, Program>>,
    bridge: Option<crate::bridge::Bridge>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub session_api_version: u32,
    pub project: String,
    pub session_id: Option<String>,
    pub executable: Option<String>,
    pub confirmed: Option<bool>,
    pub prompt: Option<String>,
    pub request_id: Option<String>,
    pub option_id: Option<String>,
    pub share_context: Option<bool>,
}
struct Control {
    method: String,
    params: Params,
    ack: oneshot::Sender<Result<()>>,
}
impl Supervisor {
    /// The operator's session configuration (TOML, version 1). No path, or a
    /// path where no file exists, disables execution; a present file that is
    /// invalid, shared, oversized or inside a project root refuses.
    pub fn configured(path: Option<&Path>, roots: &[PathBuf]) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        if !path.is_absolute() {
            return Err(Error::InvalidRequest);
        }
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            _ => {}
        }
        let canonical = path.canonicalize().map_err(|_| Error::Unavailable)?;
        if roots.iter().any(|r| canonical.starts_with(r)) {
            return Err(Error::UnsafeRuntime);
        }
        acp_ipc::transport::private_file(path)?;
        let file = acp_ipc::transport::open_private(path, false)?;
        if file.metadata().map_err(|_| Error::Unavailable)?.len() > 65536 {
            return Err(Error::TooLarge);
        }
        use std::io::Read;
        let mut raw = Vec::new();
        file.take(65537)
            .read_to_end(&mut raw)
            .map_err(|_| Error::Unavailable)?;
        if raw.len() > 65536 {
            return Err(Error::TooLarge);
        }
        let text = std::str::from_utf8(&raw).map_err(|_| Error::InvalidRequest)?;
        let c: Config = toml::from_str(text).map_err(|_| Error::InvalidRequest)?;
        if c.version != 1 || c.programs.len() > 8 {
            return Err(Error::Version);
        }
        let mut programs = BTreeMap::new();
        for mut p in c.programs {
            if !acp_ipc::valid_project(&p.id)
                || !p.executable.is_absolute()
                || p.args.len() > 16
                || p.args
                    .iter()
                    .any(|a| a.len() > 1024 || a.chars().any(char::is_control))
                || p.env.len() > 16
            {
                return Err(Error::InvalidRequest);
            }
            p.executable = p
                .executable
                .canonicalize()
                .map_err(|_| Error::Unavailable)?;
            if roots.iter().any(|r| p.executable.starts_with(r)) {
                return Err(Error::UnsafeRuntime);
            }
            if !p.executable.is_file() {
                return Err(Error::InvalidRequest);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if p.executable
                    .metadata()
                    .map_err(|_| Error::Unavailable)?
                    .permissions()
                    .mode()
                    & 0o111
                    == 0
                {
                    return Err(Error::InvalidRequest);
                }
            }
            #[cfg(windows)]
            if !p
                .executable
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
            {
                return Err(Error::InvalidRequest);
            }
            let allowed = [
                "PATH",
                "HOME",
                "USERPROFILE",
                "APPDATA",
                "LOCALAPPDATA",
                "SystemRoot",
                "WINDIR",
                "TEMP",
                "TMP",
                "TMPDIR",
                "LANG",
                "LC_ALL",
                "TERM",
                "NO_COLOR",
            ];
            if p.env.iter().any(|(k, v)| {
                !allowed.contains(&k.as_str()) || v.len() > 4096 || v.contains(['\0', '\n', '\r'])
            }) {
                return Err(Error::InvalidRequest);
            }
            if programs.insert(p.id.clone(), p).is_some() {
                return Err(Error::InvalidRequest);
            }
        }
        Ok(Self {
            programs: Arc::new(programs),
            ..Default::default()
        })
    }
    pub fn with_context_bridge(
        mut self,
        path: Option<&Path>,
        location: acp_ipc::Location,
        registry: acp_ipc::session_lease::Registry,
        ttl: Duration,
        roots: &[PathBuf],
    ) -> Result<Self> {
        self.bridge = path
            .map(|p| crate::bridge::Bridge::new(p, location, registry, ttl, roots))
            .transpose()?;
        Ok(self)
    }
    pub fn context(&self, project: &str) -> Value {
        let i = self.inner.lock().unwrap();
        if i.view.project != project {
            return Value::Null;
        }
        i.context.as_ref().map(|lease| lease.observation()).unwrap_or_else(|| json!({"enabled":i.context_enabled,"active":false,"context_calls":0,"handoff_calls":0}))
    }
    pub fn invalidate_context(&self, project: &str) {
        let lease = {
            let i = self.inner.lock().unwrap();
            (i.view.project == project)
                .then(|| i.context.clone())
                .flatten()
        };
        if let Some(lease) = lease {
            lease.revoke();
        }
    }
    fn revoke_context(&self) {
        let lease = self.inner.lock().unwrap().context.clone();
        if let Some(lease) = lease {
            lease.revoke();
        }
    }
    fn expire_context(&self) {
        let lease = self.inner.lock().unwrap().context.clone();
        if let Some(lease) = lease
            && lease.expired()
        {
            lease.revoke();
        }
    }
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            session_api_version: SESSION_API,
            protocol_version: 1,
            executables: self.programs.keys().cloned().collect(),
            resume_supported: false,
        }
    }
    pub fn get(&self, project: &str) -> Option<View> {
        let i = self.inner.lock().unwrap();
        (i.view.project == project).then(|| i.view.clone())
    }
    fn change(&self, f: impl FnOnce(&mut View)) {
        let mut i = self.inner.lock().unwrap();
        f(&mut i.view);
        i.view.last_event_ms = now();
        i.view.revision += 1;
    }
    fn reject(&self, stage: &'static str, error: Error) -> Error {
        self.event(format!("ACP rejected: {stage}"));
        error
    }
    fn event(&self, s: String) {
        self.change(|v| {
            if v.events.len() == 64 {
                v.events.remove(0);
            }
            v.events.push(clean(&s, 512));
        });
    }
    pub fn start(&self, params: Params, root: PathBuf) -> Result<View> {
        if params.confirmed != Some(true)
            || params.session_id.is_some()
            || params.prompt.is_some()
            || params.request_id.is_some()
            || params.option_id.is_some()
        {
            return Err(Error::InvalidRequest);
        }
        let program = self
            .programs
            .get(params.executable.as_deref().ok_or(Error::InvalidRequest)?)
            .ok_or(Error::Unauthorized)?
            .clone();
        if program.executable.canonicalize().ok().as_ref() != Some(&program.executable)
            || !program.executable.is_file()
        {
            return Err(Error::UnsafeRuntime);
        }
        if root.to_str().is_none()
            || !root.is_absolute()
            || !root.is_dir()
            || root.canonicalize().ok().as_ref() != Some(&root)
        {
            return Err(Error::UnsafeRuntime);
        }
        let share = params.share_context == Some(true);
        if share && self.bridge.is_none() {
            return Err(Error::Unavailable);
        }
        let mut i = self.inner.lock().unwrap();
        if i.view.process_alive || i.tx.is_some() {
            return Err(Error::Busy);
        }
        let next = i.next.checked_add(1).ok_or(Error::Unavailable)?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
        let old = i.context.take();
        i.context_enabled = share;
        i.next = next;
        let id = format!(
            "session-{}-{}",
            i.next,
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        i.view = View {
            session_api_version: SESSION_API,
            project: params.project,
            session_id: id,
            executable: program.id.clone(),
            state: State::Starting,
            process_alive: true,
            last_event_ms: now(),
            revision: 1,
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel(16);
        i.tx = Some(tx);
        let view = i.view.clone();
        drop(i);
        // Reserve Starting and its control channel atomically, then retire the
        // previous lease without blocking session readers on credential-file I/O.
        if let Some(old) = old {
            old.revoke();
        }
        let owner = self.clone();
        tokio::spawn(async move {
            owner.run(program, root, rx, share).await;
        });
        Ok(view)
    }
    pub async fn control(&self, method: &str, params: Params) -> Result<View> {
        if params.share_context.is_some()
            || params.executable.is_some()
            || params.confirmed.is_some()
        {
            return Err(Error::InvalidRequest);
        }
        let tx = {
            let i = self.inner.lock().unwrap();
            if i.view.project != params.project
                || Some(&i.view.session_id) != params.session_id.as_ref()
            {
                return Err(Error::NotFound);
            }
            i.tx.clone().ok_or(Error::Unavailable)?
        };
        let project = params.project.clone();
        let (ack, rx) = oneshot::channel();
        tx.try_send(Control {
            method: method.into(),
            params,
            ack,
        })
        .map_err(|_| Error::Busy)?;
        tokio::time::timeout(Duration::from_secs(2), rx)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Unavailable)??;
        self.get(&project).ok_or(Error::NotFound)
    }
    pub async fn shutdown(&self) {
        self.revoke_context();
        let v = self.inner.lock().unwrap().view.clone();
        if v.process_alive {
            let p = Params {
                session_api_version: SESSION_API,
                project: v.project,
                session_id: Some(v.session_id),
                executable: None,
                confirmed: None,
                prompt: None,
                request_id: None,
                option_id: None,
                share_context: None,
            };
            let _ = self.control("session.stop", p).await;
            for _ in 0..100 {
                if !self.inner.lock().unwrap().view.process_alive {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    async fn run(&self, p: Program, root: PathBuf, rx: mpsc::Receiver<Control>, share: bool) {
        let mut command = Command::new(&p.executable);
        command
            .args(&p.args)
            .env_clear()
            .envs(&p.env)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let reaped = match command.spawn() {
            Ok(mut child) => {
                let servers = if share {
                    let (project, generation) = {
                        let i = self.inner.lock().unwrap();
                        (i.view.project.clone(), i.view.session_id.clone())
                    };
                    self.bridge
                        .as_ref()
                        .ok_or(Error::Unavailable)
                        .and_then(|b| b.issue(&project, &generation, &p.id))
                        .map(|(lease, servers)| {
                            self.inner.lock().unwrap().context = Some(Arc::new(lease));
                            servers
                        })
                } else {
                    Ok(json!([]))
                };
                match servers {
                    Ok(servers) => self.drive(child, root, rx, servers).await,
                    Err(_) => {
                        self.change(|v| {
                            v.state = State::Failed;
                            v.failure = Some("Context bridge unavailable".into());
                        });
                        let _ = child.start_kill();
                        matches!(
                            tokio::time::timeout(Duration::from_secs(2), child.wait()).await,
                            Ok(Ok(_))
                        )
                    }
                }
            }
            Err(_) => {
                self.change(|v| {
                    v.state = State::Failed;
                    v.failure = Some("Launch unavailable".into());
                });
                true
            }
        };
        self.revoke_context();
        let mut i = self.inner.lock().unwrap();
        i.view.process_alive = !reaped;
        i.view.permissions.clear();
        i.tx = None;
        i.view.revision += 1;
        i.view.last_event_ms = now();
    }
    async fn drive(
        &self,
        mut child: Child,
        root: PathBuf,
        mut commands: mpsc::Receiver<Control>,
        servers: Value,
    ) -> bool {
        let mut input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (wire, mut frames) = mpsc::channel(16);
        let reader = tokio::spawn(async move {
            let mut r = BufReader::new(output);
            loop {
                let result = read_line(&mut r).await;
                let done = result.is_err();
                if wire.send(result).await.is_err() || done {
                    break;
                }
            }
        });
        let result = self
            .conversation(
                &mut child,
                &mut input,
                &root,
                &mut commands,
                &mut frames,
                servers,
            )
            .await;
        self.revoke_context();
        reader.abort();
        let _ = reader.await;
        if let Err(e) = result {
            self.change(|v| {
                if v.state == State::AuthRequired {
                    return;
                }
                v.state = State::Failed;
                if v.failure.is_none() {
                    v.failure = Some(e.to_string());
                }
            });
        }
        let _ = child.start_kill();
        let reaped = matches!(
            tokio::time::timeout(Duration::from_secs(2), child.wait()).await,
            Ok(Ok(_))
        );
        self.change(|v| {
            v.process_alive = !reaped;
            v.permissions.clear();
            if !reaped {
                v.state = State::Failed;
                v.failure = Some("Child cleanup not confirmed".into());
            } else if v.state == State::Stopping {
                v.state = State::Stopped;
            }
        });
        reaped
    }
    async fn conversation(
        &self,
        child: &mut Child,
        input: &mut ChildStdin,
        root: &Path,
        commands: &mut mpsc::Receiver<Control>,
        frames: &mut mpsc::Receiver<Result<Value>>,
        servers: Value,
    ) -> Result<()> {
        let mut rpc = 1u64;
        let mut waiting = Some((
            rpc,
            "initialize",
            tokio::time::Instant::now() + Duration::from_secs(5),
        ));
        send(input,json!({"jsonrpc":"2.0","id":rpc,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"agent-control-plane","version":"0.1.0"}}})).await?;
        let mut session = String::new();
        let mut seen = HashSet::new();
        let mut early_metadata: Vec<(String, String)> = Vec::new();
        let mut pending: BTreeMap<String, (Value, String, u64)> = BTreeMap::new();
        let mut total = 0;
        let mut permission_serial = 0;
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        loop {
            tokio::select! {
                _=tick.tick()=>{
                    self.expire_context();
                    if waiting.as_ref().is_some_and(|(_,_,t)|tokio::time::Instant::now()>=*t){return Err(Error::Timeout)}
                    let expired:Vec<_>=pending.iter().filter(|(_,(_,_,t))|now()>=*t).map(|(id,_)|id.clone()).collect();
                    for id in expired {let(raw,_,_)=pending.remove(&id).unwrap();send(input,denied(raw)).await?;self.event("Permission expired: denied".into());}
                    self.sync_permissions(&pending);
                },
                exit=child.wait()=>{let success=exit.is_ok_and(|s|s.success());let state=self.inner.lock().unwrap().view.state.clone();if success&&state==State::Completed{return Ok(())}return Err(Error::Unavailable)},
                Some(c)=commands.recv()=>{
                if c.ack.is_closed(){continue}
                    let view=self.inner.lock().unwrap().view.clone();
                    let turn=waiting.as_ref().is_some_and(|(_,method,_)|*method=="session/prompt");
                    let mut stop=false;
                    let valid=match c.method.as_str(){
                        "session.prompt"=> c.params.request_id.is_none()&&c.params.option_id.is_none()&&c.params.prompt.as_ref().is_some_and(|s|bounded(s,4096))&&matches!(view.state,State::Ready|State::Completed)&&waiting.is_none(),
                        "session.cancel"=>c.params.prompt.is_none()&&c.params.request_id.is_none()&&c.params.option_id.is_none()&&turn,
                        "session.stop"=>c.params.prompt.is_none()&&c.params.request_id.is_none()&&c.params.option_id.is_none(),
                        "session.permission"=>c.params.prompt.is_none()&&c.params.request_id.as_ref().is_some_and(|id|pending.get(id).is_some_and(|(_,allowed,t)|now()<*t&&(c.params.option_id.is_none()||(!allowed.is_empty() && c.params.option_id.as_ref()==Some(allowed))))),
                        _=>false,
                    };
                    if !valid{let _=c.ack.send(Err(Error::InvalidRequest));continue}
                    match c.method.as_str(){
                        "session.prompt"=>{rpc+=1;send(input,json!({"jsonrpc":"2.0","id":rpc,"method":"session/prompt","params":{"sessionId":session,"prompt":[{"type":"text","text":c.params.prompt.unwrap()}]}})).await?;waiting=Some((rpc,"session/prompt",tokio::time::Instant::now()+Duration::from_secs(120)));self.change(|v|{v.state=State::Running;v.stop_reason=None;});},
                        "session.cancel"|"session.stop"=>{
                            self.revoke_context();
                            for(_,(id,_,_))in std::mem::take(&mut pending){send(input,denied(id)).await?;}
                            self.change(|v|v.permissions.clear());
                            if !session.is_empty(){send(input,json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}})).await?;}
                            if c.method=="session.stop"{self.change(|v|v.state=State::Stopping);stop=true}else if let Some((_,_,deadline))=&mut waiting{*deadline=tokio::time::Instant::now()+Duration::from_secs(5);self.change(|v|v.state=State::Running);}
                        },
                        "session.permission"=>{let (raw,_,_)=pending.remove(c.params.request_id.as_ref().unwrap()).unwrap();let response=match c.params.option_id{Some(option)=>json!({"jsonrpc":"2.0","id":raw,"result":{"outcome":{"outcome":"selected","optionId":option}}}),None=>denied(raw)};send(input,response).await?;self.sync_permissions(&pending);self.event("Operator permission decision applied".into());},
                        _=>unreachable!(),
                    }
                    let _=c.ack.send(Ok(()));if stop{return Ok(())}
                },
                message=frames.recv()=>{
                    let m=message.ok_or(Error::Unavailable)??;total+=1;if total>4096{return Err(Error::TooLarge)}
                    let o=m.as_object().ok_or(Error::InvalidRequest)?;
                    if o.get("jsonrpc")!=Some(&json!("2.0")) || o.keys().any(|k|!["jsonrpc","id","method","params","result","error"].contains(&k.as_str())){return Err(self.reject("envelope",Error::InvalidRequest))}
                    if let Some(method)=m.get("method"){
                        if m.get("result").is_some()||m.get("error").is_some(){return Err(Error::InvalidRequest)}
                        let method=method.as_str().ok_or(Error::InvalidRequest)?;let params=m.get("params").ok_or(Error::InvalidRequest)?;
                        if let Some(id)=m.get("id") {
                            if !valid_id(id)||!seen.insert(id.to_string())||seen.len()>1024{return Err(Error::InvalidRequest)}
                            if method!="session/request_permission" {send(input,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Unsupported client method"}})).await?;continue}
                            if params.get("sessionId").and_then(Value::as_str)!=Some(session.as_str())||session.is_empty(){return Err(Error::InvalidRequest)}
                            let turn=waiting.as_ref().is_some_and(|(_,method,_)|*method=="session/prompt");
                            let operation=params.get("toolCall").map(Value::to_string).unwrap_or_default();
                            let options=params.get("options").and_then(Value::as_array);
                            let mut option_ids=HashSet::new();
                            let valid_options=options.is_some_and(|a|!a.is_empty()&&a.len()<=8&&a.iter().all(|p|p.get("optionId").and_then(Value::as_str).is_some_and(|s|bounded(s,64)&&option_ids.insert(s.to_owned()))&&p.get("name").and_then(Value::as_str).is_some_and(|s|bounded(s,120))&&p.get("kind").and_then(Value::as_str).is_some_and(|s|["allow_once","allow_always","reject_once","reject_always"].contains(&s))));
                            let call=params.get("toolCall");
                            if !turn||pending.len()>=4||!valid_options||!bounded(&operation,2048)||call.and_then(|v|v.get("toolCallId")).and_then(Value::as_str).is_none_or(|s|!bounded(s,128))||call.and_then(|v|v.get("title")).and_then(Value::as_str).is_none_or(|s|!bounded(s,256)){send(input,denied(id.clone())).await?;self.event("Unsupported permission request: denied".into());continue}
                            let allow=options.unwrap().iter().find(|o|o["kind"]=="allow_once").and_then(|o|o["optionId"].as_str()).map(str::to_owned).unwrap_or_default();
                            permission_serial+=1;let local=format!("permission-{permission_serial}");let expires=now()+30000;
                            pending.insert(local.clone(),(id.clone(),allow.clone(),expires));
                            self.change(|v|{v.permissions.push(Permission{request_id:local,operation,allow_once:(!allow.is_empty()).then_some(allow),expires_ms:expires});v.state=State::WaitingPermission;});
                        }else if method=="session/update"{
                            let observed=params.get("sessionId").and_then(Value::as_str).filter(|s|bounded(s,128)).ok_or(Error::InvalidRequest)?;
                            let update=params.get("update").ok_or(Error::InvalidRequest)?;let kind=update.get("sessionUpdate").and_then(Value::as_str).ok_or(Error::InvalidRequest)?;
                            if session.is_empty() {
                                if !waiting.as_ref().is_some_and(|(_,m,_)|*m=="session/new") || !crate::handshake::is_metadata(kind) { return Err(Error::InvalidRequest); }
                                if early_metadata.len()>=8 { return Err(Error::TooLarge); }
                                early_metadata.push((observed.to_owned(),kind.to_owned()));continue;
                            }
                            if observed!=session { return Err(self.reject("update scope",Error::InvalidRequest)); }
                            if crate::handshake::is_metadata(kind) { self.event(format!("{kind}: observed"));continue; }
                            if !waiting.as_ref().is_some_and(|(_,m,_)|*m=="session/prompt") { return Err(self.reject("update outside prompt",Error::InvalidRequest)); }
                            let message=match kind {"agent_message_chunk"|"agent_thought_chunk"|"user_message_chunk"=>{if update["content"]["type"]!="text"{return Err(self.reject("non-text message",Error::Unsupported))}format!("{kind}: {}",update["content"]["text"].as_str().ok_or(Error::InvalidRequest)?)},"tool_call"|"tool_call_update"=>format!("{kind}: {}",update.get("status").and_then(Value::as_str).unwrap_or("observed")),"plan"|"available_commands_update"|"current_mode_update"|"config_option_update"|"usage_update"|"session_info_update"=>format!("{kind}: observed"),_=>return Err(self.reject("update kind",Error::Unsupported))};
                            self.change(|v| {
                                if kind=="agent_message_chunk" { v.activity.response_chunks+=1;v.activity.response_characters+=update["content"]["text"].as_str().unwrap().chars().count() as u64; }
                                if matches!(kind,"tool_call"|"tool_call_update") { v.activity.tool_events+=1; }
                            });
                            self.event(message);
                        }else{return Err(self.reject("notification method",Error::Unsupported))}
                    }else{
                        let(id,method,_)=waiting.take().ok_or(Error::InvalidRequest)?;
                        if m.get("id")!=Some(&json!(id)) || m.get("result").is_some()==m.get("error").is_some(){return Err(self.reject("response correlation",Error::InvalidRequest))}
                        if let Some(error)=m.get("error"){if error.get("code").and_then(Value::as_i64)==Some(-32000){self.change(|v|{v.state=State::AuthRequired;v.authentication.status=AuthStatus::Required;v.failure=None;v.permissions.clear();});return Err(Error::Unauthorized)}return Err(self.reject("agent error",Error::Unavailable))}
                        let result=m.get("result").filter(|v|v.is_object()).ok_or(Error::InvalidRequest)?;
                        match method {
                            "initialize"=>{let observed=crate::handshake::initialize(result)?;self.change(|v|{v.agent=observed.agent;v.load_session_advertised=observed.load_session;v.agent_features=observed.features;v.authentication.methods=observed.methods;});rpc+=1;send(input,json!({"jsonrpc":"2.0","id":rpc,"method":"session/new","params":{"cwd":root,"mcpServers":servers}})).await?;waiting=Some((rpc,"session/new",tokio::time::Instant::now()+Duration::from_secs(5)));},
                            "session/new"=>{session=result["sessionId"].as_str().filter(|s|bounded(s,128)).ok_or(Error::InvalidRequest)?.into();if early_metadata.iter().any(|(id,_)|*id!=session){return Err(Error::InvalidRequest)}self.change(|v|{v.state=State::Ready;v.authentication.status=AuthStatus::SessionAccepted;});self.event("ACP v1 session ready".into());for(_,kind)in std::mem::take(&mut early_metadata){self.event(format!("{kind}: observed"));}},
                            "session/prompt"=>{if !pending.is_empty(){return Err(Error::InvalidRequest)}let reason=result["stopReason"].as_str().filter(|s|["end_turn","cancelled","max_tokens","max_turn_requests","refusal"].contains(s)).ok_or(Error::InvalidRequest)?;self.change(|v|{v.state=State::Completed;v.stop_reason=Some(reason.into());});self.event(format!("Turn ended: {reason}"));},
                            _=>return Err(Error::Unsupported),
                        }
                    }
                }
            }
        }
    }
    fn sync_permissions(&self, pending: &BTreeMap<String, (Value, String, u64)>) {
        let mut i = self.inner.lock().unwrap();
        let before = i.view.permissions.len();
        i.view
            .permissions
            .retain(|p| pending.contains_key(&p.request_id));
        if i.view.state == State::WaitingPermission && pending.is_empty() {
            i.view.state = State::Running;
            i.view.revision += 1;
        }
        if before != i.view.permissions.len() {
            i.view.revision += 1;
            i.view.last_event_ms = now();
        }
    }
}
fn valid_id(id: &Value) -> bool {
    id.as_str().is_some_and(|s| bounded(s, 64)) || id.as_u64().is_some() || id.as_i64().is_some()
}
fn denied(id: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":{"outcome":{"outcome":"cancelled"}}})
}
async fn send(input: &mut ChildStdin, v: Value) -> Result<()> {
    let mut raw = serde_json::to_vec(&v).map_err(|_| Error::InvalidRequest)?;
    if raw.len() > FRAME {
        return Err(Error::TooLarge);
    }
    raw.push(b'\n');
    tokio::time::timeout(Duration::from_secs(5), input.write_all(&raw))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Unavailable)
}
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R) -> Result<Value> {
    let mut raw = Vec::new();
    loop {
        let b = r.fill_buf().await.map_err(|_| Error::Unavailable)?;
        if b.is_empty() {
            return Err(Error::Unavailable);
        }
        let n = b
            .iter()
            .position(|c| *c == b'\n')
            .map(|n| n + 1)
            .unwrap_or(b.len());
        if raw.len() + n > FRAME {
            return Err(Error::TooLarge);
        }
        let done = b[n - 1] == b'\n';
        raw.extend_from_slice(&b[..n]);
        r.consume(n);
        if done {
            return serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest);
        }
    }
}

#[cfg(all(test, unix))]
mod config_tests {
    use super::*;
    use std::io::Write;
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        acp_ipc::transport::private_dir(&d, true).unwrap();
        d
    }
    fn write(path: &Path, text: &str) {
        let mut f = acp_ipc::transport::new_private(path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }
    const VALID: &str = "# Trusted agents; comments are why this is TOML.\nversion = 1\n\n[[programs]]\nid = \"shell\"\nexecutable = \"/bin/sh\"\nargs = [\"-c\", \"true\"]\n";
    #[test]
    fn a_missing_file_disables_sessions_and_toml_loads() {
        let d = scratch("load");
        let none = Supervisor::configured(Some(&d.join("agents.toml")), &[]).unwrap();
        assert!(none.programs.is_empty());
        write(&d.join("agents.toml"), VALID);
        let s = Supervisor::configured(Some(&d.join("agents.toml")), &[]).unwrap();
        assert_eq!(s.programs.keys().collect::<Vec<_>>(), ["shell"]);
        // args and env may be left out of a hand-written entry.
        write(
            &d.join("min.toml"),
            "version = 1\n[[programs]]\nid = \"min\"\nexecutable = \"/bin/sh\"\n",
        );
        assert!(Supervisor::configured(Some(&d.join("min.toml")), &[]).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn json_unknown_fields_shared_files_and_project_roots_refuse() {
        let d = scratch("refuse");
        write(&d.join("old.toml"), r#"{"version":1,"programs":[]}"#);
        assert_eq!(
            Supervisor::configured(Some(&d.join("old.toml")), &[]).err(),
            Some(Error::InvalidRequest)
        );
        write(
            &d.join("extra.toml"),
            "version = 1\nprograms = []\ntoken = \"x\"\n",
        );
        assert_eq!(
            Supervisor::configured(Some(&d.join("extra.toml")), &[]).err(),
            Some(Error::InvalidRequest)
        );
        write(&d.join("v2.toml"), "version = 2\nprograms = []\n");
        assert_eq!(
            Supervisor::configured(Some(&d.join("v2.toml")), &[]).err(),
            Some(Error::Version)
        );
        write(&d.join("shared.toml"), VALID);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            d.join("shared.toml"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(Supervisor::configured(Some(&d.join("shared.toml")), &[]).is_err());
        write(&d.join("agents.toml"), VALID);
        let root = d.canonicalize().unwrap();
        assert_eq!(
            Supervisor::configured(Some(&d.join("agents.toml")), &[root]).err(),
            Some(Error::UnsafeRuntime)
        );
        assert_eq!(
            Supervisor::configured(Some(Path::new("relative.toml")), &[]).err(),
            Some(Error::InvalidRequest)
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
