# Programming Helper Bot

A programming assistant/chatbot, built for live-streaming programming content.

for an over view of what this project is and what it seeks to do, see: [project-scope](project-scope.md)

## Module Progress

currently working on MVP versions

- [x] message-bus
- [ ] chatbot
- [ ] GUI
- [x] linter
- [ ] LSP
- [x] todo-collector
- [ ] text to speech
- [ ] utterance detection
- [x] webhook-intake
    - [x] need to make it send speak messages too
    - [x] add tests
    - [x] validate the webhook sender using the sercret

## TODO

- make this a single project and use lazy static to start the message bus when necessary.
- model github and gitlab web-hook data 
