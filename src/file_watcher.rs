use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct FileWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

impl std::fmt::Debug for FileWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWatcher").finish()
    }
}

impl FileWatcher {
    pub fn new<F>(callback: F) -> Result<Self, notify::Error>
    where
        F: Fn(Event) + Send + 'static,
    {
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    callback(event);
                }
            },
            notify::Config::default(),
        )?;

        Ok(Self { watcher })
    }

    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.watcher.watch(path, RecursiveMode::Recursive)
    }
}

/// Crea un watcher que monitorea cambios en el directorio de notas
/// y actualiza la base de datos automáticamente
pub fn create_notes_watcher(
    notes_path: PathBuf,
    notes_db: Arc<Mutex<crate::core::database::NotesDatabase>>,
    sender: relm4::Sender<crate::app::AppMsg>,
) -> Result<FileWatcher, notify::Error> {
    let notes_root = notes_path.clone();

    let mut watcher = FileWatcher::new(move |event| {
        use notify::EventKind;

        match event.kind {
            // Detectar creación y modificación de archivos
            EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                for path in &event.paths {
                    // Solo procesar archivos .md
                    if path.extension().map_or(false, |e| e == "md") {
                        println!("📁 Detectado cambio en: {:?}", path);

                        if let Ok(content) = std::fs::read_to_string(path) {
                            // Extraer nombre de la nota
                            let name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("untitled");

                            // Detectar carpeta (si no está en la raíz)
                            let folder = path
                                .parent()
                                .and_then(|p| p.strip_prefix(&notes_root).ok())
                                .filter(|p| !p.as_os_str().is_empty())
                                .and_then(|p| p.to_str())
                                .map(|s| s.to_string());

                            // Indexar en la base de datos
                            if let Ok(db) = notes_db.lock() {
                                if let Err(e) = db.index_note(
                                    name,
                                    path.to_str().unwrap_or(""),
                                    &content,
                                    folder.as_deref(),
                                ) {
                                    eprintln!("⚠️ Error indexando nota automáticamente: {}", e);
                                } else {
                                    println!("✅ Nota indexada: {} (carpeta: {:?})", name, folder);

                                    // Si está en una carpeta, expandirla automáticamente
                                    if let Some(ref folder_name) = folder {
                                        let _ = sender.send(crate::app::AppMsg::ExpandFolder(
                                            folder_name.clone(),
                                        ));
                                    }

                                    // Refrescar sidebar
                                    let _ = sender.send(crate::app::AppMsg::RefreshSidebar);
                                }
                            }
                        }
                    }
                }
            }

            // Detectar eliminación de archivos
            EventKind::Remove(_) => {
                for path in &event.paths {
                    if path.extension().map_or(false, |e| e == "md") {
                        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                            println!("🗑️ Detectada eliminación: {}", name);

                            if let Ok(db) = notes_db.lock() {
                                if let Err(e) = db.delete_note(name) {
                                    eprintln!("⚠️ Error eliminando nota de BD: {}", e);
                                } else {
                                    println!("✅ Nota eliminada de BD: {}", name);
                                    let _ = sender.send(crate::app::AppMsg::RefreshSidebar);
                                }
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    })?;

    watcher.watch(&notes_path)?;
    println!("👁️ File watcher activado en: {:?}", notes_path);

    Ok(watcher)
}
