//! Módulo de renderizado Markdown → HTML para el modo Normal (preview)
//!
//! Convierte el contenido Markdown a HTML completo con:
//! - Checkboxes interactivos para TODOs
//! - Links internos [[nota]] clickeables
//! - Syntax highlighting en code blocks (highlight.js)
//! - Soporte para tema claro/oscuro

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
use regex::Regex;
use std::path::PathBuf;

/// Decodifica una cadena URL-encoded (percent-encoded)
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Intentar leer dos caracteres hexadecimales
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    // Para UTF-8, necesitamos acumular bytes
                    let mut bytes = vec![byte];

                    // Verificar si hay más bytes UTF-8
                    while chars.peek() == Some(&'%') {
                        chars.next(); // consumir '%'
                        let next_hex: String = chars.by_ref().take(2).collect();
                        if next_hex.len() == 2 {
                            if let Ok(next_byte) = u8::from_str_radix(&next_hex, 16) {
                                if next_byte & 0xC0 == 0x80 {
                                    // Es un byte de continuación UTF-8
                                    bytes.push(next_byte);
                                } else {
                                    // No es continuación, devolver al flujo
                                    result.push_str(&format!("%{}", next_hex));
                                    break;
                                }
                            }
                        }
                    }

                    if let Ok(decoded) = String::from_utf8(bytes) {
                        result.push_str(&decoded);
                    } else {
                        result.push(c);
                        result.push_str(&hex);
                    }
                } else {
                    result.push(c);
                    result.push_str(&hex);
                }
            } else {
                result.push(c);
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Tema de colores para el preview
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewTheme {
    Light,
    Dark,
}

impl Default for PreviewTheme {
    fn default() -> Self {
        PreviewTheme::Dark
    }
}

/// Renderer de Markdown a HTML
pub struct HtmlRenderer {
    theme: PreviewTheme,
    base_path: Option<PathBuf>, // Directorio base para resolver rutas relativas de imágenes
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new(PreviewTheme::default())
    }
}

impl HtmlRenderer {
    pub fn new(theme: PreviewTheme) -> Self {
        Self {
            theme,
            base_path: None,
        }
    }

    /// Crea un renderer con un directorio base para resolver imágenes
    pub fn with_base_path(theme: PreviewTheme, base_path: PathBuf) -> Self {
        Self {
            theme,
            base_path: Some(base_path),
        }
    }

    /// Renderiza Markdown a HTML completo (documento completo con estilos)
    pub fn render(&self, markdown: &str) -> String {
        let body_html = self.render_body(markdown);
        self.wrap_in_document(&body_html)
    }

    /// Renderiza solo el body HTML (sin wrapper del documento)
    pub fn render_body(&self, markdown: &str) -> String {
        // Pre-procesar para TODOs y links internos
        let processed = self.preprocess_markdown(markdown);

        // Configurar parser de pulldown-cmark
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_FOOTNOTES);

        let parser = Parser::new_ext(&processed, options);

        // Procesar eventos para añadir atributos custom
        let parser = self.process_events(parser, markdown);

        // Generar HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser.into_iter());

        // Post-procesar para añadir data attributes y handlers
        self.postprocess_html(&html_output)
    }

    /// Pre-procesa el markdown para convertir sintaxis custom
    fn preprocess_markdown(&self, markdown: &str) -> String {
        let mut result = markdown.to_string();

        // Convertir [[nota]] a links especiales (placeholder que post-procesaremos)
        // URL-encode el nombre para manejar espacios y caracteres especiales
        let internal_link_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
        result = internal_link_re
            .replace_all(&result, |caps: &regex::Captures| {
                let note_name = &caps[1];
                let encoded_name = note_name.replace(' ', "%20");
                format!(r#"[{}](notnative://note/{})"#, note_name, encoded_name)
            })
            .to_string();

        // Convertir #tags a links clickeables (pero no # de headings)
        // Patrón: # seguido de letras/números/guiones, precedido por espacio o inicio de línea
        let tag_re = Regex::new(r"(?m)(^|[\s\(\[,])#([a-zA-Z][a-zA-Z0-9_-]*)").unwrap();
        result = tag_re
            .replace_all(&result, r#"$1[#$2](notnative://tag/$2)"#)
            .to_string();

        // Convertir tags de frontmatter YAML (formato: "  - tagname" o "  • tagname")
        // Solo dentro de bloques que empiecen con "tags:"
        let frontmatter_tag_re =
            Regex::new(r"(?m)^(\s*[-•]\s+)([a-zA-ZáéíóúñÁÉÍÓÚÑ][a-zA-Z0-9áéíóúñÁÉÍÓÚÑ_-]*)$")
                .unwrap();
        result = frontmatter_tag_re
            .replace_all(&result, |caps: &regex::Captures| {
                let prefix = &caps[1];
                let tag_name = &caps[2];
                format!("{}[{}](notnative://tag/{})", prefix, tag_name, tag_name)
            })
            .to_string();

        // Embeber videos de YouTube como iframes
        // Detectar URLs de YouTube y convertirlas a embeds
        // Patrón: youtube.com/watch?v=ID, youtu.be/ID, youtube.com/shorts/ID
        let youtube_patterns = [
            (
                r"https?://(?:www\.)?youtube\.com/watch\?v=([a-zA-Z0-9_-]{11})",
                "watch",
            ),
            (r"https?://youtu\.be/([a-zA-Z0-9_-]{11})", "short"),
            (
                r"https?://(?:www\.)?youtube\.com/shorts/([a-zA-Z0-9_-]{11})",
                "shorts",
            ),
        ];

        for (pattern, _) in youtube_patterns.iter() {
            let re = Regex::new(pattern).unwrap();
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let video_id = &caps[1];
                    format!(
                        r#"<div class="youtube-embed"><iframe src="https://www.youtube.com/embed/{}" frameborder="0" allowfullscreen></iframe></div>"#,
                        video_id
                    )
                })
                .to_string();
        }

        // Convertir recordatorios con formato especial
        // Detectar patrones como: 📅 2025-01-15 10:00 - Recordatorio texto
        // o: ⏰ mañana 9:00 - Recordatorio texto
        let reminder_re = Regex::new(r"(?m)^(📅|⏰|🔔)\s*(.+)$").unwrap();
        result = reminder_re
            .replace_all(&result, r#"<span class="reminder">$1 $2</span>"#)
            .to_string();

        result
    }

    /// Procesa eventos del parser para personalizar el output
    #[allow(unused_assignments)]
    fn process_events<'a>(&self, parser: Parser<'a>, original_markdown: &'a str) -> Vec<Event<'a>> {
        let lines: Vec<&str> = original_markdown.lines().collect();
        let mut events: Vec<Event<'a>> = Vec::new();
        let mut current_line = 0;
        let mut in_list_item = false;
        let mut list_item_line = 0;

        for event in parser {
            match &event {
                Event::Start(Tag::Item) => {
                    in_list_item = true;
                    // Encontrar la línea actual basándonos en el contexto
                    list_item_line = current_line;
                }
                Event::End(TagEnd::Item) => {
                    in_list_item = false;
                }
                Event::TaskListMarker(checked) => {
                    // Convertir el marcador de tarea en un checkbox interactivo
                    // Lo haremos en post-procesamiento para tener más control
                    events.push(event.clone());
                    continue;
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_line += 1;
                }
                Event::Text(text) => {
                    if text.contains('\n') {
                        current_line += text.matches('\n').count();
                    }
                }
                _ => {}
            }
            events.push(event);
        }

        events
    }

    /// Post-procesa el HTML para añadir interactividad
    fn postprocess_html(&self, html: &str) -> String {
        let mut result = html.to_string();

        // Añadir data-line a los checkboxes de tareas y hacerlos interactivos
        // pulldown-cmark puede generar varios formatos:
        // - <input type="checkbox" disabled="" />
        // - <input type="checkbox" disabled="" checked="" />
        // - <input disabled="" type="checkbox" />
        // - <input type="checkbox" disabled />
        // - <input type="checkbox" disabled="" /> (con o sin checked)
        // Usamos un regex muy flexible que captura cualquier <input> con checkbox Y disabled
        let checkbox_re =
            Regex::new(r#"<input\s+[^>]*(?:type\s*=\s*["']checkbox["'][^>]*disabled|disabled[^>]*type\s*=\s*["']checkbox["'])[^>]*/?\s*>"#).unwrap();

        let mut line_counter = 0;

        // Reemplazar todos los checkboxes deshabilitados con versiones interactivas
        result = checkbox_re
            .replace_all(&result, |caps: &regex::Captures| {
                line_counter += 1;
                let original = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                let is_checked = original.contains("checked");
                let checked_attr = if is_checked { " checked" } else { "" };
                format!(
                    r#"<input type="checkbox" class="todo-checkbox" data-line="{}" onclick="handleTodoClick(event, {}, this.checked)"{}>"#,
                    line_counter, line_counter, checked_attr
                )
            })
            .to_string();

        // Convertir links internos notnative://note/nombre a clickeables
        // El note_name puede venir URL-encoded (ej: My%20Note), hay que decodificarlo
        let internal_link_re =
            Regex::new(r#"<a href="notnative://note/([^"]+)">([^<]+)</a>"#).unwrap();
        result = internal_link_re
            .replace_all(&result, |caps: &regex::Captures| {
                let note_name_encoded = &caps[1];
                let note_name = url_decode(note_name_encoded);
                let display_text = &caps[2];
                format!(
                    "<a href=\"#\" class=\"internal-link\" data-note=\"{}\" onclick=\"notifyRust(&quot;open-note&quot;, &quot;{}&quot;); return false;\">{}</a>",
                    note_name, note_name, display_text
                )
            })
            .to_string();

        // Convertir links de tags notnative://tag/nombre a clickeables
        // El tag_name puede venir URL-encoded (ej: programaci%C3%B3n), hay que decodificarlo
        let tag_link_re = Regex::new(r#"<a href="notnative://tag/([^"]+)">([^<]+)</a>"#).unwrap();
        result = tag_link_re
            .replace_all(&result, |caps: &regex::Captures| {
                let tag_name_encoded = &caps[1];
                let tag_name = url_decode(tag_name_encoded);
                let display_text = &caps[2];
                format!(
                    "<a href=\"#\" class=\"tag-link\" data-tag=\"{}\" onclick=\"notifyRust(&quot;search-tag&quot;, &quot;{}&quot;); return false;\">{}</a>",
                    tag_name, tag_name, display_text
                )
            })
            .to_string();

        // Convertir rutas de imágenes locales a file:// URLs
        // Detectar <img src="path"> donde path no empieza con http:// o https://
        let img_re = Regex::new(r#"<img src="([^"]+)""#).unwrap();
        result = img_re
            .replace_all(&result, |caps: &regex::Captures| {
                let src = &caps[1];
                // Si ya es una URL http/https, dejarla como está
                if src.starts_with("http://")
                    || src.starts_with("https://")
                    || src.starts_with("file://")
                {
                    format!(r#"<img src="{}""#, src)
                } else if src.starts_with('/') {
                    // Ruta absoluta: añadir file://
                    format!(r#"<img src="file://{}""#, src)
                } else if let Some(ref base) = self.base_path {
                    // Ruta relativa: resolver contra base_path
                    let full_path = base.join(src);
                    format!(r#"<img src="file://{}""#, full_path.display())
                } else {
                    // Sin base_path, intentar como ruta relativa con file://
                    format!(r#"<img src="file://{}""#, src)
                }
            })
            .to_string();

        // Añadir wrapper de contenido (sin handler de click - los keybindings controlan el modo)
        result = format!(r#"<div class="content">{}</div>"#, result);

        result
    }

    /// Envuelve el body HTML en un documento completo con estilos y scripts
    fn wrap_in_document(&self, body: &str) -> String {
        let css = self.get_css();
        let js = self.get_javascript();

        format!(
            r#"<!DOCTYPE html>
<html lang="es">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
{css}
    </style>
</head>
<body class="{theme_class}">
    {body}
    <script>
{js}
    </script>
</body>
</html>"#,
            css = css,
            theme_class = match self.theme {
                PreviewTheme::Light => "light",
                PreviewTheme::Dark => "dark",
            },
            body = body,
            js = js
        )
    }

    /// Retorna el CSS para el preview
    fn get_css(&self) -> String {
        r#"
:root {
    --bg-primary: #1e1e2e;
    --bg-secondary: #313244;
    --bg-tertiary: #45475a;
    --fg-primary: #cdd6f4;
    --fg-secondary: #a6adc8;
    --fg-muted: #6c7086;
    --accent: #89b4fa;
    --accent-hover: #b4befe;
    --green: #a6e3a1;
    --red: #f38ba8;
    --yellow: #f9e2af;
    --peach: #fab387;
    --code-bg: #181825;
    --border: #45475a;
    --link: #89dceb;
    --link-internal: #cba6f7;
}

body.light {
    --bg-primary: #eff1f5;
    --bg-secondary: #e6e9ef;
    --bg-tertiary: #ccd0da;
    --fg-primary: #4c4f69;
    --fg-secondary: #5c5f77;
    --fg-muted: #8c8fa1;
    --accent: #1e66f5;
    --accent-hover: #7287fd;
    --green: #40a02b;
    --red: #d20f39;
    --yellow: #df8e1d;
    --peach: #fe640b;
    --code-bg: #dce0e8;
    --border: #bcc0cc;
    --link: #209fb5;
    --link-internal: #8839ef;
}

* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}

body {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    font-size: 16px;
    line-height: 1.7;
    color: var(--fg-primary);
    background-color: var(--bg-primary);
    padding: 24px;
    max-width: 100%;
    overflow-x: hidden;
}

.content {
    width: 100%;
    max-width: 900px;
    margin: 0 auto;
}

/* Headings */
h1, h2, h3, h4, h5, h6 {
    font-weight: 600;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    color: var(--fg-primary);
    line-height: 1.3;
}

h1 {
    font-size: 2em;
    border-bottom: 2px solid var(--border);
    padding-bottom: 0.3em;
}

h2 {
    font-size: 1.5em;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.2em;
}

h3 { font-size: 1.25em; }
h4 { font-size: 1.1em; }
h5 { font-size: 1em; }
h6 { font-size: 0.9em; color: var(--fg-secondary); }

/* Paragraphs */
p {
    margin-bottom: 1em;
}

/* Links */
a {
    color: var(--link);
    text-decoration: none;
    border-bottom: 1px solid transparent;
    transition: border-color 0.2s, color 0.2s;
}

a:hover {
    border-bottom-color: var(--link);
}

a.internal-link {
    color: var(--link-internal);
    background-color: rgba(139, 92, 246, 0.1);
    padding: 0 4px;
    border-radius: 4px;
}

a.internal-link:hover {
    background-color: rgba(139, 92, 246, 0.2);
}

/* Tags (#tag) */
a.tag-link {
    color: var(--yellow);
    background-color: rgba(249, 226, 175, 0.15);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.9em;
    font-weight: 500;
    border: none;
}

a.tag-link:hover {
    background-color: rgba(249, 226, 175, 0.3);
    border: none;
}

/* Recordatorios */
.reminder {
    display: inline-block;
    background-color: rgba(166, 227, 161, 0.15);
    border-left: 3px solid var(--green);
    padding: 4px 12px;
    border-radius: 0 6px 6px 0;
    margin: 4px 0;
    font-size: 0.95em;
}

/* Code */
code {
    font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 0.9em;
    background-color: var(--code-bg);
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--peach);
}

pre {
    background-color: var(--code-bg);
    padding: 16px;
    border-radius: 8px;
    overflow-x: auto;
    margin: 1em 0;
    border: 1px solid var(--border);
}

pre code {
    background: none;
    padding: 0;
    font-size: 0.85em;
    color: var(--fg-primary);
}

/* Blockquotes */
blockquote {
    border-left: 4px solid var(--accent);
    margin: 1em 0;
    padding: 0.5em 1em;
    background-color: var(--bg-secondary);
    border-radius: 0 8px 8px 0;
    color: var(--fg-secondary);
    font-style: italic;
}

blockquote p {
    margin-bottom: 0.5em;
}

blockquote p:last-child {
    margin-bottom: 0;
}

/* Lists */
ul, ol {
    margin: 1em 0;
    padding-left: 2em;
}

li {
    margin-bottom: 0.5em;
}

li > ul, li > ol {
    margin: 0.25em 0;
}

/* Task lists (TODOs) */
ul.contains-task-list,
li.task-list-item {
    list-style: none;
}

ul.contains-task-list {
    padding-left: 0;
    margin-left: 0;
}

li.task-list-item {
    position: relative;
    padding-left: 32px;
    margin-bottom: 4px;
}

input.todo-checkbox,
input[type="checkbox"] {
    position: absolute;
    left: 0;
    top: 2px;
    width: 20px;
    height: 20px;
    cursor: pointer;
    accent-color: var(--green);
    margin: 0;
    padding: 0;
    appearance: auto;
    -webkit-appearance: checkbox;
    z-index: 10;
}

input.todo-checkbox:hover,
input[type="checkbox"]:hover {
    transform: scale(1.1);
    box-shadow: 0 0 4px var(--accent);
}

input[type="checkbox"]:checked + *,
li.task-list-item:has(input:checked) > span,
li.task-list-item:has(input:checked) > p {
    text-decoration: line-through;
    color: var(--fg-muted);
}

/* Tables */
table {
    width: 100%;
    border-collapse: collapse;
    margin: 1em 0;
    font-size: 0.95em;
}

th, td {
    border: 1px solid var(--border);
    padding: 10px 14px;
    text-align: left;
}

th {
    background-color: var(--bg-secondary);
    font-weight: 600;
}

tr:nth-child(even) {
    background-color: var(--bg-secondary);
}

tr:hover {
    background-color: var(--bg-tertiary);
}

/* Images */
img {
    max-width: 100%;
    height: auto;
    border-radius: 8px;
    margin: 1em 0;
    display: block;
}

/* YouTube Embeds */
.youtube-embed {
    position: relative;
    width: 100%;
    max-width: 640px;
    margin: 1em 0;
    padding-bottom: 56.25%; /* 16:9 aspect ratio */
    height: 0;
    overflow: hidden;
    border-radius: 8px;
    background-color: var(--bg-secondary);
}

.youtube-embed iframe {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    border: none;
    border-radius: 8px;
}

/* Horizontal rule */
hr {
    border: none;
    border-top: 2px solid var(--border);
    margin: 2em 0;
}

/* Strikethrough */
del {
    color: var(--fg-muted);
    text-decoration: line-through;
}

/* Strong and emphasis */
strong {
    font-weight: 600;
    color: var(--fg-primary);
}

em {
    font-style: italic;
}

/* Footnotes */
.footnote-definition {
    font-size: 0.9em;
    color: var(--fg-secondary);
    margin-top: 2em;
    padding-top: 1em;
    border-top: 1px solid var(--border);
}

/* Selection */
::selection {
    background-color: var(--accent);
    color: var(--bg-primary);
}

/* Scrollbar */
::-webkit-scrollbar {
    width: 8px;
    height: 8px;
}

::-webkit-scrollbar-track {
    background: var(--bg-secondary);
}

::-webkit-scrollbar-thumb {
    background: var(--fg-muted);
    border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
    background: var(--fg-secondary);
}

/* Highlight.js theme integration */
.hljs {
    background: transparent !important;
}
"#
        .to_string()
    }

    /// Retorna el JavaScript para interactividad
    fn get_javascript(&self) -> String {
        r#"
// Bridge de comunicación con Rust via WebKit UserContentManager
function notifyRust(action, ...args) {
    try {
        // WebKit espera un mensaje en formato específico
        const message = JSON.stringify({
            action: action,
            args: args
        });
        
        // Enviar al handler registrado en Rust
        if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.notnative) {
            window.webkit.messageHandlers.notnative.postMessage(message);
        } else {
            console.warn('WebKit message handler not available:', action, args);
        }
    } catch (e) {
        console.error('Error sending message to Rust:', e);
    }
}

// Handler para clicks en checkboxes de TODOs
function handleTodoClick(event, lineNum, isChecked) {
    event.stopPropagation(); // Evitar que el click se propague
    console.log('TODO clicked:', lineNum, isChecked);
    notifyRust('todo-toggle', lineNum, isChecked);
}

// Inicialización
document.addEventListener('DOMContentLoaded', function() {
    // Prevenir arrastrar links
    document.querySelectorAll('a').forEach(function(link) {
        link.addEventListener('dragstart', function(e) {
            e.preventDefault();
        });
    });
    
    // Añadir clase a listas con tasks
    document.querySelectorAll('li').forEach(function(li) {
        if (li.querySelector('input[type="checkbox"]')) {
            li.classList.add('task-list-item');
            li.parentElement.classList.add('contains-task-list');
        }
    });
});

// Función para obtener posición de scroll (usada por Rust)
function getScrollPosition() {
    return {
        x: window.scrollX,
        y: window.scrollY,
        maxY: document.body.scrollHeight - window.innerHeight
    };
}

// Función para establecer posición de scroll (usada por Rust)
function setScrollPosition(y) {
    window.scrollTo(0, y);
}

// Función para scroll a porcentaje (usada por Rust)
function setScrollPercent(percent) {
    const maxScroll = document.body.scrollHeight - window.innerHeight;
    window.scrollTo(0, maxScroll * percent);
}
"#
        .to_string()
    }
}

/// Renderiza markdown a HTML con el tema por defecto
pub fn render_markdown_to_html(markdown: &str) -> String {
    HtmlRenderer::default().render(markdown)
}

/// Renderiza markdown a HTML con tema específico
pub fn render_markdown_to_html_themed(markdown: &str, theme: PreviewTheme) -> String {
    HtmlRenderer::new(theme).render(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_rendering() {
        let md = "# Hello World\n\nThis is a **test**.";
        let html = render_markdown_to_html(md);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("<strong>"));
    }

    #[test]
    fn test_todo_checkboxes() {
        let md = "- [ ] Unchecked task\n- [x] Checked task";
        let html = render_markdown_to_html(md);

        assert!(html.contains(r#"type="checkbox""#));
        assert!(html.contains("onclick"));
        assert!(html.contains("notifyRust"));
    }

    #[test]
    fn test_internal_links() {
        let md = "Link to [[My Note]] here.";
        let html = render_markdown_to_html(md);

        assert!(html.contains("internal-link"));
        assert!(html.contains("data-note"));
        assert!(html.contains("My Note"));
    }

    #[test]
    fn test_code_blocks() {
        let md = "```rust\nfn main() {}\n```";
        let html = render_markdown_to_html(md);

        assert!(html.contains("<pre>"));
        assert!(html.contains("<code"));
    }

    #[test]
    fn test_tables() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = render_markdown_to_html(md);

        assert!(html.contains("<table>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
    }

    #[test]
    fn test_theme_class() {
        let md = "# Test";

        let light = HtmlRenderer::new(PreviewTheme::Light).render(md);
        assert!(light.contains(r#"class="light""#));

        let dark = HtmlRenderer::new(PreviewTheme::Dark).render(md);
        assert!(dark.contains(r#"class="dark""#));
    }
}
