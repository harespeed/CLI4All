use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Ubuntu,
    Windows,
}

impl Platform {
    pub fn detect_current() -> Result<Self> {
        match std::env::consts::OS {
            "macos" => Ok(Self::Macos),
            "linux" => Ok(Self::Ubuntu),
            "windows" => Ok(Self::Windows),
            other => bail!("unsupported operating system '{other}'"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Ubuntu => "ubuntu",
            Self::Windows => "windows",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Macos => "macOS",
            Self::Ubuntu => "Ubuntu",
            Self::Windows => "Windows",
        }
    }

    pub fn prompt_name(self) -> &'static str {
        self.key()
    }
}

pub fn normalize_target_platform(target: &str) -> Option<Platform> {
    match target.trim().to_ascii_lowercase().as_str() {
        "macos" => Some(Platform::Macos),
        "linux" | "ubuntu" => Some(Platform::Ubuntu),
        "windows" => Some(Platform::Windows),
        _ => None,
    }
}
