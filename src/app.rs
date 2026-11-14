use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use tokio::sync::mpsc;

use crate::{error::Error, git::repository::Repository};

pub struct App {
    pub repos_dir: String,
    pub repos: Arc<RwLock<Vec<Repository>>>,

    _watcher: RecommendedWatcher,
}

impl App {
    pub async fn new(repos_dir: String) -> Self {
        let repos: Arc<RwLock<Vec<Repository>>> = Arc::new(RwLock::new(scan_repos(&repos_dir)));
        let watch_repos = repos.clone();

        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    let _ = tx.send(event);
                }
                Err(e) => eprintln!("repo watcher error: {e}"),
            })
            .expect("Failed to create repo directory watcher");

        watcher
            .watch(Path::new(&repos_dir), RecursiveMode::NonRecursive)
            .expect("Failed to watch repos directory");

        tokio::spawn(async move {
            let mut pending_creates: HashMap<PathBuf, tokio::task::JoinHandle<()>> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event.kind {
                    EventKind::Create(CreateKind::Folder) => {
                        for path in event.paths {
                            if let Some(handle) = pending_creates.remove(&path) {
                                handle.abort();
                            }

                            let repos_for_task = watch_repos.clone();
                            let path_for_task = path.clone();

                            let handle = tokio::spawn(async move {
                                Self::try_add_repo_when_ready(repos_for_task, path_for_task).await;
                            });

                            pending_creates.insert(path, handle);
                        }
                    }
                    EventKind::Remove(RemoveKind::Folder) => match watch_repos.write() {
                        Ok(mut guard) => {
                            let before = guard.len();
                            let removed: Vec<_> = guard
                                .iter()
                                .filter(|r| event.paths.contains(&r.path))
                                .map(|r| (r.name.clone(), r.path.clone()))
                                .collect();

                            guard.retain(|r| !event.paths.contains(&r.path));

                            for (name, path) in &removed {
                                println!("[watcher] - removed repo: {name} ({path:?})");
                            }
                            println!("[watcher] repo count now: {} (was {before})", guard.len());
                        }
                        Err(e) => eprintln!("[watcher] repo cache lock poisoned: {e}"),
                    },
                    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                        if let [old_path, new_path] = event.paths.as_slice() {
                            let (old_path, new_path) = (old_path.clone(), new_path.clone());

                            match watch_repos.write() {
                                Ok(mut guard) => guard.retain(|r| r.path != old_path),
                                Err(e) => eprintln!("[watcher] repo cache lock poisoned: {e}"),
                            }

                            let repos_for_task = watch_repos.clone();
                            tokio::spawn(async move {
                                Self::try_add_repo_when_ready(repos_for_task, new_path).await;
                            });

                            println!("[watcher] ~ rename detected: {old_path:?} -> ...");
                        }
                    }
                    _ => {}
                }
            }
        });

        Self {
            repos_dir,
            repos,
            _watcher: watcher,
        }
    }

    async fn try_add_repo_when_ready(watch_repos: Arc<RwLock<Vec<Repository>>>, path: PathBuf) {
        const MAX_ATTEMPTS: u32 = 30; // e.g. 30 * 1s = up to 30s for a big clone
        const POLL_INTERVAL: Duration = Duration::from_secs(1);
        const QUIET_CHECKS_REQUIRED: u32 = 2; // must succeed N times in a row (dir stopped changing)

        let mut consecutive_ok = 0;

        for attempt in 0..MAX_ATTEMPTS {
            tokio::time::sleep(POLL_INTERVAL).await;

            match Repository::open(&path) {
                Ok(repo) => {
                    consecutive_ok += 1;
                    if consecutive_ok >= QUIET_CHECKS_REQUIRED {
                        match watch_repos.write() {
                            Ok(mut guard) => {
                                println!("[watcher] + added repo: {} ({:?})", repo.name, path);
                                guard.push(repo);
                                println!("[watcher] repo count now: {}", guard.len());
                            }
                            Err(e) => eprintln!("[watcher] repo cache lock poisoned: {e}"),
                        }
                        return;
                    }
                }
                Err(_) => {
                    // not ready yet (mid-clone) — reset streak and keep polling
                    consecutive_ok = 0;
                }
            }

            if attempt == MAX_ATTEMPTS - 1 {
                eprintln!("[watcher] gave up waiting for repo to become ready: {path:?}");
            }
        }
    }

    pub fn find_repo(&self, name: &str) -> Option<Repository> {
        self.repos
            .read()
            .ok()?
            .iter()
            .find(|r| r.name == name)
            .cloned()
    }

    pub fn get_repo(&self, name: &str) -> Result<Repository, Error> {
        self.find_repo(name)
            .ok_or_else(|| Error::NotFound("Repo not found".to_string()))
    }
}

fn scan_repos(repos_dir: &str) -> Vec<Repository> {
    let entries = fs::read_dir(repos_dir).unwrap();

    let repos: Vec<Repository> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name();
            let path = e.path();
            match Repository::open(&path) {
                Ok(repo) => Some(repo),
                Err(e) => {
                    eprintln!("Failed to open repo {:?}: {:?}", name, e);
                    None
                }
            }
        })
        .collect();

    repos
}
