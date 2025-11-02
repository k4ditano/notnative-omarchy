use crate::core::EditorMode;

/// Acciones que el editor puede realizar en respuesta a comandos
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// Cambiar de modo
    ChangeMode(EditorMode),

    /// Movimientos del cursor
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorLineStart,
    MoveCursorLineEnd,
    MoveCursorDocStart,
    MoveCursorDocEnd,

    /// Edición
    InsertChar(char),
    InsertNewline,
    DeleteCharBefore,
    DeleteCharAfter,
    DeleteLine,
    DeleteSelection,

    /// Insertar imagen
    InsertImage,

    /// Undo/Redo
    Undo,
    Redo,

    /// Portapapeles
    Copy,
    Cut,
    Paste,

    /// Comandos ex-style
    Save,
    Quit,
    SaveAndQuit,
    ForceQuit,

    /// Búsqueda
    Search(String),

    /// Sidebar
    OpenSidebar,
    CloseSidebar,

    /// Crear nueva nota
    CreateNote,

    /// Sin acción
    None,
}

/// Parser de comandos estilo vim
#[derive(Debug)]
pub struct CommandParser {
    /// Buffer acumulativo para comandos multi-tecla (ej: "dd", "2w")
    pending: String,
}

impl CommandParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// Procesa una tecla en modo Normal y devuelve una acción
    pub fn parse_normal_mode(&mut self, key: &str, modifiers: KeyModifiers) -> EditorAction {
        // Comandos con modificadores (Ctrl, Alt)
        if modifiers.ctrl {
            return match key {
                "s" => EditorAction::Save,
                "z" => EditorAction::Undo,
                "r" => EditorAction::Redo,
                "c" => EditorAction::Copy,
                "x" => EditorAction::Cut,
                "v" => EditorAction::Paste,
                _ => EditorAction::None,
            };
        }

        // Comandos de una sola tecla
        match key {
            "i" => EditorAction::ChangeMode(EditorMode::Insert),
            ":" => EditorAction::ChangeMode(EditorMode::Command),
            "v" => EditorAction::ChangeMode(EditorMode::Visual),
            "t" => EditorAction::OpenSidebar,
            "n" => EditorAction::CreateNote,

            // Movimientos básicos (vim-style)
            "h" | "Left" => EditorAction::MoveCursorLeft,
            "j" | "Down" => EditorAction::MoveCursorDown,
            "k" | "Up" => EditorAction::MoveCursorUp,
            "l" | "Right" => EditorAction::MoveCursorRight,

            // Movimientos de línea
            "0" => EditorAction::MoveCursorLineStart,
            "$" => EditorAction::MoveCursorLineEnd,
            "g" if self.pending == "g" => {
                self.pending.clear();
                EditorAction::MoveCursorDocStart
            }
            "g" => {
                self.pending.push_str("g");
                EditorAction::None
            }
            "G" => EditorAction::MoveCursorDocEnd,

            // Edición
            "x" => EditorAction::DeleteCharAfter,
            "d" if self.pending == "d" => {
                self.pending.clear();
                EditorAction::DeleteLine
            }
            "d" => {
                self.pending.push_str("d");
                EditorAction::None
            }

            "u" => EditorAction::Undo,

            // ESC en modo Normal: cerrar sidebar si está abierto
            "Escape" => {
                self.pending.clear();
                EditorAction::CloseSidebar
            }

            _ => {
                self.pending.clear();
                EditorAction::None
            }
        }
    }

    /// Procesa entrada en modo Insert
    pub fn parse_insert_mode(&mut self, key: &str, modifiers: KeyModifiers) -> EditorAction {
        if key == "Escape" {
            return EditorAction::ChangeMode(EditorMode::Normal);
        }

        if modifiers.ctrl {
            if modifiers.shift {
                return match key {
                    "i" | "I" => EditorAction::InsertImage,
                    _ => EditorAction::None,
                };
            }

            return match key {
                "s" => EditorAction::Save,
                "c" => EditorAction::Copy,
                "x" => EditorAction::Cut,
                "v" => EditorAction::Paste,
                "z" => EditorAction::Undo,
                "r" => EditorAction::Redo,
                _ => EditorAction::None,
            };
        }

        match key {
            "Return" | "Enter" => EditorAction::InsertNewline,
            "BackSpace" => EditorAction::DeleteCharBefore,
            "Delete" => EditorAction::DeleteCharAfter,
            "Left" => EditorAction::MoveCursorLeft,
            "Right" => EditorAction::MoveCursorRight,
            "Up" => EditorAction::MoveCursorUp,
            "Down" => EditorAction::MoveCursorDown,
            "space" => EditorAction::InsertChar(' '),
            "Tab" => EditorAction::InsertChar('\t'),

            // Caracteres especiales comunes que GTK reporta con nombres específicos
            "period" => EditorAction::InsertChar('.'),
            "comma" => EditorAction::InsertChar(','),
            "semicolon" => EditorAction::InsertChar(';'),
            "colon" => EditorAction::InsertChar(':'),
            "exclam" => EditorAction::InsertChar('!'),
            "question" => EditorAction::InsertChar('?'),
            "slash" => EditorAction::InsertChar('/'),
            "backslash" => EditorAction::InsertChar('\\'),
            "minus" => EditorAction::InsertChar('-'),
            "underscore" => EditorAction::InsertChar('_'),
            "equal" => EditorAction::InsertChar('='),
            "plus" => EditorAction::InsertChar('+'),
            "asterisk" => EditorAction::InsertChar('*'),
            "ampersand" => EditorAction::InsertChar('&'),
            "percent" => EditorAction::InsertChar('%'),
            "numbersign" => EditorAction::InsertChar('#'),
            "at" => EditorAction::InsertChar('@'),
            "dollar" => EditorAction::InsertChar('$'),
            "parenleft" => EditorAction::InsertChar('('),
            "parenright" => EditorAction::InsertChar(')'),
            "bracketleft" => EditorAction::InsertChar('['),
            "bracketright" => EditorAction::InsertChar(']'),
            "braceleft" => EditorAction::InsertChar('{'),
            "braceright" => EditorAction::InsertChar('}'),
            "less" => EditorAction::InsertChar('<'),
            "greater" => EditorAction::InsertChar('>'),
            "quotedbl" => EditorAction::InsertChar('"'),
            "apostrophe" => EditorAction::InsertChar('\''),
            "grave" => EditorAction::InsertChar('`'),
            "asciitilde" => EditorAction::InsertChar('~'),
            "bar" => EditorAction::InsertChar('|'),
            "asciicircum" => EditorAction::InsertChar('^'),

            _ => {
                // Si es un carácter imprimible de longitud 1, insertarlo
                if key.chars().count() == 1 {
                    if let Some(ch) = key.chars().next() {
                        return EditorAction::InsertChar(ch);
                    }
                }
                EditorAction::None
            }
        }
    }

    /// Procesa entrada en modo Command (para comandos ex-style como :w, :q)
    pub fn parse_command_mode(&mut self, command: &str) -> EditorAction {
        let trimmed = command.trim();
        match trimmed {
            "w" | "write" => EditorAction::Save,
            "q" | "quit" => EditorAction::Quit,
            "wq" | "x" => EditorAction::SaveAndQuit,
            "q!" => EditorAction::ForceQuit,
            _ if trimmed.starts_with('/') => EditorAction::Search(trimmed[1..].to_string()),
            _ => EditorAction::None,
        }
    }

    /// Limpia el buffer de comandos pendientes
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

impl Default for CommandParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Modificadores de teclado
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_mode_basic() {
        let mut parser = CommandParser::new();
        let mods = KeyModifiers::default();

        assert_eq!(
            parser.parse_normal_mode("i", mods),
            EditorAction::ChangeMode(EditorMode::Insert)
        );
        assert_eq!(
            parser.parse_normal_mode("h", mods),
            EditorAction::MoveCursorLeft
        );
        assert_eq!(
            parser.parse_normal_mode("l", mods),
            EditorAction::MoveCursorRight
        );
    }

    #[test]
    fn test_normal_mode_multi_key() {
        let mut parser = CommandParser::new();
        let mods = KeyModifiers::default();

        // Primer 'd' no hace nada
        assert_eq!(parser.parse_normal_mode("d", mods), EditorAction::None);
        // Segundo 'd' ejecuta DeleteLine
        assert_eq!(
            parser.parse_normal_mode("d", mods),
            EditorAction::DeleteLine
        );
    }

    #[test]
    fn test_insert_mode() {
        let mut parser = CommandParser::new();
        let mods = KeyModifiers::default();

        assert_eq!(
            parser.parse_insert_mode("a", mods),
            EditorAction::InsertChar('a')
        );
        assert_eq!(
            parser.parse_insert_mode("Return", mods),
            EditorAction::InsertNewline
        );
        assert_eq!(
            parser.parse_insert_mode("Escape", mods),
            EditorAction::ChangeMode(EditorMode::Normal)
        );
    }

    #[test]
    fn test_command_mode() {
        let mut parser = CommandParser::new();

        assert_eq!(parser.parse_command_mode("w"), EditorAction::Save);
        assert_eq!(parser.parse_command_mode("q"), EditorAction::Quit);
        assert_eq!(parser.parse_command_mode("wq"), EditorAction::SaveAndQuit);
        assert_eq!(
            parser.parse_command_mode("/search"),
            EditorAction::Search("search".to_string())
        );
    }
}
