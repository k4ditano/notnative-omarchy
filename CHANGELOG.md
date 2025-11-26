# Changelog

All notable changes to NotNative will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2025-01-XX

### Added
- **🌐 WebView HTML Preview**: Nuevo renderizado estilo Obsidian en Modo Normal
  - Vista previa HTML completa con webkit6::WebView
  - Renderizado de Markdown a HTML en tiempo real
  - Diseño centrado con ancho máximo de 900px para mejor legibilidad
  
- **⌨️ Navegación por Teclado en Preview**: Scroll completo en Modo Normal
  - Flechas arriba/abajo para scroll suave
  - j/k estilo Vim para scroll
  - PgUp/PgDown para páginas completas
  - Home/End y g/G para inicio/fin del documento
  
- **☑️ TODOs Interactivos**: Checkboxes clickeables en vista WebView
  - Click en checkboxes marca/desmarca tareas
  - Sincronización automática con el archivo fuente
  - Feedback visual inmediato
  
- **🔗 Backlinks con @menciones**: Sistema de referencias entre notas
  - Autocompletado al escribir `@` + texto
  - Navegación por click en menciones
  - Popover con hasta 8 sugerencias
  
- **📂 Abrir en Explorador**: Nueva opción en menú contextual
  - Click derecho en notas/carpetas → "Abrir en explorador"
  - Compatible con todos los gestores de archivos Linux

- **🔗 Detección Automática de URLs**: Conversión inteligente al pegar
  - URLs normales se convierten a enlaces markdown automáticamente

### Fixed
- **🔧 Focus en Sidebar**: Navegación por sidebar mantiene foco correctamente
  - LoadNoteFromSidebar para cargar notas sin robar foco
  - sync_to_view_no_focus() para sincronizar sin cambiar foco
  
- **🏷️ Tags YAML con Caracteres Especiales**: Decodificación URL correcta
  - url_decode() para caracteres como %C3%B3 → ó
  - Tags con acentos y caracteres especiales funcionan correctamente
  
- **🎨 Diseño Centrado en Insert Mode**: Consistencia visual
  - TextView usa spacers con hexpand para centrado
  - Mismo ancho visual que WebView preview

### Technical
- html_renderer.rs: Módulo completo de Markdown→HTML
- webview_key_controller: Manejo de teclado en WebView con evaluate_javascript
- CSS body con padding 24px y .content con max-width 900px

---

## [0.1.1] - Previous Release

### Added
- Full-text search with SQLite FTS5
- Tag system with auto-completion
- Folder organization
- Image preview support

---

## [0.1.0] - 2024-XX-XX

### Added
- Initial release
- Vim-inspired modal editing (Normal, Insert, Visual, Command)
- Real-time Markdown rendering
- Interactive TODO checkboxes
- Basic note management (create, edit, delete, rename)
- Folder support with nested structure
- GTK4 interface with Omarchy theme integration

---

## Legend

- 🔗 Links & Navigation
- 📂 File Management
- 🏷️ Tags & Organization
- 🔍 Search & Discovery
- 🤖 AI & Automation
- 🎵 Media & Audio
- 🎨 UI/UX Improvements
- ⌨️ Keyboard & Input
- 🔧 Technical Changes
- 📚 Documentation

