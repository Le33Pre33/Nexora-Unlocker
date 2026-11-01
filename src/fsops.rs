//! Файловые операции, шаблоны переименования и защита системных путей.
//! Модуль не зависит от интерфейса и Windows API (кроме `trash`), поэтому хорошо тестируется.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Trash,
    Delete,
    Move,
    Copy,
    Rename,
}

impl Op {
    pub fn label(self) -> &'static str {
        match self {
            Op::Trash => "В Корзину",
            Op::Delete => "Удаление",
            Op::Move => "Перемещение",
            Op::Copy => "Копирование",
            Op::Rename => "Переименование",
        }
    }
}

pub fn remove_path(p: &Path) -> io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

pub fn copy_path(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_path(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

pub fn move_path(src: &Path, dst: &Path) -> io::Result<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Другой диск или занятый файл: копируем, затем удаляем оригинал.
    copy_path(src, dst)?;
    remove_path(src).map_err(|e| {
        io::Error::new(e.kind(), format!("скопировано, но исходный элемент не удалён: {e}"))
    })
}

fn other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg.to_string())
}

fn same_ci(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Выполняет операцию над одним элементом.
/// Для Move/Copy `dest` — папка назначения, для Rename — полный новый путь.
pub fn run_op(op: Op, src: &Path, dest: Option<&Path>) -> io::Result<()> {
    match op {
        Op::Trash => trash::delete(src).map_err(|e| other(&e.to_string())),
        Op::Delete => remove_path(src),
        Op::Rename => {
            let dst = dest.ok_or_else(|| other("не задано новое имя"))?;
            if dst.exists() && !same_ci(src, dst) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "элемент с таким именем уже есть"));
            }
            std::fs::rename(src, dst)
        }
        Op::Move | Op::Copy => {
            let dir = dest.ok_or_else(|| other("не выбрана папка назначения"))?;
            let name = src.file_name().ok_or_else(|| other("у элемента нет имени"))?;
            if dir.starts_with(src) {
                return Err(other("нельзя поместить папку саму в себя"));
            }
            let dst: PathBuf = dir.join(name);
            if dst.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "в папке назначения уже есть такой элемент",
                ));
            }
            if op == Op::Move {
                move_path(src, &dst)
            } else {
                copy_path(src, &dst)
            }
        }
    }
}

// ---------- шаблоны переименования ----------

/// Подставляет переменные: {имя}, {расш}, {номер}, {номер:N} (N цифр с нулями, до 9).
/// Неизвестные переменные остаются как есть.
pub fn apply_template(t: &str, stem: &str, ext: &str, index: usize) -> String {
    let mut out = String::new();
    let mut rest = t;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                let token = &after[..end];
                match token {
                    "имя" => out.push_str(stem),
                    "расш" => out.push_str(ext),
                    "номер" => out.push_str(&index.to_string()),
                    _ => match token.strip_prefix("номер:").and_then(|w| w.parse::<usize>().ok()) {
                        Some(w) if w <= 9 => out.push_str(&format!("{:0width$}", index, width = w)),
                        _ => {
                            out.push('{');
                            out.push_str(token);
                            out.push('}');
                        }
                    },
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Делит имя файла на (имя, расширение без точки).
pub fn split_ext(full: &str) -> (String, String) {
    match full.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), e.to_string()),
        _ => (full.to_string(), String::new()),
    }
}

/// Итоговое имя: расширение сохраняется автоматически, если в шаблоне нет точки и {расш}.
pub fn build_name(template: &str, stem: &str, ext: &str, index: usize) -> String {
    let mut name = apply_template(template, stem, ext, index);
    if !ext.is_empty() && !template.contains('.') && !template.contains("{расш}") {
        name.push('.');
        name.push_str(ext);
    }
    name
}

pub fn valid_file_name(n: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
        "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if n.is_empty() || n.chars().count() > 255 || n.ends_with(' ') || n.ends_with('.') {
        return false;
    }
    if n.chars().any(|c| c.is_control() || "\\/:*?\"<>|".contains(c)) {
        return false;
    }
    let base = n.split('.').next().unwrap_or("").to_lowercase();
    !RESERVED.contains(&base.as_str())
}

pub struct RenamePlan {
    pub src: PathBuf,
    pub old_name: String,
    pub new_name: String,
    pub error: Option<&'static str>,
}

/// Строит план массового переименования и находит конфликты, ничего не меняя на диске.
pub fn plan_rename(sources: &[PathBuf], template: &str, exists: &dyn Fn(&Path) -> bool) -> Vec<RenamePlan> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(sources.len());
    for (i, src) in sources.iter().enumerate() {
        let old_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (stem, ext) = if src.is_dir() { (old_name.clone(), String::new()) } else { split_ext(&old_name) };
        let new_name = build_name(template, &stem, &ext, i + 1);
        let dst = src.with_file_name(&new_name);
        let key = dst.to_string_lossy().to_lowercase();
        let error = if !valid_file_name(&new_name) {
            Some("недопустимое имя")
        } else if !seen.insert(key) {
            Some("повторяющееся имя")
        } else if new_name.to_lowercase() != old_name.to_lowercase() && exists(&dst) {
            Some("уже существует")
        } else {
            None
        };
        out.push(RenamePlan { src: src.clone(), old_name, new_name, error });
    }
    out
}

// ---------- защита ----------

fn norm(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\").trim_end_matches('\\').to_string()
}

/// Список путей, которые нельзя трогать при включённой защите.
pub struct Guard {
    /// Защищены сама папка и всё внутри.
    tree: Vec<String>,
    /// Защищена только сама папка (например, профиль пользователя).
    exact: Vec<String>,
}

impl Guard {
    pub fn new(tree: Vec<String>, exact: Vec<String>) -> Self {
        Guard { tree, exact }
    }

    pub fn from_env() -> Self {
        let mut tree = Vec::new();
        for var in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] {
            if let Some(v) = std::env::var_os(var) {
                tree.push(norm(Path::new(&v)));
            }
        }
        let mut exact = Vec::new();
        if let Some(v) = std::env::var_os("USERPROFILE") {
            exact.push(norm(Path::new(&v)));
        }
        Guard::new(tree, exact)
    }

    pub fn is_protected(&self, p: &Path) -> bool {
        let n = norm(p);
        if n.is_empty() || (n.len() == 2 && n.ends_with(':')) {
            return true; // корень диска
        }
        if n.split('\\').any(|s| s == "system volume information" || s == "$recycle.bin") {
            return true;
        }
        if self.exact.iter().any(|e| *e == n) {
            return true;
        }
        self.tree.iter().any(|r| n == *r || n.starts_with(&format!("{r}\\")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> Guard {
        Guard::new(
            vec![r"c:\windows".into(), r"c:\program files".into()],
            vec![r"c:\users\me".into()],
        )
    }

    #[test]
    fn protects_system_tree() {
        let g = guard();
        assert!(g.is_protected(Path::new(r"C:\Windows")));
        assert!(g.is_protected(Path::new(r"C:\WINDOWS\System32\cmd.exe")));
        assert!(g.is_protected(Path::new(r"c:\program files\App\a.dll")));
    }

    #[test]
    fn protects_drive_root_and_profile_root_only() {
        let g = guard();
        assert!(g.is_protected(Path::new(r"D:\")));
        assert!(g.is_protected(Path::new(r"C:\Users\Me")));
        assert!(!g.is_protected(Path::new(r"C:\Users\Me\Desktop\a.txt")));
    }

    #[test]
    fn does_not_match_similar_prefix() {
        let g = guard();
        assert!(!g.is_protected(Path::new(r"C:\Windows.old\a.txt")));
        assert!(!g.is_protected(Path::new(r"D:\Games")));
    }

    #[test]
    fn protects_recycle_bin() {
        assert!(guard().is_protected(Path::new(r"D:\$Recycle.Bin\S-1-5\x")));
    }

    #[test]
    fn template_variables() {
        assert_eq!(apply_template("Фото_{номер:3}", "a", "jpg", 7), "Фото_007");
        assert_eq!(apply_template("{имя}_копия", "doc", "txt", 1), "doc_копия");
        assert_eq!(apply_template("{номер}.{расш}", "a", "png", 12), "12.png");
    }

    #[test]
    fn template_edge_cases() {
        assert_eq!(apply_template("{неизвестно}", "a", "b", 1), "{неизвестно}");
        assert_eq!(apply_template("a{номер", "a", "b", 1), "a{номер");
        assert_eq!(apply_template("{номер:99}", "a", "b", 1), "{номер:99}");
    }

    #[test]
    fn extension_is_kept_unless_specified() {
        assert_eq!(build_name("Фото_{номер:2}", "a", "jpg", 3), "Фото_03.jpg");
        assert_eq!(build_name("report.docx", "a", "txt", 1), "report.docx");
        assert_eq!(build_name("{имя}.{расш}", "a", "txt", 1), "a.txt");
    }

    #[test]
    fn file_name_validation() {
        assert!(valid_file_name("a.txt"));
        assert!(!valid_file_name(""));
        assert!(!valid_file_name("a/b"));
        assert!(!valid_file_name("con.txt"));
        assert!(!valid_file_name("name."));
    }

    #[test]
    fn plan_detects_duplicates_and_conflicts() {
        let src = vec![PathBuf::from(r"C:\t\a.txt"), PathBuf::from(r"C:\t\b.txt")];
        let plan = plan_rename(&src, "same", &|_| false);
        assert!(plan[0].error.is_none());
        assert_eq!(plan[1].error, Some("повторяющееся имя"));

        let plan = plan_rename(&src, "x_{номер}", &|p| p.to_string_lossy().ends_with("x_2.txt"));
        assert!(plan[0].error.is_none());
        assert_eq!(plan[1].error, Some("уже существует"));
    }

    #[test]
    fn case_only_rename_is_allowed() {
        let src = vec![PathBuf::from(r"C:\t\a.txt")];
        let plan = plan_rename(&src, "A", &|_| true);
        assert!(plan[0].error.is_none());
        assert_eq!(plan[0].new_name, "A.txt");
    }
}
