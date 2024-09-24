# Programming Helper Bot

A programming assistant/chatbot, built for live-streaming programming content.

for an over view of what this project is and what it seeks to do, see: [project-scope](project-scope.md)

## Module Progress

currently working on MVP versions

- [x] message-bus
 - add file logging
- [ ] chatbot
- [ ] GUI
- [ ] TUI
- [x] linter
<!-- - [ ] LSP -->
- [x] todo-collector
 - [x] update tests to include lsp functionality.
 - [x] fix struct detection
 - [ ] add block comments
- [x] text to speech
  - needs tests
  - add a beep/jingle to play for situations where words would be over kill but user feed back is still needed.
    - decide on a beep/jingle
- [ ] utterance detection
  - [x] triggerable from messagebus
  - [ ] triggerable with voice
- [x] webhook-intake
- [x] speach to text client
  - needs tests

## TODO

- make this a single project
- use lazy static to start the message bus when necessary (like for tests and stuff)
- model github and gitlab web-hook data
- handle utterace detection with a button press
- build a wake word spotter (utterance detection)
- make a `beeps-and-jingles` folder to hold beeps and jingles for the `Beep` message type.
  - make a simple beep
  - make a chord jingle (min-7th chord played a rolled chord, with a wah-wah effect with an increasing depth, applied to it.)
