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
- [ ] utterance detection
- [x] webhook-intake
- [ ] speach to text client
- [ ] stenographer

## TODO

- make this a single project
- use lazy static to start the message bus when necessary (like for tests and stuff)
- model github and gitlab web-hook data
- handle utterace detection with a button press
