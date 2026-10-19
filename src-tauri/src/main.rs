#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod catalogs;
mod launch;
mod launchbox;
mod launchers;
mod legacy;
mod library;
mod manage;
mod names;
#[cfg(windows)]
mod registry;
mod sgdb;
mod steam;
mod vdf;

use launch::{Session, Step};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WebviewWindow};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

struct App {
    data: PathBuf,
    entries: Mutex<HashMap<String, library::Entry>>,
    session: Mutex<Option<Session>>,
    fixing: AtomicBool,
    /// One writer at a time for user.json.
    user: Mutex<()>,
    /// LaunchBox names for "Find online", built on first use.
    names: Mutex<Option<Arc<Vec<launchbox::Entry>>>>,
}

/// Runs slow work (disk, network) off the main thread, so the window stays responsive.
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppHandle, &App) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&app, &app.state::<App>()))
        .await
        .map_err(|e| e.to_string())?
}

fn kind(name: &str) -> Result<art::Kind, String> {
    art::Kind::ALL.into_iter().find(|k| k.stem() == name).ok_or_else(|| format!("unknown art: {name}"))
}

/// Desktop mode is a normal window; big screen fills the screen with no frame.
#[tauri::command]
fn set_view(app: AppHandle, desktop: bool) -> Result<(), String> {
    let w = app.get_webview_window("main").ok_or("no window")?;
    let r: tauri::Result<()> = (|| {
        if desktop {
            w.set_fullscreen(false)?;
            w.set_decorations(true)?;
            w.set_resizable(true)?;
            w.set_min_size(Some(LogicalSize::new(1024.0, 640.0)))?;
            w.set_size(LogicalSize::new(1440.0, 900.0))?;
            w.center()?;
        } else {
            w.set_min_size(None::<LogicalSize<f64>>)?;
            w.set_decorations(false)?;
            w.set_fullscreen(true)?;
        }
        w.set_focus()
    })();
    r.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_edit(app: AppHandle, id: String, patch: manage::Patch) -> Result<(), String> {
    blocking(app, move |_, s| {
        let _one = s.user.lock().unwrap();
        manage::with_user(&s.data, |u| manage::apply(u, &id, patch))
    })
    .await
}

#[tauri::command]
async fn set_art(app: AppHandle, id: String, kind: String, source: String) -> Result<(), String> {
    let kind = self::kind(&kind)?;
    blocking(app, move |_, s| {
        let _one = s.user.lock().unwrap();
        manage::with_user(&s.data, |u| manage::set_art(&s.data, u, &id, kind, &source))?
    })
    .await
}

#[tauri::command]
async fn remove_game(app: AppHandle, id: String) -> Result<(), String> {
    blocking(app, move |_, s| {
        let _one = s.user.lock().unwrap();
        manage::with_user(&s.data, |u| manage::remove(&s.data, u, &id))
    })
    .await
}

/// Adds games by their programs, then finds their art in the background.
#[tauri::command]
async fn add_games(app: AppHandle, exes: Vec<String>) -> Result<Vec<String>, String> {
    blocking(app, move |app, s| {
        let known: Vec<PathBuf> = s.entries.lock().unwrap().values().filter_map(|e| e.launch.exe.as_ref().map(PathBuf::from)).collect();
        let added = {
            let _one = s.user.lock().unwrap();
            manage::with_user(&s.data, |u| manage::add(u, &known, &exes))?
        };
        if !added.is_empty() {
            let (app, data, ids) = (app.clone(), s.data.clone(), added.clone());
            thread::spawn(move || {
                let (_, entries) = library::load(&data, steam::scan().as_ref(), &launchers::scan());
                let needs = library::needs(&entries).into_iter().filter(|n| ids.contains(&n.id)).collect();
                art::fix(&data, needs, now_unix());
                let _ = app.emit("library-changed", ());
            });
        }
        Ok(added)
    })
    .await
}

#[tauri::command]
async fn scan_folder(app: AppHandle, folder: String) -> Result<Vec<manage::Candidate>, String> {
    blocking(app, move |_, s| {
        let known: Vec<PathBuf> = s.entries.lock().unwrap().values().map(|e| e.install_dir.clone()).collect();
        Ok(manage::scan(std::path::Path::new(&folder), &known))
    })
    .await
}

/// Steam results first (official art), then SteamGridDB (with a key), then LaunchBox
/// (every platform). A "Demo" or "Playtest" in the name is left out of the search.
#[tauri::command]
async fn find_online(app: AppHandle, query: String) -> Result<Vec<manage::Match>, String> {
    blocking(app, move |_, s| {
        let query = names::search_name(&query);
        let store = art::Store::new();
        let mut out = manage::find_steam(&store, &query);
        if let Some(key) = sgdb::key(&s.data) {
            out.extend(manage::find_steamgriddb(&store, &key, &query));
        }
        let index = {
            let mut slot = s.names.lock().unwrap();
            if slot.is_none() {
                if let Some(zip) = launchbox::ensure(&s.data, &store.agent) {
                    *slot = Some(Arc::new(launchbox::index(|each| launchbox::scan_archive(&zip, each))));
                }
            }
            slot.clone()
        };
        if let Some(index) = index {
            out.extend(manage::find_launchbox(&index, &query));
        }
        Ok(out)
    })
    .await
}

/// Where "Change art" starts for one game, and the name it searched by.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtStart {
    query: String,
    found: Option<manage::Match>,
    steam_grid_db: bool,
}

#[tauri::command]
async fn art_match(app: AppHandle, game: String) -> Result<ArtStart, String> {
    blocking(app, move |_, s| {
        let entry = s.entries.lock().unwrap().get(&game).cloned().ok_or("unknown game")?;
        let key = sgdb::key(&s.data);
        let found = manage::first_match(&art::Store::new(), key.as_deref(), &entry.game.name, entry.steam_app_id);
        Ok(ArtStart { query: names::search_name(&entry.game.name), found, steam_grid_db: key.is_some() })
    })
    .await
}

#[tauri::command]
async fn art_choices(app: AppHandle, source: String, id: String) -> Result<manage::Choices, String> {
    blocking(app, move |_, s| manage::choices(&s.data, &art::Store::new(), sgdb::key(&s.data).as_deref(), &source, &id)).await
}

#[tauri::command]
async fn reset_art(app: AppHandle, id: String, kind: String) -> Result<(), String> {
    let kind = self::kind(&kind)?;
    blocking(app, move |_, s| {
        let _one = s.user.lock().unwrap();
        manage::with_user(&s.data, |u| manage::reset_art(&s.data, u, &id, kind))
    })
    .await
}

/// Saves a SteamGridDB key after one test search; an empty key removes it.
#[tauri::command]
async fn set_steam_grid_db_key(app: AppHandle, key: String) -> Result<(), String> {
    blocking(app, move |_, s| {
        let key = key.trim().to_string();
        if !key.is_empty() {
            sgdb::Client::new(&art::Store::new().agent, &key).search("Portal")?;
        }
        let settings = sgdb::Settings { steam_grid_db_key: Some(key).filter(|k| !k.is_empty()) };
        sgdb::write_settings(&s.data, &settings).map_err(|e| e.to_string())
    })
    .await
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Lets the page show art from Beanbag's folder and from Steam's cache.
fn allow_art(app: &AppHandle, data: &Path, steam_art: Option<PathBuf>) {
    let scope = app.asset_protocol_scope();
    let _ = scope.allow_directory(data.join("art"), true);
    if let Some(dir) = steam_art {
        let _ = scope.allow_directory(dir, true);
    }
}

/// Async, so the scan runs off the main thread and the window can show the cached library
/// at once. The games stay locked through the scan, so a game started from that first
/// screen waits for it instead of failing.
#[tauri::command]
async fn load_library(app: AppHandle) -> Result<library::Library, String> {
    blocking(app, |app, s| {
        let mut entries = s.entries.lock().unwrap();
        let steam = steam::scan();
        allow_art(app, &s.data, steam.as_ref().map(steam::Steam::art_dir));
        let (lib, found) = library::load(&s.data, steam.as_ref(), &launchers::scan());
        fix_art_once(app, s, library::needs(&found));
        *entries = found.into_iter().map(|e| (e.game.id.clone(), e)).collect();
        Ok(lib)
    })
    .await
}

/// Once per run, after the first screen is up: fetch missing art, then tell the page.
fn fix_art_once(app: &AppHandle, state: &App, needs: Vec<art::Need>) {
    if needs.is_empty() || state.fixing.swap(true, Ordering::SeqCst) {
        return;
    }
    let (app, data) = (app.clone(), state.data.clone());
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(4));
        if art::fix(&data, needs, now_unix()) > 0 {
            let _ = app.emit("library-changed", ());
        }
    });
}

#[tauri::command]
fn launch_game(state: State<App>, id: String) -> Result<(), String> {
    let entry = state.entries.lock().unwrap().get(&id).cloned().ok_or("unknown game")?;
    let pid = launch::start(&entry.launch)?;
    let _ = library::record_launch(&state.data, &id, now_unix());
    let dirs = launch::game_dirs(&entry.install_dir, &entry.launch);
    *state.session.lock().unwrap() = Some(Session::new(id, dirs, pid, Instant::now()));
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// Restores the launcher over whatever had focus (usually the game that just closed).
fn bring_to_front(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
        use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        // Windows gives focus only to the app that saw the last input; a synthetic Alt tap counts.
        unsafe {
            keybd_event(VK_MENU as u8, 0, 0, 0);
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
            if let Ok(hwnd) = window.hwnd() {
                SetForegroundWindow(hwnd.0 as _);
            }
        }
    }
    let _ = window.set_focus();
}

/// Follows the running game once a second: minimise when it appears, record the
/// playtime and come back when it closes.
fn follow_games(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let state = app.state::<App>();
        let mut slot = state.session.lock().unwrap();
        let Some(session) = slot.as_mut() else { continue };
        let running = launch::is_running(&session.dirs, session.pid, &launch::processes());
        let window = app.get_webview_window("main");
        match session.step(running, Instant::now()) {
            Step::Waiting | Step::Playing => {}
            Step::Started => {
                if let Some(w) = &window {
                    let _ = w.minimize();
                }
            }
            Step::Ended(played) => {
                let _ = library::record_playtime(&state.data, &session.id, played.as_secs());
                *slot = None;
                if let Some(w) = &window {
                    bring_to_front(w);
                }
                let _ = app.emit("game-exited", ());
            }
            Step::GaveUp => {
                // The game may still be loading behind a launcher; do not steal focus from it.
                *slot = None;
                let _ = app.emit("game-exited", ());
            }
        }
    });
}

/// The first release run turns on "Start with Windows"; after that the menu owns it.
/// While it is on, it points at whichever copy runs, so an update or a move keeps working.
fn first_run(app: &AppHandle, data: &PathBuf) {
    if cfg!(debug_assertions) {
        return;
    }
    let autolaunch = app.autolaunch();
    let marker = data.join("first-run-done");
    // The old "BigScreen" startup entry hands over to the new name.
    if legacy::take_startup_entry() {
        let _ = autolaunch.enable();
    } else if !marker.exists() {
        let _ = autolaunch.enable();
        let _ = std::fs::write(marker, b"");
    } else if autolaunch.is_enabled().unwrap_or(false) {
        let _ = autolaunch.enable();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                bring_to_front(&window);
            }
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_local_data_dir()?;
            legacy::move_data(&data);
            std::fs::create_dir_all(&data)?;
            // The first frame shows the cached library, so its art must be readable before any scan.
            allow_art(app.handle(), &data, steam::art_cache());
            first_run(app.handle(), &data);
            app.manage(App {
                data,
                entries: Mutex::default(),
                session: Mutex::default(),
                fixing: AtomicBool::new(false),
                user: Mutex::default(),
                names: Mutex::default(),
            });
            follow_games(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_library,
            launch_game,
            quit,
            set_view,
            save_edit,
            set_art,
            remove_game,
            add_games,
            scan_folder,
            find_online,
            art_match,
            art_choices,
            reset_art,
            set_steam_grid_db_key
        ])
        .run(tauri::generate_context!())
        .expect("Beanbag failed to start");
}

#[cfg(test)]
mod live {
    use std::path::PathBuf;

    /// Fills missing art in the real data folder over the network. Run by hand on the PC:
    /// cargo test --release -- --ignored fill_missing_art --nocapture
    #[test]
    #[ignore]
    fn fill_missing_art() {
        let data = PathBuf::from(std::env::var("LOCALAPPDATA").expect("Windows only")).join("com.beanbag.launcher");
        let steam = crate::steam::scan();
        let (_, before) = crate::library::load(&data, steam.as_ref(), &crate::launchers::scan());
        let needs = crate::library::needs(&before);
        println!("{} of {} games need something", needs.len(), before.len());
        let changed = crate::art::fix(&data, needs, crate::now_unix());
        let (_, after) = crate::library::load(&data, steam.as_ref(), &crate::launchers::scan());
        let missing = |kind: crate::art::Kind| crate::library::needs(&after).iter().filter(|n| n.kinds.contains(&kind)).count();
        println!("{changed} games improved");
        for kind in crate::art::Kind::ALL {
            println!("still missing {}: {}", kind.stem(), missing(kind));
        }
        println!("still no description: {}", crate::library::needs(&after).iter().filter(|n| n.description).count());
        for row in crate::catalogs::ORDER {
            let games: Vec<&str> = after.iter().filter(|e| e.game.catalogs.iter().any(|c| c == row)).map(|e| e.game.name.as_str()).collect();
            println!("{row}: {} — {}", games.len(), games.iter().take(12).cloned().collect::<Vec<_>>().join(", "));
        }
    }
}
