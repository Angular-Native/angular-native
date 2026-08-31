//! Localiza la raíz del repositorio y resuelve rutas de apps.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub struct Workspace {
    pub root: PathBuf,
}

impl Workspace {
    /// Sube desde el directorio actual hasta encontrar el Cargo.toml del
    /// workspace. Así `an` funciona desde cualquier subdirectorio.
    pub fn discover() -> Result<Self> {
        let mut dir = std::env::current_dir().context("no se pudo leer el directorio actual")?;
        loop {
            if dir.join("Cargo.toml").is_file() && dir.join("packages/runtime/runtime.js").is_file()
            {
                return Ok(Workspace { root: dir });
            }
            if !dir.pop() {
                bail!("no encuentro la raíz del proyecto angular-native desde aquí");
            }
        }
    }

    /// Ruta de la app, relativa a la raíz. Acepta absolutas y relativas.
    pub fn app(&self, given: Option<&str>) -> Result<PathBuf> {
        let raw = given.unwrap_or("examples/hello-angular");
        let absolute = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.root.join(raw)
        };
        if !absolute.join("tsconfig.json").is_file() {
            bail!("{} no parece una app: falta tsconfig.json", absolute.display());
        }
        let relative = absolute
            .strip_prefix(&self.root)
            .context("la app tiene que estar dentro del proyecto")?;
        Ok(relative.to_path_buf())
    }

    pub fn name(app: &Path) -> String {
        app.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "app".to_owned())
    }
}
