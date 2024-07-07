# Prog-Bot Design Document

This document describes the overall design and goals of the Prog-Bot project.

for an outline of project goals see: [What should it do?](#What%20should%20it%20do?).
for an overview of how to go about achieving these goals see: [How should it do this?](#How%20should%20it%20do%20this?).
for an understanding of the message-bus and node architecture, see: [Nodes and IPC](#Nodes%20and%20IPC).

## What should it do?

- collect TODOs from comments in the code and put them in a file
	- notify me of TODOs not completed from last save
	- allow for prioritizing/classifying TODOs
	- have a GUI to show TODOs and classify/organize/silence them
	- have a UUID generator to easily tag TODOs
- ask questions about the code if I'm quiet for too long (this question asker should be contextually aware of the TODOs and of current errors)
- alert on [rust-analyzer](https://rust-analyzer.github.io/) errors when saving the file
- alert on GitHub/GitLab CI/CD fails/successes 

each "skill" can be its own process communicating using a message-bus

## How should it do this?

- TODOs comments will be structured like this: ```// TODO: <UUID> thing to do``` this can then be parsed using [Pest](https://docs.rs/pest/latest/pest/) and/or [serde](https://docs.rs/serde/latest/serde/index.html), and will be used to construct a struct with the fallowing fields:
	- file: a file path (relative to `Cargo.toml`) where the TODO was found.
	- kind: an enum of
		- `TODO`
		- `FIXME`
		- ...
	- uuid: the uuid of the TODO
	- message: the TODO message
	- function: (optional) what function was this TODO found in
	- line_num: the line number where the TODO resides
	- saves_since_creation: how many times has the file been saved since the creation of this TODO
	- priority: how heavy handed the program should be about reminding the user of this TODO
	- muted: should the program notify the user at all
	- tags: vector of strings allowing the user to classify the TODO
- make GUI using [Tauri](https://tauri.app/) and [leptos](https://book.leptos.dev/)
- detect utterances from the microphone use the end time to reset a timer. (asking a question should also restart the timer)
	- can be done by detecting average volume over time
	- requires a calibration button to calibrate for background noise level.
	- questions powered by [Llama 3](https://ollama.com/library/llama3)
	- maintain a script of what has been spoken along with things the user has said to the chat-bot
- middle-ware to accept web-hooks from GitHub and GitLab
- node to speak utterances/messages (powered by [mimic3](https://mycroft-ai.gitbook.io/docs/mycroft-technologies/mimic-tts/mimic-3))
- run rust-analyzer on file saves

## Nodes and IPC

there should be multiple nodes all communicating over a web-socket based message bus. the message bus is quite simple, it will simply forward all incoming messages to all of the other nodes. I want this to function over web-sockets so Prog-Bot can run on a different machine then the machine that is doing the streaming, this is to conserver computational reasources on the streaming PC.

**NOTE:** if the nodes run on different machines/VMs/Containers then I will use a [Nebula Overlay Network](https://nebula.defined.net/docs/) for security.

List of Nodes:
1. TODO collector
2. [Tauri](https://tauri.app/) GUI
3. utterance detector
4. [Llama 3](https://ollama.com/library/llama3) chat-bot
5. GitHub/GitLab web-hook intake server
6. TTS node (should probably run on the streaming machine. or setup [Mimic3](https://mycroft-ai.gitbook.io/docs/mycroft-technologies/mimic-tts/mimic-3) as a server and connect to it from a client running on the streaming machine)
7. [rust-analyzer](https://rust-analyzer.github.io/) node
