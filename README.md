# NotNative

<div align="center">

**A blazingly fast native note-taking application for Linux**

Built with ❤️ for [Omarchy OS](https://omarchy.org) by [k4ditano](https://github.com/k4ditano) @ [h2r](https://github.com/h2r)

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![GTK4](https://img.shields.io/badge/GTK4-4A86CF?style=for-the-badge&logo=gtk&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)

</div>

---

## 📚 Documentation

- **[MCP Integration Guide](docs/MCP_INTEGRATION.md)** - Complete guide for the Model Context Protocol server, including:
  - 8 available tools (create_note, read_note, update_note, delete_note, list_notes, search_notes, add_tags, append_to_note)
  - Remote access via tunnels (Cloudflare, Tailscale, ngrok, SSH, WireGuard)
  - 7 practical automation examples (iOS Shortcuts, Telegram bot, Gmail sync, Notion sync, screenshots, Habitica, and more)
  - Security best practices and troubleshooting
  
- **[Background Control Guide](docs/BACKGROUND_CONTROL.md)** - Guide for background mode and external control system

---

## 🌟 About

**NotNative** is a native desktop note-taking application designed specifically for **Omarchy OS**, featuring seamless integration with the Omarchy theming system. Built with modern technologies and vim-inspired commands, it delivers maximum speed and efficiency for power users.

### 🎨 Omarchy OS Integration

- **Automatic Theme Adaptation**: Dynamically adapts to Omarchy's system theme without libadwaita
- **Real-time Theme Watching**: Automatically updates when you switch themes in Omarchy
- **Native GTK4**: Pure GTK4 implementation for perfect integration with the Omarchy desktop
- **Optimized for Wayland**: First-class support for modern Wayland compositors
- **System Portals**: Full integration with Omarchy's D-Bus portals

## 📸 Screenshots & Demo

<div align="center">

### Demo Video

<video src="https://github.com/k4ditano/notnative-app/raw/master/screenshots/screensaver.mp4" controls width="800">
  Your browser does not support the video tag.
</video>

*Full demonstration of NotNative features in action*

### YouTube Video Integration & Transcripts
![YouTube Integration](screenshots/youtube-transcript.png)
*Embed YouTube videos with automatic transcript extraction*

### TODO Lists & Checkboxes
![TODO Lists](screenshots/todo-checkboxes.png)
*Interactive TODO lists with checkbox support - Markdown checkbox syntax with real-time rendering*

### YouTube Music Player
![Music Player](screenshots/music-player.png)
*Built-in YouTube music player with playlist management, search, and persistent playlists*

</div>

## ✨ Features

### ✅ Implemented (v0.1)

#### Vim-inspired Modal Text Editor
- **Lightning-fast text buffer** powered by `ropey` with O(log n) operations
- **Modal command system** inspired by vim (Normal/Insert/Command/Visual modes)
- **Granular Undo/Redo** with 1000-operation history
- **Complete vim navigation**: `h/j/k/l`, `0/$`, `gg/G`
- **Editing commands**: `x` (delete char), `dd` (delete line), `i` (insert mode)

#### GTK4 Interface + Theming
- **Native GTK4 interface** without libadwaita (pure GTK)
- **Omarchy theme integration** - Auto-detects and adapts to system theme
- **Real-time theme switching** - Automatically updates when you switch themes in Omarchy
- **Optimized margins** - Improved visual spacing in TextView and HeaderBar
- **Status bar** with mode indicator and real-time statistics

#### File System & Persistence
- **Markdown file system** - Each note saved as an independent .md file
- **Automatic persistence** - Notes saved in `~/.local/share/notnative/notes/`
- **Smart autosave** - Saves every 5 seconds and on close (only if modified)
- **Visual indicators** - Shows `●` in title when there are unsaved changes
- **Note management** - Create, load, save, and list .md notes
- **Welcome note** - Automatically created on first run
- **Dynamic title** - Window displays current note name

#### Advanced Markdown Features
- **Real-time markdown rendering** - Clean view without symbols in Normal mode
- **Robust parser** with `pulldown-cmark` - Handles offsets correctly
- **Rich syntax support**: 
  - Headings (`#`, `##`, `###`)
  - Bold (`**text**`) and Italic (`*text*`)
  - Inline code (`` `code` ``) and blocks (` ``` `)
  - Clickable links (`[text](url)`)
  - Lists (`-` with bullets `•`)
  - Blockquotes (`>`)
  - **Checkboxes / TODO lists** (`- [ ]` / `- [x]`)
  - **Images** - Inline image preview with click to open
- **Dual mode**: 
  - Normal mode: Clean view without markdown symbols
  - Insert mode: Raw text with all symbols visible
- **GTK TextTags styling** - Adapted to system theme
- **Interactive elements** - Clickable links, images, and checkboxes

#### YouTube Integration
- **YouTube video embeds** - Paste YouTube URLs and see video preview
- **Video transcription** - Automatic video transcript extraction
- **Embedded player** - Watch videos directly in NotNative
- **Transcript viewer** - Read and search through video transcripts
- **Local server** - Built-in HTTP server for video playback on `localhost:8787`

#### YouTube Music Player
- **Search YouTube music** - Search and play music directly from YouTube
- **Audio-only playback** - Efficient audio streaming using MPV and yt-dlp
- **Playlist management** - Create, save, and load multiple playlists
- **Persistent playlists** - Playlists saved as JSON files in `~/.config/notnative/playlists/`
- **Queue management** - Add, remove, and reorder songs in the current queue
- **Playback controls** - Play, pause, next, previous, shuffle, and repeat modes
- **Auto-update playlists** - When loading a saved playlist and adding songs, saving automatically updates without asking for name
- **Background playback** - Music continues playing while you work on notes
- **Smart popover UI** - Clean interface that auto-closes after interactions
- **Now playing indicator** - Shows current song title and artist with loading states

#### MCP Server (Model Context Protocol) 🆕
- **REST API Server** - HTTP server on port 8788 exposing note operations
- **8 Core Tools** - CreateNote, ListNotes, SearchNotes, ReadNote, UpdateNote, AppendToNote, DeleteNote, ListTags
- **AI Integration** - Built-in AI chat with OpenAI/OpenRouter support
- **External Automation** - Control NotNative from n8n, Python scripts, curl, etc.
- **JSON-RPC 2.0** - Standard protocol for tool calling
- **CORS Enabled** - Access from web applications and external tools
- **Auto-refresh** - UI updates automatically when notes are modified via MCP
- **Complete Documentation** - See `docs/MCP_INTEGRATION.md` for examples

#### Background Mode & Control 🆕
- **Single Instance** - Only one instance can run at a time (PID lock file)
- **Hide to Background** - Closing window minimizes to background, app keeps running
- **MCP Server 24/7** - Server remains active even when window is hidden
- **File-based Control** - Control app via `/tmp/notnative.control` (show/hide/quit)
- **Shell Script** - `notnative-control.sh` for easy control from terminal
- **Waybar Integration** - Custom module with click actions
- **Hyprland Shortcuts** - Bind global shortcuts to show/hide NotNative
- **Auto-cleanup** - All temp files cleaned on exit
- **See Documentation** - `docs/BACKGROUND_CONTROL.md` for complete guide

#### Database & Organization
- **SQLite indexing** - Fast full-text search across all notes
- **Tag system** - Organize notes with tags (YAML frontmatter support)
- **Tag autocomplete** - Smart tag suggestions based on existing tags
- **Folder hierarchy** - Nested folder support with expandable tree view
- **Search & filter** - Find notes by content, tags, or filename

#### Sidebar & Navigation
- **Sliding sidebar** with smooth open/close animations
- **Folder system** - Hierarchical note organization
- **Expandable folders** - Click to expand/collapse with visual feedback
- **Drag & drop** - Reorder notes, move between folders, nest folders
- **Keyboard navigation** - `j/k` to move, `l/Esc` to close
- **Hover to load** - Hover over a note to load it automatically
- **Context menu** - Right-click to rename/delete notes
- **Shortcuts** - `Ctrl+E` to toggle, button in header

#### Configuration & Preferences
- **Preferences dialog** - Complete settings interface
- **Notes directory** - Configure custom location for notes
- **Autosave interval** - Customize automatic save frequency
- **Theme selection** - Choose light/dark/system theme
- **Font customization** - Configure font family and size
- **Markdown toggle** - Enable/disable real-time rendering

#### Keyboard & Events
- **Keyboard events** integrated with command system
- **Accent composition** - Full support for special characters (á, é, í, ó, ú, ñ)
- **All special characters** work correctly (.,!?:;/etc)
- **Global shortcuts**: `Ctrl+S` (save), `Ctrl+D` (toggle theme), `Ctrl+E` (sidebar), `Ctrl+F` (search), `Ctrl+Shift+F` (full-text search)

## 🚀 Installation

### Requirements

- Rust 1.70+
- GTK4
- libadwaita (optional - NotNative uses pure GTK4)
- MPV (for music player)
- yt-dlp (for YouTube audio streaming)
- OpenAI API Key (optional - for AI chat features)

### Arch Linux (Recommended for Omarchy OS)

#### Using AUR (Recommended)

```bash
# Using yay
yay -S notnative-app

# Or using paru
paru -S notnative-app
```

#### Manual Installation from AUR

```bash
git clone https://aur.archlinux.org/notnative-app.git
cd notnative-app
makepkg -si
```

#### From Source

1. **Install dependencies:**

```bash
sudo pacman -S rust gtk4 base-devel mpv yt-dlp
```

2. **Clone the repository:**

```bash
git clone https://github.com/k4ditano/notnative-app.git
cd notnative-app
```

3. **Build and install:**

```bash
cargo build --release
sudo ./install.sh
```

This will:
- Build the optimized release binary
- Install to `/usr/local/bin/notnative-app`
- Install desktop entry for application launcher
- Install icon/logo assets

### Other Linux Distributions

#### Ubuntu/Debian

```bash
sudo apt install libgtk-4-dev build-essential
cargo build --release
sudo ./install.sh
```

#### Fedora

```bash
sudo dnf install gtk4-devel gcc
cargo build --release
sudo ./install.sh
```

### Running

```bash
# If installed system-wide
notnative-app

# Or from source
cargo run --release
```

## 🔌 MCP Server & External Automation

NotNative includes a **Model Context Protocol (MCP) server** that allows external tools to interact with your notes programmatically.

### Quick Start

The MCP server starts automatically when you launch NotNative and listens on `http://localhost:8788`.

**Test the server:**
```bash
# Health check
curl http://localhost:8788/health

# List available tools
curl -X POST http://localhost:8788/mcp/list_tools \
  -H "Content-Type: application/json"

# Create a note
curl -X POST http://localhost:8788/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "CreateNote",
    "name": "API Test",
    "content": "Created via MCP API!"
  }'
```

### Available Tools

- **CreateNote** - Create new notes
- **ListNotes** - List all notes with metadata
- **SearchNotes** - Full-text search across notes
- **ReadNote** - Read note content
- **UpdateNote** - Replace note content
- **AppendToNote** - Add content to end of note
- **DeleteNote** - Delete a note
- **ListTags** - Get all tags used in notes

### Integration Examples

**Python Script:**
```python
import requests

def create_note(title, content):
    response = requests.post('http://localhost:8788/mcp/call_tool',
        json={
            'tool': 'CreateNote',
            'name': title,
            'content': content
        })
    return response.json()

create_note("Python Note", "Created from Python!")
```

**n8n Workflow:**
Use the HTTP Request node to call the MCP API and automate note creation from webhooks, emails, RSS feeds, etc.

**Complete Documentation:**
See [`docs/MCP_INTEGRATION.md`](docs/MCP_INTEGRATION.md) for detailed examples including:
- curl commands
- Python scripts  
- JavaScript/Node.js
- n8n workflows
- Deployment options (Cloudflare Tunnel, Tailscale, VPN)

## 🎛️ Background Mode & Control

NotNative can run in the background, keeping the MCP server active even when the window is hidden.

### Single Instance

Only one instance of NotNative can run at a time. If you try to launch it again:

```bash
$ notnative-app
❌ NotNative ya está corriendo (PID: 123456)
💡 Si crees que esto es un error, elimina: /tmp/notnative.lock
```

### Hide to Background

When you close the window (X button or Ctrl+Q), NotNative **minimizes to background** instead of closing:
- Window becomes hidden
- MCP Server stays active on port 8788
- Music player continues playing
- All functionality remains available

### Control Script

Use the included `notnative-control.sh` script to control the app:

```bash
# Show window
./notnative-control.sh show

# Hide window
./notnative-control.sh hide

# Toggle show/hide
./notnative-control.sh toggle

# Quit completely
./notnative-control.sh quit
```

Or directly:
```bash
echo "show" > /tmp/notnative.control
echo "hide" > /tmp/notnative.control
echo "quit" > /tmp/notnative.control
```

### Waybar Integration

Add to your `~/.config/waybar/config`:

```json
{
  "modules-right": ["custom/notnative", "..."],
  
  "custom/notnative": {
    "format": "📝 NotNative",
    "on-click": "/path/to/notnative-control.sh toggle",
    "on-click-right": "/path/to/notnative-control.sh quit",
    "tooltip": "Click: Show/Hide | Right-click: Quit"
  }
}
```

### Hyprland Shortcuts

Add to `~/.config/hypr/hyprland.conf`:

```conf
# Toggle NotNative window
bind = SUPER, N, exec, /path/to/notnative-control.sh toggle

# Quit NotNative
bind = SUPER_SHIFT, N, exec, /path/to/notnative-control.sh quit
```

**Complete Documentation:**
See [`docs/BACKGROUND_CONTROL.md`](docs/BACKGROUND_CONTROL.md) for detailed guide.

## ⌨️ Keyboard Shortcuts

### Normal Mode (default)

- `i` - Enter INSERT mode
- `:` - Enter COMMAND mode
- `h/j/k/l` - Move cursor (left/down/up/right)
- `0` - Beginning of line
- `$` - End of line
- `gg` - Beginning of document
- `G` - End of document
- `x` - Delete character
- `dd` - Delete line
- `u` - Undo
- `Ctrl+z` - Undo
- `Ctrl+r` - Redo
- `Ctrl+s` - Save
- `Ctrl+d` - Toggle theme
- `Ctrl+e` - Toggle sidebar
- `Ctrl+f` - Search in current note
- `Ctrl+Shift+f` - Search in all notes (full-text search)

### Insert Mode

- `Esc` - Return to NORMAL mode
- `Ctrl+s` - Save
- All normal keys insert text

### Command Mode

- `:w` - Save
- `:q` - Quit
- `:wq` - Save and quit
- `:q!` - Quit without saving

### Interface

- **Settings Menu** (⚙️) - Access preferences and configuration
- **Mode indicator** (footer left) - Shows current mode (NORMAL/INSERT)
- **Statistics** (footer right) - Lines, words, and unsaved changes

## 🏗️ Architecture

```
src/
├── main.rs              # Bootstrap, GTK init, Omarchy theme loading
├── app.rs               # Main UI logic with Relm4 (2500+ lines)
└── core/
    ├── mod.rs           # Public module exports
    ├── note_buffer.rs   # Text buffer with ropey + undo/redo
    ├── command.rs       # Vim command parser and actions
    ├── editor_mode.rs   # Modes: Normal, Insert, Command, Visual
    ├── note_file.rs     # .md file management and notes directory
    ├── markdown.rs      # Markdown parser with pulldown-cmark
    └── notes_config.rs  # Configuration (coming soon)
```

### File System

- **Base directory**: `~/.local/share/notnative/notes/`
- **Format**: Each note is an independent `.md` file
- **Structure**: Basic folder support (improvements pending)
- **Backup-friendly**: Files are standard readable markdown
- **Autosave**: Every 5 seconds if there are changes

### Technology Stack

- **Rust 2024 Edition** - Base language
- **GTK4** - Native toolkit (without libadwaita)
- **Relm4 0.10** - Reactive framework for GTK4
- **ropey 1.6** - Rope data structure for efficient text editing
- **pulldown-cmark 0.10** - Robust markdown parser with offsets
- **notify 6** - Watcher for system theme changes
- **serde + serde_json** - Serialization (for future config)
- **dirs 5** - System directory management
- **anyhow + thiserror** - Error handling

## 🎨 Design Philosophy

NotNative is designed to be:

1. **Fast**: O(log n) editing operations, no UI blocking
2. **Native**: Full desktop integration (Wayland, portals, D-Bus)
3. **Minimalist**: Clean interface, keyboard-only navigation
4. **Extensible**: Modular architecture ready for plugins
5. **Omarchy-first**: Built specifically for Omarchy OS theme integration

## 🔧 Development

### Tests

```bash
cargo test
```

### Buffer Structure

`NoteBuffer` uses `ropey::Rope` internally:
- Insert/delete operations: O(log n)
- Line↔character conversions: O(log n)
- Line access: O(log n)
- Undo/redo with operation stack (1000 operation history)

### Command System

```rust
KeyPress → CommandParser → EditorAction → NoteBuffer → sync_to_view()
```

Flow:
1. `EventControllerKey` captures keys in `text_view`
2. `CommandParser` converts key + mode into `EditorAction`
3. `MainApp::execute_action()` modifies the `NoteBuffer`
4. `sync_to_view()` updates GTK `TextBuffer`
5. In Normal mode: applies markdown styles and renders clean text
6. In Insert mode: shows raw text with symbols

### Markdown Rendering

Dual visualization mode:

- **Normal Mode**: Clean view
  - Markdown symbols are hidden (`**`, `#`, `` ` ``, etc.)
  - GTK TextTags styles applied (bold, italic, headings)
  - Links are clickable with pointer cursor
  - Position mapping buffer ↔ displayed text

- **Insert Mode**: Raw view
  - All markdown symbols visible
  - No styles applied (plain text)
  - Allows direct markdown editing

### Omarchy Theme Integration

NotNative integrates with the Omarchy theme system:

1. **Initial load**: Reads CSS from `~/.config/omarchy/current/theme/*.css`
2. **Watcher**: `notify` thread detects symlink changes
3. **Reload**: Applies new CSS and updates TextTag colors
4. **Adaptation**: Link and code colors extracted from theme

This seamless integration means NotNative always matches your Omarchy desktop appearance, providing a truly unified experience.

## 📋 Roadmap

### 🔥 High Priority (Active Development)

#### 1. Drag & Drop in Sidebar ✅ IMPLEMENTED
- [x] Implement `gtk::DragSource` in ListBox rows
- [x] Implement `gtk::DropTarget` to receive drops
- [x] Detect drop between notes (reorder)
- [x] Detect drop on folders (move note to folder)
- [x] Detect folder on folder drop (nesting)
- [x] Update file structure on disk
- [x] Visual animations during drag
- [x] Visual feedback (placeholder, highlight)
- [x] Persist new order in metadata

### 🔥 High Priority

#### 2. SQLite Indexing System ✅ IMPLEMENTED
- [x] Create database module (`src/core/database.rs`)
- [x] SQLite schema:
  - `notes` table (id, path, name, content, tags, created_at, modified_at)
  - FTS5 virtual table for full-text search
- [x] Index all notes on startup
- [x] Incremental updates:
  - Add note on creation
  - Update note on save
- [x] Watcher to update index on file changes
- [x] Re-index on note save
- [x] Schema migration and versioning

#### 3. Full-Text Search ✅ IMPLEMENTED
- [x] Search bar in sidebar header
- [x] Entry widget with search button
- [x] Query SQLite FTS5
- [x] Display results in sidebar
- [x] Highlight matches in results
- [x] Search by:
  - Note name
  - Content
  - Tags
  - Date (creation/modification)
- [x] Real-time filtering (debounce)
- [x] Show context snippets

#### 4. Tag System with Autocompletion ✅ IMPLEMENTED

#### 3. Full-Text Search ⚡ NEXT
- [ ] Search bar in sidebar header
- [ ] Entry widget with search button
- [ ] Query SQLite FTS5
- [ ] Display results in sidebar
- [ ] Highlight matches in results
- [ ] Search by:
  - Note name
  - Content
  - Tags
  - Date (creation/modification)
- [ ] Real-time filtering (debounce)
- [ ] Show context snippets

#### 4. Tag System with Autocompletion ✅ IMPLEMENTED
- [x] Parse YAML frontmatter in notes:
  ```yaml
  ---
  tags: [tag1, tag2, tag3]
  ---
  ```
- [x] Store tags in database
- [x] Tag input widget in header/footer
- [x] Autocompletion with `gtk::EntryCompletion`
- [x] Suggestions based on existing tags
- [x] Most used tags view
- [x] Filter sidebar by tag
- [ ] Color coding for tags (optional)

#### 5. Complete Context Menu
- [ ] Implement note renaming (structure exists, logic pending)
- [ ] Implement note deletion (base implemented, refine)
- [ ] Add deletion confirmation (dialog)
- [ ] Update sidebar after rename/delete
- [ ] Handle folders in context menu
- [ ] Create new folder from menu

#### 6. Improve Markdown Rendering
- [ ] Syntax highlighting in code blocks (use `syntect` or similar)
- [ ] Inline image support
- [ ] Markdown tables
- [ ] Nested and numbered lists
- [ ] Checkboxes (`- [ ]` / `- [x]`)
- [ ] Improve link colors based on current theme

### ⚡ Medium Priority (UX & Polish)

#### 7. Optional Markdown Preview
- [ ] Side panel with rendered preview
- [ ] Toggle to show/hide preview
- [ ] Synchronized scroll between editor and preview
- [ ] Use WebKit or native widget for rendering

#### 8. Additional Keyboard Shortcuts
- [ ] `Ctrl+N` - New note (alternative to dialog)
- [ ] `Ctrl+F` - Search in current note
- [ ] `Ctrl+Shift+F` - Search in all notes
- [ ] `/` in Normal mode - Quick search
- [ ] `:e <name>` - Open note by name

#### 9. Configuration & Preferences ✅ IMPLEMENTED
- [x] Functional preferences dialog
- [x] Configure notes directory
- [x] Configure autosave interval
- [x] Choose theme (light/dark/system)
- [x] Configure font and size
- [x] Enable/disable markdown rendering

#### 10. "About" Window
- [ ] Dialog with project information
- [ ] Current version
- [ ] License (MIT)
- [ ] Credits and links

### 🎨 Low Priority (Nice-to-Have)

#### 11. Export
- [ ] Export current note to HTML
- [ ] Export current note to PDF
- [ ] Export all notes (zip)

### 🚀 Implemented in v0.1.5-preview

#### 12. MCP Server (Model Context Protocol) ✅ IMPLEMENTED
- [x] REST API server on port 8788
- [x] 8 core tools (CreateNote, ListNotes, SearchNotes, ReadNote, UpdateNote, AppendToNote, DeleteNote, ListTags)
- [x] JSON-RPC 2.0 protocol
- [x] CORS enabled for external access
- [x] Auto-refresh UI when notes change via MCP
- [x] Complete documentation with examples
- [ ] API Key authentication (pending)
- [ ] External tool registry UI (pending)

#### 13. AI Chat Integration ✅ IMPLEMENTED
- [x] OpenAI API integration
- [x] OpenRouter support for multiple models
- [x] Chat with note context
- [x] Intelligent note creation from conversations
- [x] MCP tools available to AI
- [x] Streaming responses
- [ ] Chat memory/history (pending)
- [ ] Multiple conversation threads (pending)

#### 14. Background Mode & Control ✅ IMPLEMENTED
- [x] Single instance detection with PID lock
- [x] Hide to background when closing window
- [x] MCP Server stays active in background
- [x] File-based control system (/tmp/notnative.control)
- [x] Shell script for easy control (notnative-control.sh)
- [x] Waybar integration examples
- [x] Hyprland shortcuts support
- [x] Auto-cleanup of temp files
- [x] Complete documentation

### 🚀 Future (v0.2+)

#### 15. Hyprland Layer Shell Integration
- [ ] Layer-shell for overlay mode
- [ ] Enhanced IPC with Hyprland
- [ ] Compositor-level global shortcuts
- [ ] Floating "quick note" mode
- [ ] Window animations and effects

#### 16. Export & Publishing
- [ ] Basic Git sync
- [ ] Sync with cloud services (Nextcloud, Syncthing)
- [ ] Conflict detection and resolution

---

## 🗓️ Version Roadmap

- [x] **v0.1.0** - Functional editor with markdown, sidebar and folders ✅
- [x] **v0.1.1** - SQLite indexing, full-text search, tags, YouTube integration, TODO checkboxes, images, drag & drop, preferences ✅
- [x] **v0.1.5-preview** - MCP Server, AI Chat, Background Mode, External Automation ✅ **CURRENT**
- [ ] **v0.2** - Export, preview improvements, about dialog, API Key authentication
- [ ] **v0.3** - Hyprland integration improvements, global shortcuts
- [ ] **v0.4** - AI enhancements, chat memory, context improvements
- [ ] **v0.5** - External MCP registry, plugin system
- [ ] **v0.6** - Cloud synchronization
- [ ] **v1.0** - Stabilization and release

## 🐛 Known Issues & Troubleshooting

### Theme Not Loading

If the Omarchy theme is not applied after installation from AUR:

1. **Verify Omarchy theme files exist:**
   ```bash
   ls ~/.config/omarchy/current/theme/
   ```
   You should see files like `walker.css`, `waybar.css`, and `swayosd.css`.

2. **Check if NotNative detects the theme:**
   Run from terminal to see debug messages:
   ```bash
   notnative-app
   ```
   You should see: `✓ Tema Omarchy cargado desde ~/.config/omarchy/current/theme/`

3. **If theme files don't exist:**
   Install Omarchy theme system or create a symlink to your theme directory.

### Bugs
- [ ] Note renaming not implemented (structure ready, logic pending)
- [ ] Context menu: parent/unparent may cause GTK warnings
- [ ] Nested folders don't display correctly in sidebar
- [ ] Folder deletion not implemented

### Performance Improvements
- [ ] Markdown rendering in separate thread for very long notes
- [ ] Lazy loading of sidebar (load only visible notes)
- [ ] Debounce on sidebar hover (avoid excessive loads)

### UX/UI
- [ ] Sidebar animation could be improved (consider libadwaita AnimatedPane)
- [ ] Visual indicator when autosaving
- [ ] Visual feedback when creating/deleting notes
- [ ] Keyboard shortcuts don't appear in dialog (empty placeholder)

### Refactoring
- [ ] `app.rs` is too large (2500+ lines) - split into modules
- [ ] Separate sidebar logic into independent Relm4 component
- [ ] Extract markdown rendering to separate module
- [ ] Improve error handling (more informative user messages)

---

## 📜 License

MIT License - See [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please open an issue first to discuss major changes.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📊 Project Status

**Current Version**: v0.1.0  
**Last Updated**: November 2025  
**Status**: Alpha - Functional but under active development  
**Lines of Code**: ~4000 lines of Rust  
**Tests**: Pending implementation

---

## 👨‍💻 Author

**NotNative** is built with ❤️ by [k4ditano](https://github.com/k4ditano) @ [h2r](https://github.com/h2r)

Designed specifically for [Omarchy OS](https://omarchy.org) - The next generation Linux distribution.

---

## 🙏 Acknowledgments

- **Omarchy OS** - For providing the inspiration and theming system
- **GTK Team** - For the excellent GTK4 toolkit
- **Rust Community** - For the amazing ecosystem and tools
- All contributors and users who help improve NotNative

---

<div align="center">

**Made for power users who love speed and efficiency** ⚡

[Report Bug](https://github.com/k4ditano/notnative-app/issues) · [Request Feature](https://github.com/k4ditano/notnative-app/issues) · [Documentation](https://github.com/k4ditano/notnative-app/wiki)

</div>

---

## � License

MIT License - See [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please open an issue first to discuss major changes.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📊 Project Status

**Current Version**: v0.1.5-preview  
**Last Updated**: November 2025  
**Status**: Alpha - Advanced features with MCP Server, AI integration, background mode, and external automation  
**Lines of Code**: ~6000 lines of Rust  
**Tests**: 27 passing tests

---

## 👨‍💻 Author

**NotNative** is built with ❤️ by [k4ditano](https://github.com/k4ditano) @ [h2r](https://github.com/h2r)

Designed specifically for [Omarchy OS](https://omarchy.org) - The next generation Linux distribution.

---

## 🙏 Acknowledgments

- **Omarchy OS** - For providing the inspiration and theming system
- **GTK Team** - For the excellent GTK4 toolkit
- **Rust Community** - For the amazing ecosystem and tools
- All contributors and users who help improve NotNative

---

<div align="center">

**Made for power users who love speed and efficiency** ⚡

[Report Bug](https://github.com/k4ditano/notnative-app/issues) · [Request Feature](https://github.com/k4ditano/notnative-app/issues) · [Documentation](https://github.com/k4ditano/notnative-app/wiki)

</div>  
