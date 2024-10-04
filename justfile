trace:
  glab ci trace

ci:
  glab ci status -l

lint:
  glab ci lint

pipeline:
  glab ci view

new-window NAME CMD:
  tmux new-w -t prog-bot -n "{{NAME}}"
  tmux send-keys -t prog-bot:"{{NAME}}" "{{CMD}}" ENTER

tmux:
  tmux new -ds prog-bot -n "README"
  tmux send-keys -t prog-bot:README 'nv ./README.md "+set wrap"' ENTER
  @just new-window "Run MB" "cd ./core/message-bus/ && cargo run"
  @just new-window "Edit" "cd ./core"
  @just new-window "Run" "cd ./core"
  @just new-window "Run 2" "cd ./core"  
  @just new-window "git" "git status"
  tmux a -t prog-bot

