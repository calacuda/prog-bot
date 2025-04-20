trace:
  glab ci trace

ci:
  glab ci status -l

lint:
  glab ci lint

pipeline:
  glab ci view

_new-window NAME CMD:
  tmux new-w -t prog-bot -n "{{NAME}}"
  tmux send-keys -t prog-bot:"{{NAME}}" "{{CMD}}" ENTER

_new-tmux:
  tmux new -ds prog-bot -n "README"
  tmux send-keys -t prog-bot:README 'nv ./README.md "+set wrap"' ENTER
  @just _new-window "Run MB" "cd ./core/message-bus/ && cargo run"
  @just _new-window "Edit" "cd ./core"
  @just _new-window "Run" "cd ./core"
  @just _new-window "Run 2" "cd ./core"  
  @just _new-window "git" "git status"

tmux:
  tmux has-session -t prog-bot || just _new-tmux
  tmux a -t prog-bot
