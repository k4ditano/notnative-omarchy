use ropey::Rope;
use std::ops::Range;

/// Representa el estado de un buffer de texto usando Rope para edición eficiente.
#[derive(Debug, Clone)]
pub struct NoteBuffer {
    /// El contenido de texto subyacente, almacenado como Rope para operaciones O(log n)
    rope: Rope,
    /// Historial de operaciones para undo
    undo_stack: Vec<BufferEdit>,
    /// Historial de operaciones para redo
    redo_stack: Vec<BufferEdit>,
    /// Límite de operaciones en el historial
    max_history: usize,
}

/// Representa una edición atómica en el buffer
#[derive(Debug, Clone)]
struct BufferEdit {
    /// Tipo de operación realizada
    kind: EditKind,
    /// Rango afectado por la edición
    range: Range<usize>,
    /// Texto involucrado en la operación
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Insert,
    Delete,
}

impl NoteBuffer {
    /// Crea un nuevo buffer vacío
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 1000,
        }
    }

    /// Crea un buffer desde un texto existente
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 1000,
        }
    }

    /// Devuelve el contenido completo como String
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Devuelve el número de caracteres en el buffer
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Devuelve el número de líneas en el buffer
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Devuelve una referencia al rope interno para operaciones avanzadas
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Verifica si el buffer está vacío
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Inserta texto en una posición específica (índice de caracteres)
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        if char_idx > self.len_chars() {
            return;
        }

        self.rope.insert(char_idx, text);

        let char_len = text.chars().count();
        let edit = BufferEdit {
            kind: EditKind::Insert,
            range: char_idx..char_idx + char_len,
            text: text.to_string(),
        };

        self.push_undo(edit);
        self.redo_stack.clear();
    }

    /// Elimina un rango de caracteres
    pub fn delete(&mut self, range: Range<usize>) {
        if range.start >= self.len_chars() || range.end > self.len_chars() {
            return;
        }

        let deleted_text = self.rope.slice(range.clone()).to_string();
        self.rope.remove(range.clone());

        let edit = BufferEdit {
            kind: EditKind::Delete,
            range,
            text: deleted_text,
        };

        self.push_undo(edit);
        self.redo_stack.clear();
    }

    /// Reemplaza un rango de texto con nuevo contenido
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        self.delete(range.clone());
        self.insert(range.start, text);
    }

    /// Obtiene una línea específica como String
    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx >= self.len_lines() {
            return None;
        }
        Some(self.rope.line(line_idx).to_string())
    }

    /// Obtiene un slice de texto en un rango
    pub fn slice(&self, range: Range<usize>) -> Option<String> {
        if range.end > self.len_chars() {
            return None;
        }
        Some(self.rope.slice(range).to_string())
    }

    /// Deshace la última operación
    pub fn undo(&mut self) -> bool {
        if let Some(edit) = self.undo_stack.pop() {
            match edit.kind {
                EditKind::Insert => {
                    // Revertir inserción eliminando el texto insertado
                    self.rope.remove(edit.range.clone());
                }
                EditKind::Delete => {
                    // Revertir eliminación insertando el texto eliminado
                    self.rope.insert(edit.range.start, &edit.text);
                }
            }
            self.redo_stack.push(edit);
            true
        } else {
            false
        }
    }

    /// Rehace la última operación deshecha
    pub fn redo(&mut self) -> bool {
        if let Some(edit) = self.redo_stack.pop() {
            match edit.kind {
                EditKind::Insert => {
                    // Rehacer inserción
                    self.rope.insert(edit.range.start, &edit.text);
                }
                EditKind::Delete => {
                    // Rehacer eliminación
                    self.rope.remove(edit.range.clone());
                }
            }
            self.undo_stack.push(edit);
            true
        } else {
            false
        }
    }

    /// Limpia todo el contenido del buffer
    pub fn clear(&mut self) {
        let len = self.len_chars();
        if len > 0 {
            self.delete(0..len);
        }
    }

    /// Verifica si hay operaciones para deshacer
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Verifica si hay operaciones para rehacer
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Agrega una edición al stack de undo, respetando el límite
    fn push_undo(&mut self, edit: BufferEdit) {
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(edit);
    }

    /// Convierte un índice de línea y columna a índice de carácter
    pub fn line_col_to_char(&self, line: usize, col: usize) -> Option<usize> {
        if line >= self.len_lines() {
            return None;
        }
        let line_start = self.rope.line_to_char(line);
        let line_len = self.rope.line(line).len_chars();
        if col > line_len {
            return None;
        }
        Some(line_start + col)
    }

    /// Convierte un índice de carácter a línea y columna
    pub fn char_to_line_col(&self, char_idx: usize) -> Option<(usize, usize)> {
        if char_idx > self.len_chars() {
            return None;
        }
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let col = char_idx - line_start;
        Some((line, col))
    }
}

impl Default for NoteBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut buffer = NoteBuffer::new();
        assert!(buffer.is_empty());

        buffer.insert(0, "Hello");
        assert_eq!(buffer.to_string(), "Hello");
        assert_eq!(buffer.len_chars(), 5);

        buffer.insert(5, " World");
        assert_eq!(buffer.to_string(), "Hello World");
    }

    #[test]
    fn test_undo_redo() {
        let mut buffer = NoteBuffer::new();
        buffer.insert(0, "First");
        buffer.insert(5, " Second");
        assert_eq!(buffer.to_string(), "First Second");

        buffer.undo();
        assert_eq!(buffer.to_string(), "First");

        buffer.redo();
        assert_eq!(buffer.to_string(), "First Second");
    }

    #[test]
    fn test_delete() {
        let mut buffer = NoteBuffer::from_text("Hello World");
        buffer.delete(5..11);
        assert_eq!(buffer.to_string(), "Hello");
    }

    #[test]
    fn test_line_operations() {
        let buffer = NoteBuffer::from_text("Line 1\nLine 2\nLine 3");
        assert_eq!(buffer.len_lines(), 3);
        assert_eq!(buffer.line(0), Some("Line 1\n".to_string()));
        assert_eq!(buffer.line(1), Some("Line 2\n".to_string()));
    }

    #[test]
    fn test_line_col_conversion() {
        let buffer = NoteBuffer::from_text("abc\ndefgh\nij");

        // "abc\n" = 4 chars (línea 0)
        // "defgh\n" = 6 chars (línea 1)
        // "ij" = 2 chars (línea 2)

        assert_eq!(buffer.line_col_to_char(0, 0), Some(0)); // 'a'
        assert_eq!(buffer.line_col_to_char(0, 2), Some(2)); // 'c'
        assert_eq!(buffer.line_col_to_char(1, 0), Some(4)); // 'd'
        assert_eq!(buffer.line_col_to_char(2, 1), Some(11)); // 'j'

        assert_eq!(buffer.char_to_line_col(0), Some((0, 0)));
        assert_eq!(buffer.char_to_line_col(4), Some((1, 0)));
        assert_eq!(buffer.char_to_line_col(11), Some((2, 1)));
    }
}
