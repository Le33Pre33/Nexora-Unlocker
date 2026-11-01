//! Настройки приложения: простой текстовый файл `ключ=значение`.
//! Защита системных путей намеренно НЕ сохраняется: после перезапуска она снова включена.

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconSet {
    /// Segoe Fluent Icons (Windows 11).
    Fluent,
    /// Segoe MDL2 Assets (Windows 10).
    Mdl2,
    /// Без значков, только текст.
    None,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    pub dark: bool,
    pub icons: IconSet,
    pub blockers_open: bool,
    pub protection: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { dark: true, icons: IconSet::Fluent, blockers_open: true, protection: true }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "theme" => s.dark = v != "light",
                "icons" => {
                    s.icons = match v {
                        "mdl2" => IconSet::Mdl2,
                        "none" => IconSet::None,
                        _ => IconSet::Fluent,
                    }
                }
                "blockers_open" => s.blockers_open = v != "false",
                _ => {}
            }
        }
        s
    }

    pub fn serialize(&self) -> String {
        let icons = match self.icons {
            IconSet::Fluent => "fluent",
            IconSet::Mdl2 => "mdl2",
            IconSet::None => "none",
        };
        format!(
            "theme={}\nicons={}\nblockers_open={}\n",
            if self.dark { "dark" } else { "light" },
            icons,
            self.blockers_open
        )
    }

    /// Рядом с .exe лежит `portable.flag` → настройки хранятся там же, иначе в %APPDATA%.
    pub fn path() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        if exe_dir.join("portable.flag").exists() {
            return Some(exe_dir.join("settings.ini"));
        }
        let appdata = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(appdata).join("NexoraUnlocker").join("settings.ini"))
    }

    pub fn load() -> Settings {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| Settings::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.serialize());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Settings { dark: false, icons: IconSet::Mdl2, blockers_open: false, protection: true };
        assert_eq!(Settings::parse(&s.serialize()), s);
    }

    #[test]
    fn garbage_gives_defaults() {
        assert_eq!(Settings::parse("мусор\n=\nicons"), Settings::default());
    }

    #[test]
    fn protection_is_never_persisted() {
        let s = Settings { protection: false, ..Settings::default() };
        assert!(Settings::parse(&s.serialize()).protection);
    }
}
