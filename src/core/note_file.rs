use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre de la carpeta de papelera
const TRASH_DIR: &str = ".trash";
/// Nombre de la carpeta de historial
const HISTORY_DIR: &str = ".history";

/// Gestor de archivos .md para notas
#[derive(Debug, Clone)]
pub struct NoteFile {
    /// Ruta absoluta al archivo .md
    path: PathBuf,
    /// Nombre de la nota (sin extensión, puede incluir ruta relativa como "Docs VS/nota")
    pub(crate) name: String,
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

    /// Mueve el archivo a la papelera
    pub fn trash(self, notes_dir: &NotesDirectory) -> Result<()> {
        let trash_path = notes_dir.trash_path();
        if !trash_path.exists() {
            fs::create_dir_all(&trash_path)
                .context("No se pudo crear el directorio de papelera")?;
        }

        // Generar un nombre único con timestamp para evitar colisiones
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Preservar la estructura de carpetas en el nombre del archivo en la papelera
        // Ej: "Docs VS/nota.md" -> "Docs VS_nota_1234567890.md"
        let safe_name = self.name.replace('/', "_");
        let trash_filename = format!("{}_{}.md", safe_name, timestamp);
        let dest_path = trash_path.join(trash_filename);

        fs::rename(&self.path, &dest_path).context("No se pudo mover el archivo a la papelera")
    }

    /// Crea una copia de seguridad del archivo actual en el historial
    pub fn backup(&self, notes_dir: &NotesDirectory) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        let history_path = notes_dir.root().join(HISTORY_DIR);
        if !history_path.exists() {
            fs::create_dir_all(&history_path)
                .context("No se pudo crear directorio de historial")?;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let safe_name = self.name.replace('/', "_");
        let backup_filename = format!("{}_{}.md", safe_name, timestamp);
        let dest_path = history_path.join(backup_filename);

        fs::copy(&self.path, &dest_path).context("No se pudo crear backup")?;

        Ok(())
    }

    /// Elimina el archivo permanentemente
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

    /// Obtiene la ruta al directorio de papelera
    pub fn trash_path(&self) -> PathBuf {
        self.root.join(TRASH_DIR)
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

        // Ignorar la carpeta de papelera si estamos escaneando dentro de ella (no debería pasar con la lógica de abajo, pero por seguridad)
        if dir.ends_with(TRASH_DIR) || dir.ends_with(HISTORY_DIR) {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Ignorar la carpeta de papelera y historial
            if path.file_name().and_then(|s| s.to_str()) == Some(TRASH_DIR)
                || path.file_name().and_then(|s| s.to_str()) == Some(HISTORY_DIR)
            {
                continue;
            }

            if path.is_dir() {
                self.scan_directory(&path, notes)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(mut note) = NoteFile::open(&path) {
                    // Si la nota está en una subcarpeta, ajustar su nombre para incluir la ruta relativa
                    if let Ok(relative_path) = path.strip_prefix(&self.root) {
                        if let Some(parent) = relative_path.parent() {
                            if parent != Path::new("") {
                                // Reconstruir el nombre con la carpeta: "Docs VS/nombre"
                                let folder = parent.to_string_lossy();
                                let file_stem =
                                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                note.name = format!("{}/{}", folder, file_stem);
                            }
                        }
                    }
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
        let folder_path = self.root.join(folder);

        // Crear la carpeta si no existe (incluyendo carpetas padre)
        std::fs::create_dir_all(&folder_path)?;

        let path = folder_path.join(filename);
        NoteFile::create(path, content)
    }

    /// Busca una nota por nombre
    pub fn find_note(&self, name: &str) -> Result<Option<NoteFile>> {
        // Si el nombre empieza por .trash/, buscar directamente allí sin usar list_notes
        if name.starts_with(".trash/") {
            let path = self.root.join(format!("{}.md", name));
            if path.exists() {
                let mut note = NoteFile::open(&path)?;
                note.name = name.to_string(); // Forzar el nombre correcto con carpeta
                return Ok(Some(note));
            }
        }

        let notes = self.list_notes()?;

        // Primero intentar coincidencia exacta por nombre
        if let Some(note) = notes.iter().find(|n| n.name() == name).cloned() {
            return Ok(Some(note));
        }

        // Si el nombre tiene '/', intentar construir la ruta y buscar por ruta
        if name.contains('/') {
            let target_path = self.root.join(format!("{}.md", name));
            if let Some(note) = notes.iter().find(|n| n.path() == target_path).cloned() {
                return Ok(Some(note));
            }
        }

        // Si no se encuentra, buscar solo por el nombre base (sin carpeta)
        let base_name = name.split('/').last().unwrap_or(name);
        Ok(notes.into_iter().find(|n| {
            let note_base = n.name().split('/').last().unwrap_or(n.name());
            note_base == base_name
        }))
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

    #[test]
    fn test_trash_note() {
        let temp_dir = env::temp_dir().join("notnative_test_trash");
        let notes_dir = NotesDirectory::new(&temp_dir).unwrap();

        let note = notes_dir.create_note("trash_me", "Delete me").unwrap();

        let note_path = note.path().to_path_buf();
        assert!(note_path.exists());

        // Trash the note
        note.trash(&notes_dir).unwrap();

        // Original file should not exist
        assert!(!note_path.exists());

        // Trash directory should exist
        let trash_dir = notes_dir.trash_path();
        assert!(trash_dir.exists());

        // Should be a file in trash
        let entries: Vec<_> = fs::read_dir(&trash_dir)
            .unwrap()
            .map(|res| res.unwrap().path())
            .collect();

        assert_eq!(entries.len(), 1);
        let trashed_file = &entries[0];
        assert!(
            trashed_file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("trash_me_")
        );

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }
}
