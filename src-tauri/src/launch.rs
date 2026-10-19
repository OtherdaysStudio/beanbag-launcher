//! Starting a game and noticing when it ends, without any launcher in between.

use crate::library::Launch;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// How long a game may take to show a process (Steam updates, launcher splash screens).
const START_TIMEOUT: Duration = Duration::from_secs(120);
/// Gaps shorter than this are a game restarting itself, not the end of a session.
const END_GRACE: Duration = Duration::from_secs(5);

/// Opens a URL or starts an exe the way Explorer would, so elevation prompts and
/// store links work. Returns the new process id when Windows reports one.
#[cfg(windows)]
pub fn start(launch: &Launch) -> Result<Option<u32>, String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::GetProcessId,
        UI::{
            Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (file, params, dir) = match (&launch.url, &launch.exe) {
        (Some(url), _) => (url.clone(), String::new(), None),
        (None, Some(exe)) => (exe.clone(), launch.args.clone(), launch.working_dir.clone()),
        (None, None) => return Err("nothing to start".into()),
    };
    let (verb, file, params) = (wide("open"), wide(&file), wide(&params));
    let dir = dir.filter(|d| !d.is_empty()).map(|d| wide(&d));

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.lpDirectory = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
    info.nShow = SW_SHOWNORMAL;

    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if info.hProcess.is_null() {
        return Ok(None);
    }
    let pid = unsafe { GetProcessId(info.hProcess) };
    unsafe { CloseHandle(info.hProcess) };
    Ok(Some(pid).filter(|&p| p != 0))
}

#[cfg(not(windows))]
pub fn start(launch: &Launch) -> Result<Option<u32>, String> {
    let target = launch.url.as_ref().or(launch.exe.as_ref()).ok_or("nothing to start")?;
    std::process::Command::new("open").arg(target).spawn().map_err(|e| e.to_string())?;
    Ok(None)
}

/// Every running process as (pid, lower-case exe path).
#[cfg(windows)]
pub fn processes() -> Vec<(u32, String)> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
            Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snap, &mut entry) != 0;
        while more {
            let pid = entry.th32ProcessID;
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !handle.is_null() {
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                if QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) != 0 {
                    out.push((pid, String::from_utf16_lossy(&buf[..len as usize]).to_lowercase()));
                }
                CloseHandle(handle);
            }
            more = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
pub fn processes() -> Vec<(u32, String)> {
    Vec::new()
}

/// Folders whose processes belong to the game. A drive root would match half the PC,
/// and a launcher's folder (Battle.net.exe) is not the game, so neither is used.
pub fn game_dirs(install_dir: &Path, launch: &Launch) -> Vec<String> {
    let deep_enough = |p: &Path| p.components().count() >= 3;
    let dir = if deep_enough(install_dir) {
        Some(install_dir.to_path_buf())
    } else if launch.url.is_none() && install_dir.as_os_str().is_empty() {
        launch.exe.as_ref().and_then(|e| Path::new(e).parent().map(PathBuf::from)).filter(|p| deep_enough(p))
    } else {
        None
    };
    dir.map(|d| {
        let mut s = d.to_string_lossy().to_lowercase().replace('/', "\\");
        if !s.ends_with('\\') {
            s.push('\\');
        }
        s
    })
    .into_iter()
    .collect()
}

pub fn is_running(dirs: &[String], pid: Option<u32>, procs: &[(u32, String)]) -> bool {
    procs
        .iter()
        .any(|(p, exe)| Some(*p) == pid || dirs.iter().any(|d| exe.replace('/', "\\").starts_with(d.as_str())))
}

#[derive(Debug, PartialEq)]
pub enum Step {
    Waiting,
    Started,
    Playing,
    Ended(Duration),
    GaveUp,
}

/// One game from launch to close.
pub struct Session {
    pub id: String,
    pub dirs: Vec<String>,
    pub pid: Option<u32>,
    launched: Instant,
    started: Option<Instant>,
    last_seen: Instant,
}

impl Session {
    pub fn new(id: String, dirs: Vec<String>, pid: Option<u32>, now: Instant) -> Self {
        Session { id, dirs, pid, launched: now, started: None, last_seen: now }
    }

    pub fn step(&mut self, running: bool, now: Instant) -> Step {
        match (self.started, running) {
            (None, true) => {
                self.started = Some(now);
                self.last_seen = now;
                Step::Started
            }
            (None, false) if now - self.launched > START_TIMEOUT => Step::GaveUp,
            (None, false) => Step::Waiting,
            (Some(_), true) => {
                self.last_seen = now;
                Step::Playing
            }
            (Some(start), false) if now - self.last_seen > END_GRACE => Step::Ended(self.last_seen - start),
            (Some(_), false) => Step::Playing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(path: &str) -> Launch {
        Launch { exe: Some(path.into()), ..Default::default() }
    }

    #[test]
    fn tracks_the_install_folder_but_never_a_drive_root() {
        assert_eq!(game_dirs(Path::new("D:/Games/Hades"), &exe("D:/Games/Hades/Hades.exe")), ["d:\\games\\hades\\"]);
        assert!(game_dirs(Path::new("D:/"), &exe("D:/x.exe")).is_empty());
        assert_eq!(game_dirs(Path::new(""), &exe("D:/Games/Celeste/Celeste.exe")), ["d:\\games\\celeste\\"]);
    }

    #[test]
    fn a_launcher_folder_is_not_the_game() {
        let battlenet = exe("D:/SteamLibrary/Battle.net/Battle.net/Battle.net.exe");
        assert_eq!(
            game_dirs(Path::new("D:/SteamLibrary/Battle.net/Overwatch"), &battlenet),
            ["d:\\steamlibrary\\battle.net\\overwatch\\"]
        );
    }

    #[test]
    fn sees_a_game_by_folder_or_by_process_id() {
        let dirs = vec!["d:\\games\\hades\\".to_string()];
        let procs = vec![(7, "c:\\windows\\explorer.exe".to_string()), (9, "d:\\games\\hades\\x64\\hades.exe".into())];
        assert!(is_running(&dirs, None, &procs));
        assert!(is_running(&[], Some(7), &procs));
        assert!(!is_running(&["d:\\games\\had\\".into()], None, &procs));
    }

    #[test]
    fn a_session_starts_plays_and_ends_after_a_grace_period() {
        let t0 = Instant::now();
        let s = |secs| t0 + Duration::from_secs(secs);
        let mut session = Session::new("g".into(), vec![], None, t0);
        assert_eq!(session.step(false, s(3)), Step::Waiting);
        assert_eq!(session.step(true, s(10)), Step::Started);
        assert_eq!(session.step(true, s(70)), Step::Playing);
        assert_eq!(session.step(false, s(72)), Step::Playing);
        assert_eq!(session.step(true, s(73)), Step::Playing);
        assert_eq!(session.step(false, s(80)), Step::Ended(Duration::from_secs(63)));
    }

    #[test]
    fn a_game_that_never_appears_is_given_up() {
        let t0 = Instant::now();
        let mut session = Session::new("g".into(), vec![], None, t0);
        assert_eq!(session.step(false, t0 + Duration::from_secs(119)), Step::Waiting);
        assert_eq!(session.step(false, t0 + Duration::from_secs(121)), Step::GaveUp);
    }
}
