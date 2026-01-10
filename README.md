# Otto

AI-powered task executor for macOS. Convert natural language commands into automated desktop actions.

![macOS](https://img.shields.io/badge/macOS-11.0+-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![Ollama](https://img.shields.io/badge/Ollama-required-green)

## What is Otto?

Otto is a lightweight macOS app that listens for natural language commands and executes them as automated actions. Press a hotkey, type what you want to do, and Otto handles the rest.

```
"open safari and search for rust tutorials"
"click on the Settings button"
"type hello world"
```

## Features

- **Natural Language Understanding** - Powered by local LLMs via Ollama
- **Vision-Based Interaction** - Find and click UI elements by description
- **Keyboard & Mouse Automation** - Type text, press keys, click anywhere
- **Global Hotkey** - Activate from anywhere with `Cmd + Shift + Space`
- **Privacy First** - Everything runs locally, no data leaves your machine

## Requirements

- macOS 11.0+
- [Ollama](https://ollama.ai) installed and running
- Required models:
  - `qwen2.5:0.5b` - Command parsing
  - `moondream` - Vision/element detection

## Installation

### 1. Install Ollama

```bash
brew install ollama
```

### 2. Pull required models

```bash
ollama pull qwen2.5:0.5b
ollama pull moondream
```

### 3. Clone and build Otto

```bash
git clone https://github.com/eatnug/otto.git
cd otto
npm install
npm run tauri build
```

The built app will be in `src-tauri/target/release/bundle/`.

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Optional: Auto-sign for persistent permissions
./watch-sign.sh &
npm run tauri dev
```

## Usage

1. **Start Otto** - Launch the app
2. **Activate** - Press `Cmd + Shift + Space`
3. **Type command** - Enter what you want to do
4. **Execute** - Press Enter to run

### Supported Commands

| Command Pattern | Example |
|----------------|---------|
| Open app | `open safari` |
| Search in browser | `open chrome and search for weather` |
| Type text | `type hello world` |
| Click element | `click on the submit button` |
| Find and click | `find and click the search icon` |

## Architecture

```
otto/
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── store/              # Zustand state management
│   └── hooks/              # Tauri event hooks
├── src-tauri/              # Rust backend
│   └── src/
│       ├── llm.rs          # Ollama integration
│       ├── vision.rs       # Vision model for element detection
│       ├── computer.rs     # Keyboard/mouse automation
│       ├── executor.rs     # Action execution engine
│       └── hotkey.rs       # Global shortcut handling
```

## How It Works

1. **Command Input** → User types natural language command
2. **Pattern Matching** → Fast regex matching for common patterns
3. **LLM Fallback** → Complex commands parsed by qwen2.5
4. **Action Plan** → Command converted to step-by-step actions
5. **Execution** → Actions executed via AppleScript/CGEvent

For vision-based commands:
1. **Screenshot** → Capture and resize screen (1280x720)
2. **Vision Model** → moondream analyzes image for target element
3. **Coordinates** → Element position extracted and scaled
4. **Click** → Mouse moved and clicked at location

## Permissions

Otto requires the following macOS permissions:

- **Accessibility** - For keyboard/mouse control
- **Screen Recording** - For vision-based element detection

Grant these in System Preferences → Privacy & Security.

## Troubleshooting

### "Ollama not running"
```bash
ollama serve
```

### Permission prompts on every rebuild
Use the signing script during development:
```bash
./watch-sign.sh &
npm run tauri dev
```

### Vision model slow
The first request warms up the model. Subsequent requests are faster.

## License

MIT

## Acknowledgments

- [Tauri](https://tauri.app) - Desktop app framework
- [Ollama](https://ollama.ai) - Local LLM runtime
- [moondream](https://github.com/vikhyat/moondream) - Vision language model
