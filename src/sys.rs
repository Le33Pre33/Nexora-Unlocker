//! Тонкая обёртка над Win32 API: блокирующие процессы, завершение, права, время, шрифты.

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, ERROR_SUCCESS, HWND};
use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
use windows::Win32::System::RestartManager::*;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Процесс, удерживающий файл.
#[derive(Clone, Debug)]
pub struct Holder {
    pub pid: u32,
    pub name: String,
    /// Системный процесс, проводник или служба: завершать опасно.
    pub critical: bool,
}

const MAX_FILES_PER_SCAN: usize = 5000;

fn collect_files(p: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_FILES_PER_SCAN {
        return;
    }
    if p.is_dir() {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                collect_files(&e.path(), out);
            }
        }
    } else {
        out.push(p.to_path_buf());
    }
}

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Процессы, которые держат файл или любой файл внутри папки (Windows Restart Manager).
/// `Err(код)` — ошибка Windows API: результат неизвестен, «свободен» утверждать нельзя.
pub fn holders(path: &Path) -> Result<Vec<Holder>, u32> {
    let mut files = Vec::new();
    collect_files(path, &mut files);
    if files.is_empty() {
        return Ok(vec![]);
    }
    let wides: Vec<Vec<u16>> = files.iter().map(|f| wide(f)).collect();
    let ptrs: Vec<PCWSTR> = wides.iter().map(|w| PCWSTR(w.as_ptr())).collect();

    unsafe {
        let mut session = 0u32;
        let mut key = [0u16; 33];
        let r = RmStartSession(&mut session, 0, PWSTR(key.as_mut_ptr()));
        if r != ERROR_SUCCESS {
            return Err(r.0);
        }
        let out = query_session(session, &ptrs);
        let _ = RmEndSession(session);
        out
    }
}

unsafe fn query_session(session: u32, ptrs: &[PCWSTR]) -> Result<Vec<Holder>, u32> {
    let me = std::process::id();
    let r = RmRegisterResources(session, Some(ptrs), None, None);
    if r != ERROR_SUCCESS {
        return Err(r.0);
    }
    let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
    let r = RmGetList(session, &mut needed, &mut count, None, &mut reasons);
    if r != ERROR_SUCCESS && r != ERROR_MORE_DATA {
        return Err(r.0);
    }
    if needed == 0 {
        return Ok(vec![]);
    }
    let mut infos: Vec<RM_PROCESS_INFO> = (0..needed).map(|_| std::mem::zeroed()).collect();
    count = needed;
    let r = RmGetList(session, &mut needed, &mut count, Some(infos.as_mut_ptr()), &mut reasons);
    if r != ERROR_SUCCESS {
        return Err(r.0);
    }
    let mut result: Vec<Holder> = Vec::new();
    for i in infos.iter().take(count as usize) {
        let pid = i.Process.dwProcessId;
        if pid == me || result.iter().any(|h| h.pid == pid) {
            continue;
        }
        let name = String::from_utf16_lossy(&i.strAppName).trim_end_matches('\0').to_string();
        let t = i.ApplicationType;
        let critical = t == RmCritical || t == RmExplorer || t == RmService || pid <= 4;
        result.push(Holder { pid, name, critical });
    }
    Ok(result)
}

pub fn kill(pid: u32) -> bool {
    unsafe {
        match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(h) => {
                let ok = TerminateProcess(h, 1).is_ok();
                let _ = CloseHandle(h);
                ok
            }
            Err(_) => false,
        }
    }
}

/// Планирует удаление при следующей перезагрузке.
pub fn delete_on_reboot(p: &Path) -> bool {
    let w = wide(p);
    unsafe { MoveFileExW(PCWSTR(w.as_ptr()), PCWSTR::null(), MOVEFILE_DELAY_UNTIL_REBOOT).is_ok() }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

/// Запускает копию программы с запросом UAC. UAC не обходится: решает пользователь.
pub fn restart_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let wexe = wide(&exe);
    let r = unsafe {
        ShellExecuteW(
            HWND::default(),
            w!("runas"),
            PCWSTR(wexe.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    r.0 as isize > 32
}

/// Локальное время «ЧЧ:ММ:СС» для журнала.
pub fn local_time() -> String {
    let t = unsafe { GetLocalTime() };
    format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond)
}

/// Читает системный шрифт из C:\Windows\Fonts (первый найденный из списка).
pub fn read_system_font(names: &[&str]) -> Option<Vec<u8>> {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    names.iter().find_map(|n| std::fs::read(root.join("Fonts").join(n)).ok())
}
