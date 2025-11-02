use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, CowStr, OffsetIter};

/// Información de estilo para aplicar a un rango de texto
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub start: usize,
    pub end: usize,
    pub style_type: StyleType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleType {
    Heading1,
    Heading2,
    Heading3,
    Bold,
    Italic,
    Code,
    CodeBlock,
    Link,
    Quote,
    Image { src: String, alt: String },
    YouTubeVideo { video_id: String, url: String },
}

/// Parser de markdown que extrae información de estilo
pub struct MarkdownParser {
    text: String,
}

impl MarkdownParser {
    pub fn new(text: String) -> Self {
        Self { text }
    }
    
    /// Parsea el texto markdown usando pulldown-cmark con offsets precisos
    pub fn parse(&self) -> Vec<TextStyle> {
        let mut styles = Vec::new();
        
        // Configurar parser con offsets habilitados
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        
        // Crear parser con offsets
        let parser = Parser::new_ext(&self.text, options).into_offset_iter();
        
        // Stack para trackear contexto anidado
        let mut heading_range: Option<(usize, StyleType)> = None;
        let mut strong_range: Option<usize> = None;
        let mut emphasis_range: Option<usize> = None;
        let mut code_range: Option<usize> = None;
        let mut code_block_range: Option<usize> = None;
        let mut image_range: Option<(usize, String, String)> = None; // (start, src, alt)
        
        for (event, range) in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Heading { level, .. } => {
                            let style = match level {
                                pulldown_cmark::HeadingLevel::H1 => StyleType::Heading1,
                                pulldown_cmark::HeadingLevel::H2 => StyleType::Heading2,
                                pulldown_cmark::HeadingLevel::H3 => StyleType::Heading3,
                                _ => StyleType::Heading3,
                            };
                            heading_range = Some((range.start, style));
                        }
                        Tag::Strong => {
                            strong_range = Some(range.start);
                        }
                        Tag::Emphasis => {
                            emphasis_range = Some(range.start);
                        }
                        Tag::CodeBlock(_) => {
                            code_block_range = Some(range.start);
                        }
                        Tag::Image { dest_url, title, .. } => {
                            image_range = Some((range.start, dest_url.to_string(), title.to_string()));
                        }
                        _ => {}
                    }
                }
                Event::End(tag_end) => {
                    match tag_end {
                        TagEnd::Heading(_) => {
                            if let Some((start, style)) = heading_range.take() {
                                // Para headings, aplicar a toda la línea incluyendo el #
                                styles.push(TextStyle {
                                    start,
                                    end: range.end,
                                    style_type: style,
                                });
                            }
                        }
                        TagEnd::Strong => {
                            if let Some(start) = strong_range.take() {
                                // Excluir los ** delimitadores
                                let content_start = start + 2; // Skip "**"
                                let content_end = if range.end >= 2 { range.end - 2 } else { range.end }; // Skip "**"
                                
                                if content_start < content_end {
                                    styles.push(TextStyle {
                                        start: content_start,
                                        end: content_end,
                                        style_type: StyleType::Bold,
                                    });
                                }
                            }
                        }
                        TagEnd::Emphasis => {
                            if let Some(start) = emphasis_range.take() {
                                // Excluir los * delimitadores
                                let content_start = start + 1; // Skip "*"
                                let content_end = if range.end >= 1 { range.end - 1 } else { range.end }; // Skip "*"
                                
                                if content_start < content_end {
                                    styles.push(TextStyle {
                                        start: content_start,
                                        end: content_end,
                                        style_type: StyleType::Italic,
                                    });
                                }
                            }
                        }
                        TagEnd::CodeBlock => {
                            if let Some(start) = code_block_range.take() {
                                styles.push(TextStyle {
                                    start,
                                    end: range.end,
                                    style_type: StyleType::CodeBlock,
                                });
                            }
                        }
                        TagEnd::Image => {
                            if let Some((start, src, alt)) = image_range.take() {
                                styles.push(TextStyle {
                                    start,
                                    end: range.end,
                                    style_type: StyleType::Image { src, alt },
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Event::Code(_) => {
                    // Excluir los backticks del rango
                    // El rango incluye los ` delimitadores, necesitamos solo el contenido
                    if range.start < range.end {
                        // Buscar el primer backtick después de range.start
                        let content_start = self.text[range.start..range.end]
                            .find('`')
                            .map(|pos| range.start + pos + 1)
                            .unwrap_or(range.start);
                        
                        // Buscar el último backtick antes de range.end
                        let content_end = self.text[range.start..range.end]
                            .rfind('`')
                            .map(|pos| range.start + pos)
                            .unwrap_or(range.end);
                        
                        if content_start < content_end {
                            styles.push(TextStyle {
                                start: content_start,
                                end: content_end,
                                style_type: StyleType::Code,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        
        styles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_heading_detection() {
        let text = "# Heading 1\n## Heading 2\n### Heading 3".to_string();
        let parser = MarkdownParser::new(text);
        let styles = parser.parse();
        
        assert!(styles.iter().any(|s| s.style_type == StyleType::Heading1));
        assert!(styles.iter().any(|s| s.style_type == StyleType::Heading2));
        assert!(styles.iter().any(|s| s.style_type == StyleType::Heading3));
    }
    
    #[test]
    fn test_bold_italic() {
        let text = "**bold** and *italic*".to_string();
        let parser = MarkdownParser::new(text);
        let styles = parser.parse();
        
        assert!(styles.iter().any(|s| s.style_type == StyleType::Bold));
        assert!(styles.iter().any(|s| s.style_type == StyleType::Italic));
    }
    
    #[test]
    fn test_code() {
        let text = "inline `code` here".to_string();
        let parser = MarkdownParser::new(text);
        let styles = parser.parse();
        
        assert!(styles.iter().any(|s| s.style_type == StyleType::Code));
    }
}
