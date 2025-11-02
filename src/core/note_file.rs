use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Gestor de archivos .md para notas
#[derive(Debug, Clone)]
pub struct NoteFile {
    /// Ruta absoluta al archivo .md
    path: PathBuf,
    /// Nombre de la nota (sin extensión)
    name: String,
}

impl NoteFile {
    /// Crea una referencia a un archivo de nota existente
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            anyhow::bail!("El archivo no existe: {:?}", path);
        }

        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            anyhow::bail!("El archivo debe tener extensión .md");
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("No se pudo obtener el nombre del archivo")?
            .to_string();

        Ok(Self { path, name })
    }

    /// Crea un nuevo archivo de nota
    pub fn create<P: AsRef<Path>>(path: P, initial_content: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Crear directorios padres si no existen
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("No se pudo crear el directorio padre")?;
        }

        // Escribir contenido inicial
        fs::write(&path, initial_content).context("No se pudo escribir el archivo")?;

        Self::open(path)
    }

    /// Lee el contenido del archivo
    pub fn read(&self) -> Result<String> {
        fs::read_to_string(&self.path).context("No se pudo leer el archivo")
    }

    /// Escribe contenido al archivo
    pub fn write(&self, content: &str) -> Result<()> {
        fs::write(&self.path, content).context("No se pudo escribir el archivo")
    }

    /// Devuelve la ruta del archivo
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Devuelve el nombre de la nota
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renombra el archivo
    pub fn rename<P: AsRef<Path>>(&mut self, new_name: P) -> Result<()> {
        let new_path = self
            .path
            .parent()
            .context("No se pudo obtener el directorio padre")?
            .join(new_name.as_ref());

        fs::rename(&self.path, &new_path).context("No se pudo renombrar el archivo")?;

        self.path = new_path;
        self.name = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("No se pudo obtener el nuevo nombre")?
            .to_string();

        Ok(())
    }

    /// Elimina el archivo
    pub fn delete(self) -> Result<()> {
        fs::remove_file(&self.path).context("No se pudo eliminar el archivo")
    }
}

/// Gestor del directorio de notas
#[derive(Debug, Clone)]
pub struct NotesDirectory {
    /// Ruta al directorio raíz de notas
    root: PathBuf,
}

impl NotesDirectory {
    /// Crea o abre un directorio de notas
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let root = root.as_ref().to_path_buf();

        if !root.exists() {
            fs::create_dir_all(&root).context("No se pudo crear el directorio de notas")?;
        }

        Ok(Self { root })
    }

    /// Obtiene la ruta al directorio raíz
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Obtiene la ruta al archivo de base de datos
    pub fn db_path(&self) -> PathBuf {
        self.root.parent().unwrap_or(&self.root).join("notes.db")
    }

    /// Obtiene la carpeta relativa de una nota (si está en una subcarpeta)
    pub fn relative_folder(&self, note_path: &Path) -> Option<String> {
        note_path
            .parent()
            .and_then(|p| p.strip_prefix(&self.root).ok())
            .filter(|p| p != &Path::new(""))
            .map(|p| p.to_string_lossy().to_string())
    }

    /// Lista todas las notas en el directorio (recursivo)
    pub fn list_notes(&self) -> Result<Vec<NoteFile>> {
        let mut notes = Vec::new();
        self.scan_directory(&self.root, &mut notes)?;
        Ok(notes)
    }

    fn scan_directory(&self, dir: &Path, notes: &mut Vec<NoteFile>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_directory(&path, notes)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(note) = NoteFile::open(&path) {
                    notes.push(note);
                }
            }
        }

        Ok(())
    }

    /// Crea una nueva nota en el directorio raíz
    pub fn create_note(&self, name: &str, content: &str) -> Result<NoteFile> {
        let filename = format!("{}.md", name);
        let path = self.root.join(filename);
        NoteFile::create(path, content)
    }

    /// Crea una nueva nota en una subcarpeta
    pub fn create_note_in_folder(
        &self,
        folder: &str,
        name: &str,
        content: &str,
    ) -> Result<NoteFile> {
        let filename = format!("{}.md", name);
        let path = self.root.join(folder).join(filename);
        NoteFile::create(path, content)
    }

    /// Busca una nota por nombre
    pub fn find_note(&self, name: &str) -> Result<Option<NoteFile>> {
        let notes = self.list_notes()?;
        Ok(notes.into_iter().find(|n| n.name() == name))
    }
}

impl Default for NotesDirectory {
    fn default() -> Self {
        // Por defecto usar ~/.local/share/notnative/notes
        let home = dirs::home_dir().expect("No se pudo obtener el directorio home");
        let root = home.join(".local/share/notnative/notes");
        Self::new(root).expect("No se pudo crear el directorio de notas por defecto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_create_and_read_note() {
        let temp_dir = env::temp_dir().join("notnative_test");
        let notes_dir = NotesDirectory::new(&temp_dir).unwrap();

        let note = notes_dir
            .create_note("test", "# Test Note\n\nHello World")
            .unwrap();
        assert_eq!(note.name(), "test");

        let content = note.read().unwrap();
        assert_eq!(content, "# Test Note\n\nHello World");

        // Cleanup
        note.delete().unwrap();
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_list_notes() {
        let temp_dir = env::temp_dir().join("notnative_test_list");
        let notes_dir = NotesDirectory::new(&temp_dir).unwrap();

        notes_dir.create_note("note1", "Content 1").unwrap();
        notes_dir.create_note("note2", "Content 2").unwrap();

        let notes = notes_dir.list_notes().unwrap();
        assert_eq!(notes.len(), 2);

        // Cleanup
        for note in notes {
            note.delete().unwrap();
        }
        let _ = fs::remove_dir_all(temp_dir);
    }
}
