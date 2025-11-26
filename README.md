# NotNative

<div align="center">

**Your second brain. Native, fast, and powerful.**

A modern note-taking app built for Linux with Vim-like editing, AI chat, MCP automation, and YouTube integration.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![GTK4](https://img.shields.io/badge/GTK4-4A86CF?style=for-the-badge&logo=gtk&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)

[Install](#-installation) • [Features](#-features) • [Documentation](docs/)

</div>

---

## 🎬 See it in action

<div align="center">

### Full Demo

https://github.com/k4ditano/notnative-app/raw/master/screenshots/screensaver.mp4

### YouTube Integration & Transcripts
![YouTube Integration](screenshots/youtube-transcript.png)

### Interactive TODO Lists
![TODO Lists](screenshots/todo-checkboxes.png)

### Built-in Music Player
![Music Player](screenshots/music-player.png)

</div>

---

## ✨ Why NotNative?

**🚀 Blazingly Fast** - Built with Rust. O(log n) editing operations. No lag, ever.

**⌨️ Keyboard First** - Vim-inspired commands. Modal editing. 100+ shortcuts.

**🤖 AI-Powered** - Chat with your notes. Ask questions. Get summaries. Create content.

**🔌 Automation Ready** - REST API included. Control from scripts, n8n, Python, curl.

**🎵 YouTube Built-in** - Embed videos, extract transcripts, play music while you work.

**🌍 Multi-language** - Full i18n support (English/Spanish). More languages coming.

**🎨 Beautiful** - Adaptive themes. WebView HTML preview. Clean, distraction-free interface.

---

## 🚀 Installation

### Arch Linux (Recommended)

```bash
yay -S notnative-app-bin
```

### Other Linux

```bash
# Install dependencies
sudo apt install libgtk-4-dev mpv yt-dlp  # Ubuntu/Debian
sudo dnf install gtk4-devel mpv yt-dlp    # Fedora

# Build from source
git clone https://github.com/k4ditano/notnative-app.git
cd notnative-app/notnative-app
cargo build --release
sudo ./install.sh
```

---

## 💡 Features

### 📝 Smart Editor
- **Vim-inspired modal editing** - Normal, Insert, Visual, Command modes
- **WebView HTML preview** - Beautiful Obsidian-style rendering in Normal mode (v0.1.2)
- **Centered content layout** - Comfortable reading experience, both edit and preview modes (v0.1.2)
- **Keyboard scroll in preview** - Navigate with arrows/j/k/PgUp/PgDown in Normal mode (v0.1.2)
- **Lightning-fast buffer** - Powered by ropey, handles huge documents
- **Interactive TODOs** - Click checkboxes to mark tasks complete
- **Smart tag system** - #tags clickable anywhere, even at line start
- **YAML frontmatter tags** - Tags in lists (• tag) are also clickable with special chars support (v0.1.2)
- **Precise tag search** - Search #tag finds only that specific tag
- **Image preview** - See images inline, click to open
- **🔗 Backlinks with @mentions** - Link notes with `@NoteName`, autocomplete included
- **🔗 Smart URL detection** - Pasted URLs auto-convert to markdown links
- **📂 Open in file manager** - Right-click notes/folders → open in explorer

### 🤖 AI Integration
- **Chat with AI** - Ask questions about your notes
- **OpenAI & OpenRouter support** - Use GPT-4, Claude, or any LLM
- **Context-aware** - Attach notes as context for better answers
- **Smart suggestions** - AI helps you write better
- **40+ MCP tools available** - Advanced automation capabilities

### 🔌 Automation & API
- **MCP Server included** - REST API on port 8788
- **40+ powerful tools** - Comprehensive automation toolkit
- **External control** - Integrate with n8n, Python, curl, anything
- **iOS Shortcuts ready** - Capture notes from your phone
- **Telegram bot support** - Send messages directly to your notes

### 🎵 YouTube Integration
- **Embed videos** - Paste URLs, watch inline
- **Auto-transcripts** - Extract video transcriptions automatically
- **Music player** - Search and play YouTube audio
- **Playlists** - Create, save, and load music playlists
- **Background playback** - Music continues while you work

### 🎨 Beautiful UX
- **Markdown everywhere** - Headings, bold, italic, code, links, lists
- **System tray** - Minimize to tray, control with one click
- **Folder organization** - Nested folders, drag & drop
- **Full-text search** - Find anything instantly with SQLite FTS
- **Tag system** - Organize with tags, auto-completion included
- **Adaptive themes** - Works with your system's color scheme
- **Real-time theme switching** - Changes instantly when you switch themes

---

## ⌨️ Quick Start

### Essential Shortcuts

| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `Esc` | Normal mode |
| `:w` | Save note |
| `Ctrl+N` | New note |
| `Ctrl+F` | Search |
| `Ctrl+E` | Toggle sidebar |
| `dd` | Delete line |
| `u` | Undo |
| `n` | New note |
| `a` | Chat AI mode |

### Create Your First Note

1. Launch NotNative: `notnative-app`
2. Press `Ctrl+N` to create a note
3. Type a name and press Enter
4. Press `i` to start writing
5. Press `Esc` when done, then `:w` to save

---

## 🔌 API & Automation

NotNative includes a **REST API** for external automation:

```bash
# Health check
curl http://localhost:8788/health

# Create a note via API
curl -X POST http://localhost:8788/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "call_tool",
    "params": {
      "tool": "create_note",
      "args": {
        "name": "API Test",
        "content": "# Created via API!\n\nThis is awesome."
      }
    }
  }'
```

**Use cases:**
- 📱 Capture notes from iOS Shortcuts
- 🤖 Create Telegram bot for note-taking
- 📧 Auto-save important emails as notes
- 🔄 Sync with Notion, Obsidian, etc.
- 🎯 Integrate with Habitica for gamification

**[Full API documentation →](docs/MCP_INTEGRATION.md)**

---

## 📚 Documentation

- **[ cURL Examples](docs/CURL_EXAMPLES.md)** - Ready-to-use command examples
- **[🔌 MCP Integration Guide](docs/MCP_INTEGRATION.md)** - Complete API reference
- **[🛠️ MCP Tools Reference](docs/MCP_TOOLS_REFERENCE.md)** - Full list of 40+ available tools
- **[Background Control](docs/BACKGROUND_CONTROL.md)** - System tray and external control

---

## 🛠️ Built With

- **Rust** - Speed and safety
- **GTK4** - Native Linux interface
- **Relm4** - Reactive UI framework
- **ropey** - Fast rope data structure
- **SQLite** - Full-text search
- **MPV** - Audio playback
- **yt-dlp** - YouTube integration

---

## 🗺️ Roadmap

- [x] Vim-inspired editing
- [x] Markdown rendering
- [x] Full-text search
- [x] MCP API server
- [x] AI chat integration
- [x] YouTube player
- [x] System tray
- [x] Multi-language (i18n)
- [x] 40+ MCP automation tools
- [x] Smart tag system
- [x] YAML frontmatter clickable tags
- [x] Precise tag-based search
- [x] WebView HTML preview (v0.1.2)
- [x] Keyboard scroll in preview (v0.1.2)
- [x] Centered content layout (v0.1.2)
- [ ] Mobile app (planned)
- [ ] End-to-end encryption (planned)
- [ ] Cloud sync (planned)

---

## 📄 License

MIT License - See [LICENSE](LICENSE)

---

## 🤝 Contributing

We welcome contributions! Open an issue or submit a PR.

---

<div align="center">

**Built with ❤️ for Linux by [k4ditano](https://github.com/k4ditano)**

⭐ Star this repo if you find it useful!

[Report Bug](https://github.com/k4ditano/notnative-app/issues) • [Request Feature](https://github.com/k4ditano/notnative-app/issues)

</div>
